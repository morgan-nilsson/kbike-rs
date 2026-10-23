
use nrf_softdevice::ble::{Connection, gatt_server, peripheral};
use nrf_softdevice::Softdevice;
use embassy_futures::select::select;
use embassy_time::{Duration, Ticker};
use embassy_executor::Spawner;
use crate::config::CONFIG;
use crate::kbike_rs::ride_state::ride_update;
use crate::kbike_rs::{cps::{CyclingPowerService, CyclingPowerServiceEvent, encode_cps_packet}, ride_state::ride_get};
use nrf_softdevice::ble::advertisement_builder::{LegacyAdvertisementBuilder, LegacyAdvertisementPayload, Flag, ServiceUuid16};
use crate::kbike_rs::ftms::{FitnessMachineService, FitnessMachineServiceEvent, encode_ftms_data, handle_control_point_request};
use crate::kbike_rs::{cps::CPS_FEATURES, ftms::FTMS_INDOOR_BIKE_DATA_FLAGS};
use crate::kbike_rs::ftms::encode_supported_power_range;

#[nrf_softdevice::gatt_server]
pub struct Server {
    pub cps: CyclingPowerService,
    pub ftms: FitnessMachineService,
}

impl Server {
    pub fn setup_services(&self) {
        self.cps.feature_set(&CPS_FEATURES.bits().to_le_bytes()).unwrap();
        self.cps.sensor_location_set(&5u8).unwrap();

        self.ftms.fitness_machine_feature_set(&FTMS_INDOOR_BIKE_DATA_FLAGS.bits().to_le_bytes()).unwrap();
        self.ftms.supported_power_range_set(&encode_supported_power_range()).unwrap();
    }
}

#[embassy_executor::task]
pub async fn ble_advertise(sd: &'static Softdevice, server: &'static Server, spawner: Spawner) {

    // Advertise only services in the config
    let services: &[ServiceUuid16] = if CONFIG.bluetooth_provides.contains(&"CPS") && CONFIG.bluetooth_provides.contains(&"FTMS") {
        &[ServiceUuid16::CYCLING_POWER, ServiceUuid16::FITNESS_MACHINE]
    } else if CONFIG.bluetooth_provides.contains(&"CPS") {
        &[ServiceUuid16::CYCLING_POWER]
    } else if CONFIG.bluetooth_provides.contains(&"FTMS") {
        &[ServiceUuid16::FITNESS_MACHINE]
    } else {
        &[]
    };

    let ad_data: LegacyAdvertisementPayload = LegacyAdvertisementBuilder::new()
        .flags(&[Flag::GeneralDiscovery, Flag::LE_Only])
        .services_16(nrf_softdevice::ble::advertisement_builder::ServiceList::Complete, services)
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
                spawner.spawn(ble_task(conn, server).expect("Failed to spawn BLE task"));
            }
            Err(e) => {
                defmt::warn!("Advertising error: {:?}", e);
            }
        }
    }
}

#[embassy_executor::task(pool_size = CONFIG.max_bluetooth_connections as usize)]
async fn ble_task(conn: Connection, server: &'static Server) {
    defmt::info!("Connected");

    let gatt_fut = gatt_server::run(&conn, server, |e| match e {
        ServerEvent::Cps(CyclingPowerServiceEvent::MeasurementCccdWrite { notifications }) => {
            defmt::info!("CPS Measurement CCCD write: notifications={}", notifications);
        }
        ServerEvent::Ftms(FitnessMachineServiceEvent::IndoorBikeDataCccdWrite { notifications }) => {
            defmt::info!("FTMS Indoor Bike Data CCCD write: notifications={}", notifications);
        }

        ServerEvent::Ftms(FitnessMachineServiceEvent::FitnessMachineControlPointWrite ( value )) => {
            defmt::info!("FTMS Control Point Write: {:?}", value);
            let mut resp = heapless::Vec::<u8, 8>::new();
            ride_update(|ride| resp = handle_control_point_request(&value, ride));
            let _ = server.ftms.fitness_machine_control_point_indicate(&conn, &resp);
        }

        ServerEvent::Ftms(FitnessMachineServiceEvent::FitnessMachineControlPointCccdWrite { indications  }) => {}

        ServerEvent::Ftms(FitnessMachineServiceEvent::FitnessMachineControlPointResponseCccdWrite { .. }) => {}
    });

    let notify_fut = async {
        let mut ticker = Ticker::every(Duration::from_secs(1));
        loop {
            ticker.next().await;
            let s = ride_get();
            let cps_pkt = encode_cps_packet(s.power_w, s.crank_revs, s.last_rev_ticks);
            let _ = server.cps.measurement_notify(&conn, &cps_pkt);

            let ftms_pkt = encode_ftms_data(&s);
            let _ = server.ftms.indoor_bike_data_notify(&conn, &ftms_pkt);
        }
    };

    select(gatt_fut, notify_fut).await;
    defmt::info!("Connection closed");

}