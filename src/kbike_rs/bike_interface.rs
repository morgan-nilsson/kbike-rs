use embassy_nrf::{Peripherals, saadc, Peri};
use embassy_nrf::gpio::{AnyPin, Input, Pull};
use embassy_nrf::saadc::{AnyInput, ChannelConfig, Input as SaadcInput, Saadc};
use embassy_nrf::twim::{self, Twim};
use embassy_time::{Duration, Instant, Ticker, Timer};
use embassy_futures::select::{select, Either};
use crate::Irqs;
use crate::config::CONFIG;
use crate::kbike_rs::ride_state::ride_get;

use super::ride_state::ride_update;

const CRANK_DEBOUNCE_MS: u16 = 150;

pub struct Board {
    pub wiper: saadc::AnyInput<'static>,
    pub reed: Peri<'static, AnyPin>,
    pub saadc: Peri<'static, embassy_nrf::peripherals::SAADC>,
    pub twim_pin: Peri<'static, embassy_nrf::peripherals::TWISPI0>,
    pub scl: Peri<'static, AnyPin>,
    pub sda: Peri<'static, AnyPin>,
    pub bat_vol: saadc::AnyInput<'static>,
}

impl Board {
    pub fn split(p: Peripherals) -> Board {
        let reed: Peri<'static, AnyPin> = match CONFIG.reed_pin {
            "P1_08" => p.P1_08.into(), // D5
            "P0_07" => p.P0_07.into(), // D6
            "P0_26" => p.P0_26.into(), // D9
            "P0_27" => p.P0_27.into(), // D10
            "P0_06" => p.P0_06.into(), // D11
            "P0_08" => p.P0_08.into(), // D12
            "P0_14" => p.P0_14.into(), // SCK   (free — no SPI in this project)
            "P0_13" => p.P0_13.into(), // MOSI  (free)
            "P0_15" => p.P0_15.into(), // MISO  (free)
            "P0_24" => p.P0_24.into(), // RX    (free — UART unused)
            "P0_25" => p.P0_25.into(), // TX    (free)
            other => panic!("cfg.toml reed_pin '{}' not in whitelist", other),
        };

        let wiper: AnyInput<'static> = match CONFIG.wiper_pin {
            "P0_04" => p.P0_04.degrade_saadc(), // A0 / AIN2
            "P0_05" => p.P0_05.degrade_saadc(), // A1 / AIN3
            "P0_28" => p.P0_28.degrade_saadc(), // A2 / AIN4
            "P0_30" => p.P0_30.degrade_saadc(), // A3 / AIN6
            "P0_02" => p.P0_02.degrade_saadc(), // A4 / AIN0
            "P0_03" => p.P0_03.degrade_saadc(), // A5 / AIN1
            other => panic!("cfg.toml wiper_pin '{}' not in whitelist", other),
        };

        let bat_vol: AnyInput<'static> = p.P0_29.degrade_saadc();



        Board {
           wiper,
           reed,
           saadc: p.SAADC,
           twim_pin: p.TWISPI0,
           scl: p.P0_11.into(), // D3
           sda: p.P0_12.into(), // D4
           bat_vol,
        }
    }
}

pub fn cps_ticks_1024(t: embassy_time::Instant) -> u16 {
    ((t.as_ticks() as u128 * 1024 / embassy_time::TICK_HZ as u128) & 0xFFFF) as u16
}


pub struct Crank {
    input: Input<'static>,
}

pub struct Resistance {
    adc: Saadc<'static, 2>,
    calibration: Calibration,
}

impl Board {
    /// Splits the board into the parts each task owns.
    pub fn build(self) -> (Crank, Resistance, Twim<'static>) {
        let input = Input::new(self.reed, Pull::Up);

        let resistance_config = embassy_nrf::saadc::Config::default();
        let ch = ChannelConfig::single_ended(self.wiper);
        // piggy back the battery voltage with the resistance
        let bat_ch = ChannelConfig::single_ended(self.bat_vol);
        let adc = Saadc::new(self.saadc, Irqs, resistance_config, [ch, bat_ch]);

        (
            Crank { input },
            Resistance { adc, calibration: Calibration::DEFAULT },
            Twim::new(self.twim_pin, Irqs, self.sda, self.scl, twim::Config::default(), &mut [])
        )
    }
}

