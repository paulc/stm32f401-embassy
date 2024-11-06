use defmt::info;
use display_interface_spi::SPIInterface;
use eg_seven_segment::{Digit, Segments, SevenSegmentStyle, SevenSegmentStyleBuilder};
use embassy_stm32::{
    gpio::{AnyPin, Level, Output, Speed},
    spi,
    time::Hertz,
};
use embedded_graphics::{
    draw_target::DrawTarget,
    mono_font::MonoTextStyle,
    pixelcolor::Rgb565,
    prelude::*,
    primitives::rectangle::Rectangle,
    primitives::PrimitiveStyle,
    text::{Alignment, Text},
};
use embedded_hal_bus::spi::ExclusiveDevice;
use ili9341::{DisplaySize240x320, Ili9341, Orientation};
use profont::PROFONT_24_POINT;

pub type DisplaySpi = embassy_stm32::peripherals::SPI2;
pub type DisplaySpiSck = embassy_stm32::peripherals::PB13;
pub type DisplaySpiMosi = embassy_stm32::peripherals::PB15;
pub type DisplaySpiRxDma = embassy_stm32::peripherals::DMA1_CH4;

// 7-segment display
const DIGIT_WIDTH: u32 = 32;
const DIGIT_HEIGHT: u32 = 64;
const DIGIT_SPACING: u32 = 4;
const SEGMENT_WIDTH: u32 = 8;
const SEGMENT_COLOUR: Rgb565 = Rgb565::GREEN;
const BACKGROUND_COLOUR: Rgb565 = Rgb565::WHITE;
const START_Y: i32 = 60;
const START_X: i32 = 2;

// START_X | DIGIT | SP | DIGIT | SP | SEP | SP | DIGIT | SP | DIGIT | SP | SEP | SP | DIGIT | SEP
const SEPARATOR_OFFSETS: [i32; 2] = [
    START_X + (2 * DIGIT_WIDTH + 2 * DIGIT_SPACING) as i32,
    START_X + (4 * DIGIT_WIDTH + 5 * DIGIT_SPACING + SEGMENT_WIDTH) as i32,
];
const DIGIT_OFFSETS: [i32; 6] = [
    START_X,
    START_X + (DIGIT_WIDTH + DIGIT_SPACING) as i32,
    START_X + (2 * DIGIT_WIDTH + 3 * DIGIT_SPACING + SEGMENT_WIDTH) as i32,
    START_X + (3 * DIGIT_WIDTH + 4 * DIGIT_SPACING + SEGMENT_WIDTH) as i32,
    START_X + (4 * DIGIT_WIDTH + 6 * DIGIT_SPACING + 2 * SEGMENT_WIDTH) as i32,
    START_X + (5 * DIGIT_WIDTH + 7 * DIGIT_SPACING + 2 * SEGMENT_WIDTH) as i32,
];

pub struct DisplayPins {
    pub sck: DisplaySpiSck,
    pub mosi: DisplaySpiMosi,
    pub dc: AnyPin,
    pub cs: AnyPin,
    pub reset: AnyPin,
    pub backlight: AnyPin,
}

#[embassy_executor::task]
pub async fn display(pins: DisplayPins, spi: DisplaySpi, _rxdma: DisplaySpiRxDma) {
    let mut config = spi::Config::default();
    config.mode = spi::Mode {
        polarity: spi::Polarity::IdleLow,
        phase: spi::Phase::CaptureOnFirstTransition,
    };
    config.frequency = Hertz(15_000_000);

    let mut delay = embassy_time::Delay;

    // let spi_bus = spi::Spi::new_txonly(spi, pins.sck, pins.mosi, rxdma, config);
    let spi_bus = spi::Spi::new_blocking_txonly(spi, pins.sck, pins.mosi, config);

    let lcd_dc = Output::new(pins.dc, Level::Low, Speed::High);
    let lcd_cs = Output::new(pins.cs, Level::High, Speed::High);
    let lcd_reset = Output::new(pins.reset, Level::Low, Speed::High);
    let mut lcd_backlight = Output::new(pins.backlight, Level::Low, Speed::Low);

    let spi_device = ExclusiveDevice::new(spi_bus, lcd_cs, delay.clone()).unwrap();
    let display_if = SPIInterface::new(spi_device, lcd_dc);

    let mut display = Ili9341::new(
        display_if,
        lcd_reset,
        &mut delay,
        Orientation::Portrait,
        DisplaySize240x320,
    )
    .unwrap();

    // let mut scroll = display.configure_vertical_scroll(30, 2).unwrap();

    info!("Starting Display");
    lcd_backlight.set_high();

    display.clear(Rgb565::WHITE).ok();

    Text::with_alignment(
        "HEADER HEADER",
        Point::new(20, 29),
        MonoTextStyle::new(&PROFONT_24_POINT, Rgb565::RED),
        Alignment::Left,
    )
    .draw(&mut display)
    .ok();

    let mut rtc_time_rx = crate::RTC_TIME.receiver().unwrap();
    info!("DIGIT OFFSETS >> {:?}", DIGIT_OFFSETS);
    info!("SEPARATOR OFFSETS >> {:?}", SEPARATOR_OFFSETS);

    let segment_style = SevenSegmentStyleBuilder::new()
        .digit_size(Size::new(DIGIT_WIDTH, DIGIT_HEIGHT))
        .digit_spacing(DIGIT_SPACING)
        .segment_width(SEGMENT_WIDTH)
        .segment_color(SEGMENT_COLOUR)
        .build();
    let background_style = PrimitiveStyle::with_fill(BACKGROUND_COLOUR);

    draw_separators(&mut display, segment_style);

    let mut t_prev: [u8; 6] = [11; 6]; // Make sure all digits are invalid

    loop {
        let t = rtc_time_rx.changed().await;
        t_prev = draw_clock(&mut display, segment_style, background_style, t, t_prev);
    }
}

fn draw_separators<D>(display: &mut D, segment_style: SevenSegmentStyle<Rgb565>)
where
    D: DrawTarget<Color = Rgb565>,
{
    for x_offset in SEPARATOR_OFFSETS {
        Text::new(
            ":",
            Point::new(x_offset, START_Y + DIGIT_HEIGHT as i32),
            segment_style,
        )
        .draw(display)
        .ok();
    }
}

fn draw_clock<D>(
    display: &mut D,
    segment_style: SevenSegmentStyle<Rgb565>,
    background_style: PrimitiveStyle<Rgb565>,
    (h, m, s): (u8, u8, u8),
    t_prev: [u8; 6],
) -> [u8; 6]
where
    D: DrawTarget<Color = Rgb565>,
{
    let t_next = [h / 10, h % 10, m / 10, m % 10, s / 10, s % 10];
    for ((digit, prev), x_offset) in t_next.into_iter().zip(t_prev).zip(DIGIT_OFFSETS) {
        if digit != prev {
            Rectangle::new(
                Point::new(x_offset, START_Y),
                Size::new(DIGIT_WIDTH, DIGIT_HEIGHT),
            )
            .into_styled(background_style)
            .draw(display)
            .ok();
            let segments = Segments::try_from(char::from(digit + b'0')).unwrap();
            Digit::new(segments, Point::new(x_offset, START_Y))
                .into_styled(segment_style)
                .draw(display)
                .ok();
        }
    }
    t_next
}
