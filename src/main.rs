#![no_std]
#![no_main]

use defmt::*;
use display_interface_spi::SPIInterface;
use embassy_executor::Spawner;
use embassy_stm32::exti::{AnyChannel, Channel, ExtiInput};
use embassy_stm32::gpio::{AnyPin, Level, Output, Pin, Pull, Speed};
use embassy_stm32::peripherals::{DMA1_CH4, PB13, PB15, SPI2};
use embassy_stm32::spi::{Config, Mode, Phase, Polarity, Spi};
use embassy_stm32::time::mhz;
use embassy_time::Timer;
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use embedded_hal_bus::spi::ExclusiveDevice;
use ili9341::{DisplaySize240x320, Ili9341, Orientation};
use portable_atomic::{AtomicBool, Ordering};
use {defmt_rtt as _, panic_probe as _};

static FLASH: AtomicBool = AtomicBool::new(false);

#[embassy_executor::task]
async fn blink(led: AnyPin) {
    let mut led = Output::new(led, Level::High, Speed::Low);
    led.set_high();

    loop {
        if FLASH.load(Ordering::Relaxed) {
            info!("FLASH -> true");
            led.set_low();
            Timer::after_millis(100).await;
            led.set_high();
            Timer::after_millis(100).await;
        } else {
            Timer::after_millis(10).await;
        }
    }
}

#[embassy_executor::task]
async fn button(button: AnyPin, exti: AnyChannel) {
    let mut button = ExtiInput::new(button, exti, Pull::Up);
    loop {
        button.wait_for_falling_edge().await;
        FLASH.store(true, Ordering::Relaxed);
        button.wait_for_rising_edge().await;
        FLASH.store(false, Ordering::Relaxed);
    }
}

struct DisplayPins {
    sck: PB13,
    mosi: PB15,
    dc: AnyPin,
    cs: AnyPin,
    reset: AnyPin,
    backlight: AnyPin,
}

#[embassy_executor::task]
async fn display(pins: DisplayPins, spi: SPI2, rxdma: DMA1_CH4) {
    let mut config = Config::default();
    config.mode = Mode {
        polarity: Polarity::IdleLow,
        phase: Phase::CaptureOnFirstTransition,
    };
    config.frequency = mhz(10);

    let mut delay = embassy_time::Delay;

    let spi_bus = Spi::new_txonly(spi, pins.sck, pins.mosi, rxdma, config);

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
        Orientation::Landscape,
        DisplaySize240x320,
    )
    .unwrap();

    info!("Starting Display");
    lcd_backlight.set_high();
    loop {
        info!("RED");
        display.clear(Rgb565::RED).ok();
        Timer::after_millis(100).await;
        info!("BLUE");
        display.clear(Rgb565::BLUE).ok();
        Timer::after_millis(100).await;
        info!("GREEN");
        display.clear(Rgb565::GREEN).ok();
        Timer::after_millis(100).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    info!("embassy_stm32::init");

    let display_pins = DisplayPins {
        sck: p.PB13,
        mosi: p.PB15,
        dc: p.PB0.degrade(),
        cs: p.PB1.degrade(),
        reset: p.PB2.degrade(),
        backlight: p.PB12.degrade(),
    };

    spawner.spawn(blink(p.PC13.degrade())).unwrap();
    spawner
        .spawn(button(p.PA0.degrade(), p.EXTI0.degrade()))
        .unwrap();
    spawner
        .spawn(display(display_pins, p.SPI2, p.DMA1_CH4))
        .unwrap();

    loop {
        Timer::after_millis(100).await;
    }
}
