#[toml_cfg::toml_config]
pub struct Config {
    // filters on resistance values
    #[default(true)]
    resistance_filter: bool,

    #[default(false)]
    resistance_display_filter: bool,

    // ===== power save settings =====
    // "disabled" | "enabled" | "enabled_no_shutdown"
    #[default("enabled")]
    power_save_mode: &'static str,

    #[default(60)]
    dim_time_seconds: u32,

    #[default(120)]
    blank_time_seconds: u32,

    #[default(60)]
    no_bluetooth_enter_power_save_time_seconds: u32,

    #[default(600)]
    bluetooth_enter_power_save_time_seconds: u32,

    // ===== Display settings =====
    #[default(true)]
    display_enabled: bool,

    #[default(0xFF)]
    display_contrast_full: u8,

    #[default(0x10)]
    display_contrast_dim: u8,

    // ===== Battery settings =====
    #[default(true)]
    has_battery: bool,

    #[default(true)]
    battery_use_battery_service: bool,

    #[default(1)]
    battery_used_vdd_as_low: u8,

    // flash battery icon threshold, percent
    #[default(15)]
    battery_is_low: u8,

    // mV
    #[default(3250)]
    vdd_min: u16,

    // ===== Resistance =====
    // "gear" | "percent"
    #[default("gear")]
    display_resistance_as: &'static str,

    // "linear" | "uneven" (original keiser spacing)
    #[default("uneven")]
    gear_spacing: &'static str,

    // ===== Bluetooth settings =====
    #[default(true)]
    bluetooth_enabled: bool,

    #[default("kbike-rs")]
    bluetooth_name: &'static str,

    #[default(3)]
    max_bluetooth_connections: u8,

    // "CPS" | "FTMS"
    // toml-cfg pastes the toml array in as an array literal, so the length is fixed
    #[default(["CPS", "FTMS"])]
    bluetooth_provides: [&'static str; 2],

    // dBm: -40, -20, -16, -12, -8, -4, 0, 2, 3, 4, 5, 6, 7, 8
    #[default(-12)]
    bluetooth_transmit_power: i8,

    // milliseconds
    #[default(1000)]
    bluetooth_led_interval: u32,

    // ===== Console =====
    #[default(10)]
    confirmation_timeout_seconds: u32,

    #[default(true)]
    use_serial_console: bool,

    #[default(true)]
    use_serial_bluetooth: bool,

    // ===== Bike interface settings =====
    // saadc sampling time, microseconds
    #[default(20)]
    analog_sample_time: u32,

    #[default("P1_08")]
    reed_pin: &'static str,

    #[default("P0_04")]
    wiper_pin: &'static str,
}

// ===== Verification =====
// Evaluated at compile time
const _: () = verify(&CONFIG);

pub(crate) const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

const fn str_in(s: &str, options: &[&str]) -> bool {
    let mut i = 0;
    while i < options.len() {
        if str_eq(s, options[i]) {
            return true;
        }
        i += 1;
    }
    false
}

const fn verify(c: &Config) {
    assert!(
        str_in(c.power_save_mode, &["disabled", "enabled", "enabled_no_shutdown"]),
        "cfg.toml: power_save_mode must be \"disabled\", \"enabled\" or \"enabled_no_shutdown\""
    );
    assert!(
        c.dim_time_seconds <= c.blank_time_seconds,
        "cfg.toml: dim_time_seconds must be <= blank_time_seconds"
    );
    assert!(
        c.display_contrast_dim <= c.display_contrast_full,
        "cfg.toml: display_contrast_dim must be <= display_contrast_full"
    );

    assert!(c.battery_is_low <= 100, "cfg.toml: battery_is_low is a percent (0-100)");
    assert!(
        c.vdd_min >= 1700 && c.vdd_min <= 3600,
        "cfg.toml: vdd_min must be within the nRF52840 VDD range (1700-3600 mV)"
    );

    assert!(
        str_in(c.display_resistance_as, &["gear", "percent"]),
        "cfg.toml: display_resistance_as must be \"gear\" or \"percent\""
    );
    assert!(
        str_in(c.gear_spacing, &["linear", "uneven"]),
        "cfg.toml: gear_spacing must be \"linear\" or \"uneven\""
    );

    // Advertising data is 31 bytes; leave room for flags and the name header
    assert!(!c.bluetooth_name.is_empty(), "cfg.toml: bluetooth_name must not be empty");
    assert!(c.bluetooth_name.len() <= 26, "cfg.toml: bluetooth_name must be <= 26 bytes");
    assert!(
        c.max_bluetooth_connections >= 1 && c.max_bluetooth_connections <= 20,
        "cfg.toml: max_bluetooth_connections must be 1-20"
    );

    let mut i = 0;
    while i < c.bluetooth_provides.len() {
        assert!(
            str_in(c.bluetooth_provides[i], &["CPS", "FTMS"]),
            "cfg.toml: bluetooth_provides entries must be \"CPS\" or \"FTMS\""
        );
        let mut j = i + 1;
        while j < c.bluetooth_provides.len() {
            assert!(
                !str_eq(c.bluetooth_provides[i], c.bluetooth_provides[j]),
                "cfg.toml: bluetooth_provides has a duplicate entry"
            );
            j += 1;
        }
        i += 1;
    }

    assert!(
        matches!(
            c.bluetooth_transmit_power,
            -40 | -20 | -16 | -12 | -8 | -4 | 0 | 2 | 3 | 4 | 5 | 6 | 7 | 8
        ),
        "cfg.toml: bluetooth_transmit_power must be one of -40, -20, -16, -12, -8, -4, 0, 2, 3, 4, 5, 6, 7, 8"
    );
    assert!(c.bluetooth_led_interval > 0, "cfg.toml: bluetooth_led_interval must be > 0");

    assert!(
        c.confirmation_timeout_seconds > 0,
        "cfg.toml: confirmation_timeout_seconds must be > 0"
    );

    // SAADC TACQ options
    assert!(
        matches!(c.analog_sample_time, 3 | 5 | 10 | 15 | 20 | 40),
        "cfg.toml: analog_sample_time must be one of 3, 5, 10, 15, 20, 40 (us)"
    );
}
