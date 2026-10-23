use crate::kbike_rs::ride_state::{ RideState, ride_get };
use embassy_executor::task;
use heapless::String;
use core::fmt::Write;
use sh1106::{mode::GraphicsMode};
use embassy_nrf::twim::{self, Twim};
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle, Triangle},
    text::Text,
};
use embassy_time::{Duration, Ticker};

#[embassy_executor::task]
pub async fn display_task(twim: Twim<'static>) -> ! {

    let mut disp: GraphicsMode<_> = sh1106::Builder::new()
        .with_rotation(sh1106::displayrotation::DisplayRotation::Rotate270)
        .connect_i2c(twim)
        .into();

    disp.init().unwrap();
    disp.flush().unwrap();

    let style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
    let mut ticker = Ticker::every(Duration::from_millis(500));

    loop {
        ticker.next().await;

        let ride_state: RideState = ride_get();
        disp.clear();
        let mut text: String<64> = String::new();

        let mut y = 20;
        let step = 20;

        let _ = write!(text, "Rpm:");
        Text::new(&text, Point::new(0, y), style).draw(&mut disp).unwrap();
        y += step;

        text.clear();
        let _ = write!(text, "{}", ride_state.cadence_rpm);
        Text::new(&text, Point::new(0, y), style).draw(&mut disp).unwrap();
        y += step;

        text.clear();
        let _ = write!(text, "Gear:");
        Text::new(&text, Point::new(0, y), style).draw(&mut disp).unwrap();
        y += step;

        text.clear();
        let gear = ride_state.calc_gear();
        let _ = write!(text, "{}", gear);
        if let Some(req) = ride_state.target_gear() {
            if req != gear {
                let _ = write!(text, " {:+}", req as i16 - gear as i16);
            }
        }
        Text::new(&text, Point::new(0, y), style).draw(&mut disp).unwrap();
        y += step;

        text.clear();
        let _ = write!(text, "Power: ");
        Text::new(&text, Point::new(0, y), style).draw(&mut disp).unwrap();
        y += step;

        text.clear();
        let _ = write!(text, "{}", ride_state.power_w);
        Text::new(&text, Point::new(0, y), style).draw(&mut disp).unwrap();

        disp.flush().unwrap();
    }
}