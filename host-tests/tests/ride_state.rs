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
const EVEN_GEARS: [f32; 25] = [
    0.00, 10.10, 12.25, 14.46, 16.72, 19.03, 21.41, 23.85, 26.36, 28.95, 31.63, 34.40, 37.28,
    40.27, 43.41, 46.69, 50.16, 53.84, 57.78, 62.05, 66.74, 72.00, 78.13, 85.77, 100.0,
];
const EVEN_POWER90: [f32; 25] = [
    22.40, 25.23, 27.55, 30.84, 35.31, 41.16, 48.59, 57.86, 69.20, 82.90, 99.25, 118.56, 141.20,
    167.55, 198.05, 233.17, 273.45, 319.52, 372.07, 431.91, 499.98, 577.34, 665.11, 763.85,
    877.84,
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

fn state(spacing: &'static str, resistance: f32) -> RideState {
    let mut s = RideState::new();
    s.update_gear_spacing(spacing);
    s.resistance_position_pct = resistance;
    s
}

fn power(spacing: &'static str, resistance: f32, rpm: u16) -> i16 {
    state(spacing, resistance).update_power(rpm)
}

// ---- cadence --------------------------------------------------------------

#[test]
fn zero_rpm_gives_zero_power() {
    for r in [0.0, 25.0, 50.0, 85.0] {
        assert_eq!(power("uneven", r, 0), 0, "resistance {r}");
    }
}

#[test]
fn power_is_never_negative_at_low_cadence() {
    for rpm in 0..=5 {
        for r in [0.0, 10.0, 50.0, 85.0] {
            assert!(power("uneven", r, rpm) >= 0, "rpm {rpm}, resistance {r}");
        }
    }
}

#[test]
fn power_rises_with_cadence() {
    let mut last = 0;
    for rpm in (10..=150).step_by(10) {
        let p = power("uneven", 40.0, rpm);
        assert!(p > last, "power did not rise at {rpm} rpm: {p} <= {last}");
        last = p;
    }
}

// ---- table points ---------------------------------------------------------

#[test]
fn power_at_table_points_matches_table_uneven() {
    // Index 24 (100 %) is covered by `top_segment_power_is_correct`.
    for i in 0..24 {
        let got = power("uneven", UNEVEN_GEARS[i], 90);
        let want = (UNEVEN_POWER90[i] * cadence_factor(90)).round() as i16;
        assert_eq!(got, want, "uneven table point {i} ({} %)", UNEVEN_GEARS[i]);
    }
}

#[test]
fn power_at_table_points_matches_table_linear() {
    for i in 0..24 {
        let got = power("linear", EVEN_GEARS[i], 90);
        let want = (EVEN_POWER90[i] * cadence_factor(90)).round() as i16;
        assert_eq!(got, want, "linear table point {i} ({} %)", EVEN_GEARS[i]);
    }
}

#[test]
fn power_is_rounded_to_nearest_watt() {
    // 98.3 W * 0.99315 = 97.63 W -> 98
    assert_eq!(power("uneven", UNEVEN_GEARS[10], 90), 98);
}

#[test]
fn gear_spacing_selects_table() {
    // Bottom of each table at 90 rpm: 20.9 W (uneven) vs 22.4 W (linear)
    assert_eq!(power("uneven", 0.0, 90), 21);
    assert_eq!(power("linear", 0.0, 90), 22);
}

// ---- gear lookup ----------------------------------------------------------

#[test]
fn gear_is_one_at_zero_resistance() {
    assert_eq!(state("uneven", 0.0).gear_from_resistance(0.0), 1);
}

#[test]
fn gear_is_top_at_full_resistance() {
    // The M3 has 24 gears, so the top table segment shows as gear 24.
    assert_eq!(state("uneven", 0.0).gear_from_resistance(100.0), 24);
}

#[test]
fn gear_never_decreases_as_resistance_rises() {
    let s = state("uneven", 0.0);
    let mut last = 0;
    for tenth in 0..=1000 {
        let g = s.gear_from_resistance(tenth as f32 / 10.0);
        assert!(g >= last, "gear dropped to {g} at {} %", tenth as f32 / 10.0);
        assert!((1..=24).contains(&g));
        last = g;
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

// ---- known bugs -----------------------------------------------------------

#[test]
fn top_segment_power_is_correct() {
    for r in [89.5, 95.0, 100.0] {
        let got = power("uneven", r, 90) as f32;
        let want = expected_power(&UNEVEN_GEARS, &UNEVEN_POWER90, r, 90);
        assert!((got - want).abs() <= 1.0, "{r} %: got {got} W, want {want:.1} W");
    }
}

#[test]
fn power_between_table_points_interpolates() {
    for (spacing, gears, power90) in [
        ("uneven", &UNEVEN_GEARS, &UNEVEN_POWER90),
        ("linear", &EVEN_GEARS, &EVEN_POWER90),
    ] {
        // Stops below the top segment, which `top_segment_power_is_correct` covers.
        for i in 1..24 {
            let mid = (gears[i - 1] + gears[i]) / 2.0;
            let got = power(spacing, mid, 90) as f32;
            let want = expected_power(gears, power90, mid, 90);
            // Slope tables are rounded, so allow 1 W either side.
            assert!(
                (got - want).abs() <= 1.0,
                "{spacing} midpoint of segment {i} ({mid} %): got {got} W, want {want:.1} W"
            );
        }
    }
}

#[test]
fn power_never_decreases_as_resistance_rises() {
    let mut last = 0;
    // Stops below the top segment, which `top_segment_power_is_correct` covers.
    for tenth in 0..890 {
        let r = tenth as f32 / 10.0;
        let p = power("uneven", r, 90);
        assert!(p >= last, "power dropped from {last} to {p} W at {r} %");
        last = p;
    }
}

#[test]
fn gear_above_full_scale_stays_at_top() {
    assert_eq!(state("uneven", 0.0).gear_from_resistance(100.5), 24);
}
