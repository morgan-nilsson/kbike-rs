use embassy_nrf::{Peripherals, saadc, Peri};
use embassy_nrf::gpio::{AnyPin, Input, Pull, Pin};
use embassy_nrf::saadc::{AnyInput, ChannelConfig, Input as SaadcInput, Saadc};
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


        Board {
           wiper,
           reed,
           saadc: p.SAADC,
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
    adc: Saadc<'static, 1>,
    calibration: Calibration,
}

impl Board {
    /// Splits the board into the parts each task owns.
    pub fn build(self) -> (Crank, Resistance) {
        let input = Input::new(self.reed, Pull::Up);

        let resistance_config = embassy_nrf::saadc::Config::default();
        let ch = ChannelConfig::single_ended(self.wiper);
        let adc = Saadc::new(self.saadc, Irqs, resistance_config, [ch]);

        (
            Crank { input },
            Resistance { adc, calibration: Calibration::DEFAULT },
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
                        rpm = (60_000 / dt_ms) as u16;
                    }
                    last_edge = Some(now);
                    revs = revs.wrapping_add(1);
                    let ticks = cps_ticks_1024(now);

                    let power = ride_get().update_power(rpm);

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
        let mut ticker = Ticker::every(Duration::from_millis(50));
        let mut buf = [0i16; 1];
        loop {
            ticker.next().await;
            self.adc.sample(&mut buf).await;
            let raw_resistance = buf[0];

            let pct = Self::positions_pct(raw_resistance, &self.calibration);
            ride_update(|state| {
                state.resistance_position_pct = pct;
            });
        }
    }
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
    pub fn is_valid(&self) -> bool {
        self.counts_max - self.counts_min > 100.0 
            && self.power_scale > 0.1
            && self.power_scale < 10.0
    }
}