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

        // top right, icon is 22 px wide including the terminal
        let icon_x = disp.size().width as i32 - 22;
        draw_battery_icon(&mut disp, icon_x, 0, ride_state.battery_pct(), ride_state.battery_charging);

        disp.flush().unwrap();
    }
}

fn draw_battery_icon<D>(disp: &mut D, x: i32, y: i32, pct: u8, charging: bool)
where
    D: DrawTarget<Color = BinaryColor, Error = core::convert::Infallible>,
{
    let battery_width: i32 = 20;
    let battery_height: i32 = 10;
    let battery_border = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
    let battery_fill = PrimitiveStyle::with_fill(BinaryColor::On);

    // Draw battery outline
    Rectangle::new(Point::new(x, y), Size::new(battery_width as u32, battery_height as u32))
        .into_styled(battery_border)
        .draw(disp)
        .unwrap();

    // Draw battery positive terminal
    Rectangle::new(Point::new(x + battery_width, y + 3), Size::new(2, battery_height as u32 - 6))
        .into_styled(battery_border)
        .draw(disp)
        .unwrap();

    // Draw battery fill based on percentage
    let fill_width = (battery_width - 2) * pct as i32 / 100;
    if fill_width > 0 {
        Rectangle::new(Point::new(x + 1, y + 1), Size::new(fill_width as u32, battery_height as u32 - 2))
            .into_styled(battery_fill)
            .draw(disp)
            .unwrap();
    }

    // Draw charging icon if charging, inverted against whatever is underneath so it shows at any charge level
    if charging {
        let bolt = Triangle::new(
            Point::new(x + battery_width / 2 - 2, y + 2),
            Point::new(x + battery_width / 2 + 2, y + 2),
            Point::new(x + battery_width / 2, y + battery_height - 2),
        );
        let fill_width = fill_width.max(0);
        let inner_height = battery_height as u32 - 2;
        let filled = Rectangle::new(Point::new(x + 1, y + 1), Size::new(fill_width as u32, inner_height));
        let empty = Rectangle::new(
            Point::new(x + 1 + fill_width, y + 1),
            Size::new((battery_width - 2 - fill_width) as u32, inner_height),
        );

        bolt.into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
            .draw(&mut disp.clipped(&filled))
            .unwrap();
        bolt.into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(&mut disp.clipped(&empty))
            .unwrap();
    }
}