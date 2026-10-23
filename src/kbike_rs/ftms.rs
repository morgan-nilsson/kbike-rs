use crate::kbike_rs::ride_state::RideState;

#[nrf_softdevice::gatt_service(uuid = "1826")]
pub struct FitnessMachineService {
    #[characteristic(uuid = "2ACC", read)]
    pub fitness_machine_feature: [u8; 2],
    #[characteristic(uuid = "2AD2", notify)]
    pub indoor_bike_data: [u8; 8],
    #[characteristic(uuid = "2AD9", write, indicate)]
    pub fitness_machine_control_point: heapless::Vec<u8, 8>,
    #[characteristic(uuid = "2ADA", notify)]
    pub fitness_machine_control_point_response: [u8; 4],
    #[characteristic(uuid = "2AD8", read)]
    pub supported_power_range: [u8; 6],
}

const MIN_POWER_W: i16 = 0;
const MAX_POWER_W: i16 = 1000;
const INC_POWER_W: i16 = 1;

pub fn encode_supported_power_range() -> [u8; 6] {
    let mut buf = [0u8; 6];
    buf[0..2].copy_from_slice(&MIN_POWER_W.to_le_bytes());
    buf[2..4].copy_from_slice(&MAX_POWER_W.to_le_bytes());
    buf[4..6].copy_from_slice(&INC_POWER_W.to_le_bytes());
    buf
}

pub const FTMS_FEATURES: FitnessMachineFeatures = FitnessMachineFeatures::Cadence
    .union(FitnessMachineFeatures::POWER_MEASUREMENT);

pub const FTMS_TARGETS: TargetSettingFeatures = TargetSettingFeatures::empty();

