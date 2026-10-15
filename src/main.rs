#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_futures::select::select;
use embassy_time::{Duration, Ticker};
use static_cell::StaticCell;
use crate::{config::CONFIG, kbike_rs::{bike_interface::Board, ride_state::ride_get}};

use {defmt_rtt as _, panic_probe as _};

use embassy_nrf::{bind_interrupts, interrupt::{self, InterruptExt}, saadc};
use nrf_softdevice::{Softdevice, ble::{Connection, advertisement_builder::{Flag, LegacyAdvertisementBuilder, LegacyAdvertisementPayload, ServiceUuid16}, gatt_server::self, peripheral}, raw};
use embassy_nrf::interrupt::Priority;

//mod config;
mod kbike_rs;
use kbike_rs::bike_interface::{Crank, Resistance};
use kbike_rs::cps::{ CpsServer, CPS_FEATURES, encode_cps_packet, CpsServerEvent, CyclingPowerServiceEvent};

pub mod config;

bind_interrupts!(struct Irqs {
    SAADC => saadc::InterruptHandler;
});


#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice) -> ! {
    sd.run().await
}

#[embassy_executor::task]
async fn ble_advertise(sd: &'static Softdevice, server: &'static CpsServer, spawner: Spawner) {
    let ad_data: LegacyAdvertisementPayload = LegacyAdvertisementBuilder::new()
        .flags(&[Flag::GeneralDiscovery, Flag::LE_Only])
        .services_16(nrf_softdevice::ble::advertisement_builder::ServiceList::Complete, &[ServiceUuid16::CYCLING_POWER])
        .full_name(CONFIG.bluetooth_name)
        .build();

    let scan_data: LegacyAdvertisementPayload = LegacyAdvertisementBuilder::new().build();

    loop {
        let adv = peripheral::ConnectableAdvertisement::ScannableUndirected { 
            adv_data: &ad_data, 
            scan_data: &scan_data,
        };

        let config = peripheral::Config::default();

        match peripheral::advertise_connectable(sd, adv, &config).await{
            Ok(conn) => {
                spawner.spawn(ble_task(sd, conn, server).expect("Failed to spawn BLE task"));
            }
            Err(e) => {
                defmt::warn!("Advertising error: {:?}", e);
            }
        }
    }
}

#[embassy_executor::task(pool_size = CONFIG.max_bluetooth_connections as usize)]
async fn ble_task(sd: &'static Softdevice, conn: Connection, server: &'static CpsServer) {
    defmt::info!("Connected");

    let gatt_fut = gatt_server::run(&conn, server, |e| match e {
        CpsServerEvent::Cps(CyclingPowerServiceEvent::MeasurementCccdWrite { notifications }) => {
            defmt::info!("CPS Measurement CCCD write: notifications={}", notifications);
        }
    });

    let notify_fut = async {
        let mut ticker = Ticker::every(Duration::from_secs(1));
        loop {
            ticker.next().await;
            let s = ride_get();
            let pkt = encode_cps_packet(s.power_w, s.crank_revs, s.last_rev_ticks);
            let _ = server.cps.measurement_notify(&conn, &pkt);

        }
    };

    select(gatt_fut, notify_fut).await;
    defmt::info!("Connection closed");

}

#[embassy_executor::task]
async fn reed_task(mut crank: Crank) -> ! {
    crank.run().await
}

#[embassy_executor::task]
async fn wiper_task(mut resistance: Resistance) -> ! {
    resistance.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    defmt::info!("Hello World");

    let mut config = embassy_nrf::config::Config::default();
    config.gpiote_interrupt_priority = Priority::P2;
    config.time_interrupt_priority = Priority::P2;

    let peripherals = embassy_nrf::init(config);
    let board = Board::split(peripherals);

    let sd_config = nrf_softdevice::Config {
        clock: Some(raw::nrf_clock_lf_cfg_t {
            source: raw::NRF_CLOCK_LF_SRC_RC as u8,   // internal RC — always works
            rc_ctiv: 16,
            rc_temp_ctiv: 2,
            accuracy: raw::NRF_CLOCK_LF_ACCURACY_500_PPM as u8,
        }),
        conn_gatt: Some(raw::ble_gatt_conn_cfg_t { att_mtu: 256 }),
        gatts_attr_tab_size: Some(raw::ble_gatts_cfg_attr_tab_size_t {
            attr_tab_size: raw::BLE_GATTS_ATTR_TAB_SIZE_DEFAULT,
        }),
        gap_role_count: Some(raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: CONFIG.max_bluetooth_connections,
            central_role_count: 0,
            central_sec_count: 0,
            _bitfield_1: raw::ble_gap_cfg_role_count_t::new_bitfield_1(0),
        }),
        gap_device_name: Some(raw::ble_gap_cfg_device_name_t {
            p_value: CONFIG.bluetooth_name.as_ptr() as *mut u8,
            current_len: CONFIG.bluetooth_name.len() as u16,
            max_len: CONFIG.bluetooth_name.len() as u16,
            write_perm: unsafe { core::mem::zeroed() }, // No write permission
            _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                raw::BLE_GATTS_VLOC_STACK as u8,
            )
        }),
        conn_gap: Some(raw::ble_gap_conn_cfg_t {
            conn_count: CONFIG.max_bluetooth_connections,
            event_length: 24,
        }),
        ..Default::default()
    };

    defmt::info!("Softdevice enabled");

    let sd = Softdevice::enable(&sd_config);

    static SERVER: StaticCell<CpsServer> = StaticCell::new();
    let cps_server = SERVER.init(CpsServer::new(sd).unwrap());
    cps_server.cps.feature_set(&CPS_FEATURES.bits().to_le_bytes()).unwrap();
    cps_server.cps.sensor_location_set(&5u8).unwrap();

    let sd: &'static Softdevice = sd;
    spawner.spawn(softdevice_task(sd).expect("Failed to spawn softdevice task"));

    interrupt::SAADC.set_priority(Priority::P3);

    let (crank, resistance) = board.build();

    // Spawn the tasks for the crank and resistance
    spawner.spawn(reed_task(crank).expect("Failed to spawn reed task"));
    spawner.spawn(wiper_task(resistance).expect("Failed to spawn wiper task"));

    // Spawn the BLE task
    spawner.spawn(ble_advertise(sd, cps_server, spawner).expect("Failed to spawn BLE task"));
}