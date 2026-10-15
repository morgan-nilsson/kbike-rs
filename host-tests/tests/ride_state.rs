//! Tests for the power model in `src/kbike_rs/ride_state.rs`.
use kbike_host_tests::ride_state::{ride_get, ride_update, RideState};

// Reference copies of the firmware tables (gear position %, power at 90 rpm in W).
const UNEVEN_GEARS: [f32; 25] = [
    0.00, 7.51, 11.40, 14.59, 18.57, 20.12, 23.32, 24.87, 27.03, 29.53, 31.87, 34.20, 36.70,
    39.72, 42.83, 46.03, 49.14, 53.11, 56.99, 60.97, 66.49, 72.02, 78.41, 89.38, 100.00,
];
const UNEVEN_POWER90: [f32; 25] = [
    20.9, 24.3, 27.7, 33.5, 40.4, 45.0, 54.3, 61.3, 71.7, 84.4, 98.3, 118.0, 136.6, 163.3, 193.4,
    225.9, 261.9, 308.3, 360.5, 415.0, 497.4, 577.5, 671.5, 796.8, 878.0,
];

/// Cadence correction factor from the firmware's quadratic model.
fn cadence_factor(rpm: u16) -> f32 {
    let r = rpm as f32;
    -2.70E-03 + 6.79E-03 * r + 4.75E-05 * r * r
}

/// Expected watts: straight-line interpolation of the power table, scaled for cadence.
fn expected_power(gears: &[f32; 25], power90: &[f32; 25], resistance: f32, rpm: u16) -> f32 {
    let i = gears.iter().position(|&g| g >= resistance).unwrap_or(24).max(1);
    let t = (resistance - gears[i - 1]) / (gears[i] - gears[i - 1]);
    let p90 = power90[i - 1] + t * (power90[i] - power90[i - 1]);
    (p90 * cadence_factor(rpm)).max(0.0)
}

/// Watts from the firmware at a given lever position and cadence.
/// Uses the gear spacing from cfg.toml ("uneven" by default).
fn power(resistance: f32, rpm: u16) -> i16 {
    let mut s = RideState::new();
    s.resistance_position_pct = resistance;
    s.calc_power(rpm)
}

// ---- cadence --------------------------------------------------------------

#[test]
fn zero_rpm_gives_zero_power() {
    for r in [0.0, 25.0, 50.0, 85.0] {
        assert_eq!(power(r, 0), 0, "resistance {r}");
    }
}

#[test]
fn power_is_never_negative_at_low_cadence() {
    for rpm in 0..=5 {
        for r in [0.0, 10.0, 50.0, 85.0] {
            assert!(power(r, rpm) >= 0, "rpm {rpm}, resistance {r}");
        }
    }
}

#[test]
fn power_rises_with_cadence() {
    let mut last = 0;
    for rpm in (10..=150).step_by(10) {
        let p = power(40.0, rpm);
        assert!(p > last, "power did not rise at {rpm} rpm: {p} <= {last}");
        last = p;
    }
}

// ---- table points ---------------------------------------------------------

#[test]
fn power_at_table_points_matches_table() {
    // Index 24 (100 %) is covered by `top_segment_power_is_correct`.
    for i in 0..24 {
        let got = power(UNEVEN_GEARS[i], 90);
        let want = (UNEVEN_POWER90[i] * cadence_factor(90)).round() as i16;
        assert_eq!(got, want, "table point {i} ({} %)", UNEVEN_GEARS[i]);
    }
}

#[test]
fn power_is_rounded_to_nearest_watt() {
    // 98.3 W * 0.99315 = 97.63 W -> 98
    assert_eq!(power(UNEVEN_GEARS[10], 90), 98);
}

// ---- power scale ----------------------------------------------------------

/// Watts with a calibration scale applied.
fn scaled_power(resistance: f32, rpm: u16, scale: f32) -> i16 {
    let mut s = RideState::new();
    s.resistance_position_pct = resistance;
    s.power_scale = scale;
    s.calc_power(rpm)
}

#[test]
fn power_scale_defaults_to_one() {
    assert_eq!(RideState::new().power_scale, 1.0);
}

#[test]
fn power_scale_of_one_changes_nothing() {
    for r in [0.0, 31.87, 60.0, 95.0] {
        assert_eq!(scaled_power(r, 90, 1.0), power(r, 90), "{r} %");
    }
}

#[test]
fn power_scale_multiplies_power() {
    // 98.3 W * 0.99315 = 97.63 W; scaled before rounding
    assert_eq!(scaled_power(UNEVEN_GEARS[10], 90, 1.1), 107); // 107.39
    assert_eq!(scaled_power(UNEVEN_GEARS[10], 90, 0.5), 49); // 48.81
}

#[test]
fn scaled_power_is_never_negative_at_low_cadence() {
    for scale in [0.5, 1.0, 2.0] {
        assert_eq!(scaled_power(50.0, 0, scale), 0, "scale {scale}");
    }
}

// ---- shared ride state ----------------------------------------------------

#[test]
fn ride_update_is_visible_to_ride_get() {
    ride_update(|s| {
        s.cadence_rpm = 87;
        s.crank_revs = 1234;
    });
    let s = ride_get();
    assert_eq!(s.cadence_rpm, 87);
    assert_eq!(s.crank_revs, 1234);
}

// ---- interpolation --------------------------------------------------------

#[test]
fn top_segment_power_is_correct() {
    for r in [89.5, 95.0, 100.0] {
        let got = power(r, 90) as f32;
        let want = expected_power(&UNEVEN_GEARS, &UNEVEN_POWER90, r, 90);
        assert!((got - want).abs() <= 1.0, "{r} %: got {got} W, want {want:.1} W");
    }
}

#[test]
fn power_between_table_points_interpolates() {
    // Stops below the top segment, which `top_segment_power_is_correct` covers.
    for i in 1..24 {
        let mid = (UNEVEN_GEARS[i - 1] + UNEVEN_GEARS[i]) / 2.0;
        let got = power(mid, 90) as f32;
        let want = expected_power(&UNEVEN_GEARS, &UNEVEN_POWER90, mid, 90);
        // Slope tables are rounded, so allow 1 W either side.
        assert!(
            (got - want).abs() <= 1.0,
            "midpoint of segment {i} ({mid} %): got {got} W, want {want:.1} W"
        );
    }
}

#[test]
fn power_never_decreases_as_resistance_rises() {
    let mut last = 0;
    // Stops below the top segment, which `top_segment_power_is_correct` covers.
    for tenth in 0..890 {
        let r = tenth as f32 / 10.0;
        let p = power(r, 90);
        assert!(p >= last, "power dropped from {last} to {p} W at {r} %");
        last = p;
    }
}

