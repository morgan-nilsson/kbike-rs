//! Firmware modules compiled for the host so their pure logic can be unit tested.
#[path = "../../src/config.rs"]
pub mod config;

#[path = "../../src/kbike_rs/ride_state.rs"]
pub mod ride_state;

// Unit tests (not under tests/) because `config::str_eq` is crate-private.
#[cfg(test)]
mod config_tests;
