use defmt::*;
use display_interface_spi::SPIInterface;
use embassy_stm32::{
    gpio::{AnyPin, Level, Output, Speed},
    spi,
    time::Hertz,
};
use embassy_time::Timer;
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
use profont::PROFONT_18_POINT;

pub type DisplaySpi = embassy_stm32::peripherals::SPI2;
pub type DisplaySpiSck = embassy_stm32::peripherals::PB13;
pub type DisplaySpiMosi = embassy_stm32::peripherals::PB15;
pub type DisplaySpiRxDma = embassy_stm32::peripherals::DMA1_CH4;

pub struct DisplayPins {
    pub sck: DisplaySpiSck,
    pub mosi: DisplaySpiMosi,
    pub dc: AnyPin,
    pub cs: AnyPin,
    pub reset: AnyPin,
    pub backlight: AnyPin,
}

#[embassy_executor::task]
pub async fn display(pins: DisplayPins, spi: DisplaySpi, rxdma: DisplaySpiRxDma) {
    let mut config = spi::Config::default();
    config.mode = spi::Mode {
        polarity: spi::Polarity::IdleLow,
        phase: spi::Phase::CaptureOnFirstTransition,
    };
    config.frequency = Hertz(30_000_000);

    let mut delay = embassy_time::Delay;

    let spi_bus = spi::Spi::new_txonly(spi, pins.sck, pins.mosi, rxdma, config);

    let lcd_dc = Output::new(pins.dc, Level::Low, Speed::Low);
    let lcd_cs = Output::new(pins.cs, Level::High, Speed::High);
    let lcd_reset = Output::new(pins.reset, Level::Low, Speed::Low);
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

    let mut scroll = display.configure_vertical_scroll(24, 8).unwrap();

    info!("Starting Display");
    lcd_backlight.set_high();
    display.clear(Rgb565::GREEN).ok();

    Text::with_alignment(
        "HEADER HEADER HEADER",
        Point::new(20, 23),
        MonoTextStyle::new(&PROFONT_18_POINT, Rgb565::RED),
        Alignment::Left,
    )
    .draw(&mut display)
    .ok();

    Text::with_alignment(
        "ABCDEFGHIJKLMNOPQ",
        Point::new(20, 287),
        MonoTextStyle::new(&PROFONT_18_POINT, Rgb565::BLUE),
        Alignment::Left,
    )
    .draw(&mut display)
    .ok();

    Text::with_alignment(
        "ABCDEFGHIJKLMNOPQ",
        Point::new(20, 311),
        MonoTextStyle::new(&PROFONT_18_POINT, Rgb565::BLUE),
        Alignment::Left,
    )
    .draw(&mut display)
    .ok();

    let style = PrimitiveStyle::with_fill(Rgb565::MAGENTA);

    loop {
        Timer::after_millis(100).await;
        display.scroll_vertically(&mut scroll, 24).ok();
        Rectangle::new(Point::new(0, 24), Size::new(240, 24))
            .into_styled(style)
            .draw(&mut display)
            .ok();
    }
}
