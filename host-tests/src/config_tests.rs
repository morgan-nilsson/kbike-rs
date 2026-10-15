//! Tests for `src/config.rs`.
use crate::config::{str_eq, CONFIG};

#[test]
fn str_eq_matches_equal_strings() {
    assert!(str_eq("uneven", "uneven"));
    assert!(str_eq("", ""));
}

#[test]
fn str_eq_rejects_different_strings() {
    assert!(!str_eq("uneven", "linear"));
    assert!(!str_eq("CPS", "cps"));
    assert!(!str_eq("CPS", "CPSX"));
    assert!(!str_eq("CPSX", "CPS"));
    assert!(!str_eq("", "a"));
}

#[test]
fn defaults_match_shipped_cfg_toml() {
    assert_eq!(CONFIG.gear_spacing, "uneven");
    assert_eq!(CONFIG.bluetooth_name, "kbike-rs");
    assert_eq!(CONFIG.max_bluetooth_connections, 3);
    assert_eq!(CONFIG.reed_pin, "P1_08");
    assert_eq!(CONFIG.wiper_pin, "P0_04");
}