impl Crank {
    pub async fn run(&mut self) -> ! {
        let mut last_edge: Option<Instant> = None;
        let mut rpm = 0u16;
        let mut revs: u16 = 0;

        loop {
            match select(self.input.wait_for_falling_edge(), Timer::after_secs(3)).await {
                Either::First(_) => {
                    let now = Instant::now();
                    if let Some(last) = last_edge {
                        let dt_ms = now.duration_since(last).as_millis() as u16;
                        if dt_ms < CRANK_DEBOUNCE_MS { continue; } // debounce
                        rpm = 60_000 / dt_ms;
                    }
                    last_edge = Some(now);
                    revs = revs.wrapping_add(1);
                    let ticks = cps_ticks_1024(now);

                    let power = ride_get().calc_power(rpm);

                    ride_update(|state| {
                        state.power_w = power;
                        state.crank_revs = revs;
                        state.last_rev_ticks = ticks;
                        state.cadence_rpm = rpm;
                    });
                }
                Either::Second(_) => {
                    if rpm != 0 {
                        defmt::debug!("cadence timeout rpm -> 0");
                    }
                    rpm = 0;

                    ride_update(|state| {
                        state.cadence_rpm = 0;
                        state.power_w = 0;
                    });
                }
            }
        }
    }
}

impl Resistance {
    pub fn positions_pct(counts: i16, cal: &Calibration) -> f32 {
        let pct = (counts as f32 - cal.counts_min) / (cal.counts_max - cal.counts_min) * 100.0;
        pct.clamp(0.0, 100.0)
    }

    pub async fn run(&mut self) -> ! {
        let power_scale = self.calibration.power_scale;
        ride_update(|state| {
            state.power_scale = power_scale;
        });

        let mut ticker = Ticker::every(Duration::from_millis(50));
        let mut buf = [0i16; 2];
        let mut bat_acc: u32 = 0; // filtered battery mV * 16
        loop {
            ticker.next().await;
            self.adc.sample(&mut buf).await;
            let raw_resistance = buf[0];

            let pct = Self::positions_pct(raw_resistance, &self.calibration);

            // smoothed since vbat dips whenever the radio transmits
            let mv = battery_mv(buf[1]) as u32;
            bat_acc = if bat_acc == 0 { mv * 16 } else { bat_acc - bat_acc / 16 + mv };
            let filtered_mv = (bat_acc / 16) as u16;

            ride_update(|state| {
                state.resistance_position_pct = pct;
                state.update_battery(filtered_mv, usb_connected());
            });
        }
    }
}

fn battery_mv(raw: i16) -> u16 {
    (raw.max(0) as u32 * 3600 * 2 / 4096) as u16
}

const USBREGSTATUS_VBUSDETECT: u32 = 1 << 0;

// the softdevice owns POWER so usb status has to go through it, needs the softdevice enabled first
fn usb_connected() -> bool {
    let mut status: u32 = 0;
    unsafe { nrf_softdevice::raw::sd_power_usbregstatus_get(&mut status) };
    status & USBREGSTATUS_VBUSDETECT != 0
}

pub struct Calibration {
    counts_min: f32, // ADC when lever all the way down
    counts_max: f32, // ADC when lever all the way up
    power_scale: f32, // 1.0 default, >1.0 for more power at same resistance, <1.0 for less
}

impl Calibration {
    pub const DEFAULT: Self = Self {
        counts_min: 0.0,
        counts_max: 5755.0,
        power_scale: 1.0,
    };
    pub fn _is_valid(&self) -> bool {
        self.counts_max - self.counts_min > 100.0 
            && self.power_scale > 0.1
            && self.power_scale < 10.0
    }
}