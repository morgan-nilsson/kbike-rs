use nrf_softdevice::ble::gatt_server;

#[nrf_softdevice::gatt_service(uuid = "1818")]
pub struct CyclingPowerService {
    #[characteristic(uuid = "2A63", read, notify)]
    pub measurement: [u8; 8],
    #[characteristic(uuid = "2A65", read)]
    pub feature: [u8; 2],
    #[characteristic(uuid = "2A5D", read)]
    pub sensor_location: u8,
}

#[nrf_softdevice::gatt_server]
pub struct CpsServer {
    pub cps: CyclingPowerService,
}

pub const CPS_FEATURES: CPSFeatureCharacteristics = CPSFeatureCharacteristics::Crank_Revolution_Data;

bitflags::bitflags! {

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CPSMeasurementFlags: u16 {
        const _PEDAL_POWER_BALANCE_PRESENT = 1 << 0;
        const _PEDAL_POWER_BALANCE_REFERENCE = 1 << 1;
        const _ACCUMULATED_TORQUE_PRESENT = 1 << 2;
        const _ACCUMULATED_TORQUE_SOURCE = 1 << 3;
        const _WHEEL_REVOLUTION_DATA_PRESENT = 1 << 4;
        const CRANK_REVOLUTION_DATA_PRESENT = 1 << 5;
        const _EXTREME_FORCE_MAGNITUDES_PRESENT = 1 << 6;
        const _EXTREME_TORQUE_MAGNITUDES_PRESENT = 1 << 7;
        const _EXTREME_ANGLES_PRESENT = 1 << 8;
        const _TOP_DEAD_SPOT_ANGLE_PRESENT = 1 << 9;
        const _BOTTOM_DEAD_SPOT_ANGLE_PRESENT = 1 << 10;
        const _ACCUMULATED_ENERGY_PRESENT = 1 << 11;
        const _OFFSET_COMPENSATION_INDICATOR = 1 << 12;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CPSFeatureCharacteristics: u16 {
        const _Pedal_POWER_BALANCE = 1 << 0;
        const _Accumulated_Torque = 1 << 1;
        const _Wheel_Revolution_Data = 1 << 2;
        const Crank_Revolution_Data = 1 << 3;
        const _Extreme_Magnitudes = 1 << 4;
        const _Extreme_Angles = 1 << 5;
        const _TOP_DEAD_SPOT_ANGLE = 1 << 6;
        const _BOTTOM_DEAD_SPOT_ANGLE = 1 << 7;
        const _ACCUMULATED_ENERGY = 1 << 8;
        // reserved 9-15
    }
}

pub fn encode_cps_packet(power_w: i16, crank_revs: u16, last_rev_ticks: u16) -> [u8; 8] {
    let mut packet = [0u8; 8];
    let flags = CPSMeasurementFlags::CRANK_REVOLUTION_DATA_PRESENT.bits();
    packet[0..2].copy_from_slice(&flags.to_le_bytes());
    packet[2..4].copy_from_slice(&power_w.to_le_bytes());
    packet[4..6].copy_from_slice(&crank_revs.to_le_bytes());
    packet[6..8].copy_from_slice(&last_rev_ticks.to_le_bytes());
    packet
}

pub struct CpsOptions {
    pub enabled: bool,
    pub sensor_location: u8,       // the only real knob; 5 = left crank
}