pub const FTMS_INDOOR_BIKE_DATA_FLAGS: IndoorBikeDataFlags = IndoorBikeDataFlags::MORE_DATA
    .union(IndoorBikeDataFlags::INSTANTANEOUS_CADENCE)
    .union(IndoorBikeDataFlags::RESISTANCE_LEVEL)
    .union(IndoorBikeDataFlags::INSTANTANEOUS_POWER);

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FitnessMachineFeatures: u32 {
        const _Average_SPEED = 1 << 0;
        const Cadence = 1 << 1;
        const _TOTAL_DISTANCE = 1 << 2;
        const _INCLINATION = 1 << 3;
        const _ELEVATION_GAIN = 1 << 4;
        const _PACE = 1 << 5;
        const _STEP_COUNT = 1 << 6;
        const RESISTANCE_LEVEL = 1 << 7;
        const _STRIDE_COUNT = 1 << 8;
        const _EXPENDED_ENERGY = 1 << 9;
        const _HEART_RATE_MEASUREMENT = 1 << 10;
        const _METABOLIC_EQUIVALENT = 1 << 11;
        const _ELAPSED_TIME = 1 << 12;
        const _REMAINING_TIME = 1 << 13;
        const POWER_MEASUREMENT = 1 << 14;
        const _FORCE_ON_BELT_AND_POWER_OUTPUT = 1 << 15;
        const _USER_DATA_RETENTION = 1 << 16;
        // reserved 17-31
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TargetSettingFeatures: u32 {
        const _SPEED_TARGET = 1 << 0;
        const _INCLINATION_TARGET = 1 << 1;
        const _RESISTANCE_TARGET = 1 << 2;
        const _POWER_TARGET = 1 << 3;
        const _HEART_RATE_TARGET = 1 << 4;
        const _TARGETED_EXPENDED_ENERGY = 1 << 5;
        const _TARGETED_STEP_NUMBER = 1 << 6;
        const _TARGETED_STRIDE_NUMBER = 1 << 7;
        const _TARGETED_DISTANCE = 1 << 8;
        const _TARGETED_TRAINING_TIME = 1 << 9;
        const _TARGETED_TIME_IN_TWO_HR_ZONES = 1 << 10;
        const _TARGETED_TIME_IN_THREE_HR_ZONES = 1 << 11;
        const _TARGETED_TIME_IN_FIVE_HR_ZONES = 1 << 12;
        const _INDOOR_BIKE_SIMULATION = 1 << 13;
        const _WHEEL_CIRCUMFERENCE = 1 << 14;
        const _SPIN_DOWN_CONTROL = 1 << 15;
        const _TARGETED_CADENCE = 1 << 16;
        // reserved 17-31
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct IndoorBikeDataFlags: u16 {
        const MORE_DATA = 1 << 0;              // inverted: 0 = instantaneous speed u16 (0.01 km/h) present
        const _AVERAGE_SPEED = 1 << 1;          // u16, 0.01 km/h
        const INSTANTANEOUS_CADENCE = 1 << 2;   // u16, 0.5 rpm
        const _AVERAGE_CADENCE = 1 << 3;        // u16, 0.5 rpm
        const _TOTAL_DISTANCE = 1 << 4;         // u24, m
        const RESISTANCE_LEVEL = 1 << 5;        // s16, unitless
        const INSTANTANEOUS_POWER = 1 << 6;     // s16, W
        const _AVERAGE_POWER = 1 << 7;          // s16, W
        const _EXPENDED_ENERGY = 1 << 8;        // u16 total kcal, u16 kcal/h, u8 kcal/min
        const _HEART_RATE = 1 << 9;             // u8, bpm
        const _METABOLIC_EQUIVALENT = 1 << 10;  // u8, 0.1
        const _ELAPSED_TIME = 1 << 11;          // u16, s
        const _REMAINING_TIME = 1 << 12;        // u16, s
        // reserved 13-15
    }
}

pub fn encode_ftms_data(ride: &RideState) -> [u8; 8] {
    let mut buf = [0u8; 8];
    buf[0..2].copy_from_slice(&FTMS_INDOOR_BIKE_DATA_FLAGS.bits().to_le_bytes());
    buf[2..4].copy_from_slice(&(ride.cadence_rpm * 2).to_le_bytes());
    buf[4..6].copy_from_slice(&(ride.calc_gear() as i16).to_le_bytes());
    buf[6..8].copy_from_slice(&ride.power_w.to_le_bytes());
    buf
}

pub fn handle_control_point_request(request: &[u8], ride: &mut RideState) -> heapless::Vec<u8, 8> {
    let mut resp = heapless::Vec::<u8, 8>::new();
    let (op, result) = match request.first() {
        Some(&0x00) => { // take control
            ride.is_controlled = true;
            (0x00, 0x01) // success
        }
        Some(&0x01) => { // release control
            ride.is_controlled = false;
            ride.requested_gear = None;
            ride.target_power_w = None;
            (0x01, 0x01) // success
        }
        Some(0x04) => { // set target resistance
            let target_resistance = request.get(1).copied().unwrap_or(0);
            ride.requested_gear = Some(((target_resistance as u16 + 5) / 10).clamp(1, 24) as u8);
            ride.target_power_w = None;

            (0x04, 0x01) // success
        }
        Some(0x05) => { // set target power, sint16 watts
            match request.get(1..3) {
                Some(b) => {
                    ride.target_power_w = Some(i16::from_le_bytes([b[0], b[1]]));
                    ride.requested_gear = None;
                    (0x05, 0x01) // success
                }
                None => (0x05, 0x03), // invalid parameter
            }
        }
        Some(0x11) => { // simulation
            // not supported
            (0x11, 0x02)           
        }
        Some(0x07) => { // Start
            (0x07, 0x01) // success
        }
        Some(0x08) => { // Stop
            (0x08, 0x01) // success
        }
        Some(&op) => (op, 0x02), // unknown op code
        None =>(0x00, 0x03)
    };
    defmt::info!("FTMS Control Point Request: op=0x{:02X}, result=0x{:02X}", op, result);
    resp.extend_from_slice(&[0x80, op, result]).unwrap();
    resp
}