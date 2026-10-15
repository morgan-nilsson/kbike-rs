use core::cell::Cell;
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use crate::config::{str_eq, CONFIG};

type GearTables = (&'static [f32; 25], &'static [f32; 25], &'static [f32; 25]);

const fn gear_tables(spacing: &str) -> GearTables {
    if str_eq(spacing, "linear") {
        (&EVEN_GEARS, &EVEN_POWER90, &EVEN_PWR_SLOPES)
    } else if str_eq(spacing, "uneven") {
        (&UNEVEN_GEARS, &UNEVEN_POWER90, &UNEVEN_PWR_SLOPE)
    } else {
        panic!("Invalid gear spacing configuration")
    }
}

const PC2: f32 = -2.70E-03;
const PB2: f32 = 6.79E-03;
const PA2: f32 = 4.75E-05;

#[derive(Debug, Clone, Copy, PartialEq, defmt::Format)]
pub struct RideState {
    pub power_w: i16,
    pub crank_revs: u16,
    pub last_rev_ticks: u16,
    pub cadence_rpm: u16,
    pub resistance_position_pct: f32,

    gears_table: &'static [f32; 25],
    power_table: &'static [f32; 25],
    slope_table: &'static [f32; 25],
}

impl RideState {
    pub const fn new() -> Self {

        let (gears_table, power_table, slope_table) = gear_tables(CONFIG.gear_spacing);

        Self {
            power_w: 0,
            crank_revs: 0,
            last_rev_ticks: 0,
            cadence_rpm: 0,
            resistance_position_pct: 0.00,

            gears_table,
            power_table,
            slope_table,
        }
    }

    pub fn update_gear_spacing(&mut self, gear_spacing: &'static str) {
        let (gears_table, power_table, slope_table) = gear_tables(gear_spacing);

        self.gears_table = gears_table;
        self.power_table = power_table;
        self.slope_table = slope_table;
    }

    pub fn gear_from_resistance(&self, resistance: f32) -> u8 {
        let mut gear = self.gears_table.len() as u8; // default to max gear
        for (i, &g) in self.gears_table.iter().enumerate() {
            if g >= resistance as f32 {
                gear = i as u8 + 1;
                break;
            }
        }
        gear.min(24)
    }

    pub fn update_power(&self, rpm: u16) -> i16 {
        let mut i = self.gears_table.len() - 1; // default to top segment
        for (j, &g) in self.gears_table.iter().enumerate() {
            if g >= self.resistance_position_pct {
                i = j;
                break;
            }
        }
        let p90 = self.power_table[i]
            + (self.resistance_position_pct - self.gears_table[i])
            * self.slope_table[i];

        let c = PC2 
            + PB2 * (rpm as f32) 
            + PA2 * (rpm as f32) * (rpm as f32);

        // p >= 0
        let p = (p90 * c).max(0.0);
        // half add for rounding
        return (p + 0.5) as i16;
    }

}

pub static RIDE: Mutex<CriticalSectionRawMutex, Cell<RideState>>
    = Mutex::new(Cell::new(RideState::new()));

pub fn ride_update(f: impl FnOnce(&mut RideState)) {
    RIDE.lock(|cell| {
        let mut state = cell.get();
        f(&mut state);
        cell.set(state);
    });
    defmt::debug!("Ride state updated: {:?}", ride_get());
}

pub fn ride_get() -> RideState {
    RIDE.lock(|cell| cell.get())
}

const UNEVEN_GEARS: [f32; 25] = [  0.00,  7.51, 11.40, 14.59, 18.57, 20.12, 23.32, 24.87, 
    27.03, 29.53, 31.87, 34.20, 36.70, 39.72, 42.83, 46.03, 
    49.14, 53.11, 56.99, 60.97, 66.49, 72.02, 78.41, 89.38, 100.00 ];
    
const UNEVEN_POWER90: [f32; 25] = [ 20.9,  24.3,  27.7,  33.5,  40.4,  45.0,  54.3,  61.3,  
      71.7,  84.4,  98.3, 118.0, 136.6, 163.3, 193.4, 225.9, 
     261.9, 308.3, 360.5, 415.0, 497.4, 577.5, 671.5, 796.8, 878.0 ];

const UNEVEN_PWR_SLOPE: [f32; 25] =   [    0.0,  0.446,  0.884,  1.805,  1.742,  2.976,  2.896,  4.471,
         4.830,  5.089,  5.965,  8.456,  7.408,  8.826,  9.701, 10.166,
        11.569, 11.683, 13.437, 13.730, 14.908, 14.488, 14.709, 11.425,  7.642 ];

const EVEN_GEARS: [f32; 25] = [ 0.00, 10.10, 12.25, 14.46, 16.72, 19.03, 21.41, 23.85,
   26.36, 28.95, 31.63, 34.40, 37.28, 40.27, 43.41, 46.69,
   50.16, 53.84, 57.78, 62.05, 66.74, 72.00, 78.13, 85.77, 100.0];

const EVEN_POWER90: [f32; 25] = [  22.40,  25.23,  27.55,  30.84,  35.31,  41.16,  48.59,  57.86,
       69.20,  82.90,  99.25, 118.56, 141.20, 167.55, 198.05, 233.17,
      273.45, 319.52, 372.07, 431.91, 499.98, 577.34, 665.11, 763.85, 877.84 ];

const EVEN_PWR_SLOPES: [f32; 25] = [  0.00,  0.28,  1.07,  1.49,  1.98,  2.52,  3.13,  3.80,
      4.52,  5.29,  6.11,  6.97,  7.87,  8.79,  9.74, 10.69,
     11.62, 12.51, 13.33, 14.03, 14.52, 14.70, 14.32, 12.92, 8.01 ];