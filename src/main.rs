#![no_std]
#![no_main]

use defmt::*;
use display_interface_spi::SPIInterface;
use embassy_executor::Spawner;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{AnyPin, Level, Output, Pin, Pull, Speed};
use embassy_stm32::peripherals;
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
async fn button(button: peripherals::PA0, exti: peripherals::EXTI0) {
    let mut button = ExtiInput::new(button, exti, Pull::Up);
    loop {
        button.wait_for_falling_edge().await;
        FLASH.store(true, Ordering::Relaxed);
        button.wait_for_rising_edge().await;
        FLASH.store(false, Ordering::Relaxed);
    }
}

use embassy_stm32::mode::Async;
use embedded_hal_bus::spi::NoDelay;
type _DisplayType<'a> = Ili9341<
    SPIInterface<ExclusiveDevice<Spi<'a, Async>, Output<'a>, NoDelay>, Output<'a>>,
    Output<'a>,
>;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    let mut delay = embassy_time::Delay;
    info!("embassy_stm32::init");

    // SPI bus
    let sck = p.PB13;
    let mosi = p.PB15;

    let mut config = Config::default();
    config.mode = Mode {
        polarity: Polarity::IdleLow,
        phase: Phase::CaptureOnFirstTransition,
    };
    config.frequency = mhz(10);

    let spi_bus = Spi::new_txonly(p.SPI2, sck, mosi, p.DMA1_CH4, config);

    let lcd_dc = Output::new(p.PB0, Level::Low, Speed::Low);
    let lcd_cs = Output::new(p.PB1, Level::High, Speed::High);
    let lcd_reset = Output::new(p.PB2, Level::Low, Speed::Low);
    let mut lcd_backlight = Output::new(p.PB12, Level::Low, Speed::Low);

    // let spi_device = ExclusiveDevice::new_no_delay(spi_bus, lcd_cs).unwrap();
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
    info!("RED");
    display.clear(Rgb565::RED).ok();
    info!("BLUE");
    display.clear(Rgb565::BLUE).ok();
    info!("GREEN");
    display.clear(Rgb565::GREEN).ok();

    spawner.spawn(blink(p.PC13.degrade())).unwrap();
    spawner.spawn(button(p.PA0, p.EXTI0)).unwrap();

    loop {
        Timer::after_millis(100).await;
    }
}
