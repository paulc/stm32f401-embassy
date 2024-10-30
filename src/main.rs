#![no_std]
#![no_main]

use defmt::*;
use display_interface_spi::SPIInterface;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_stm32::bind_interrupts;
use embassy_stm32::exti::{AnyChannel, Channel, ExtiInput};
use embassy_stm32::gpio::{AnyPin, Level, Output, Pin, Pull, Speed};
use embassy_stm32::peripherals::{self, DMA1_CH4, PB13, PB15, SPI2};
use embassy_stm32::spi;
use embassy_stm32::time::{mhz, Hertz};
use embassy_stm32::usb::{self, Driver, Instance};
use embassy_time::Timer;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
// use embassy_usb::driver::EndpointError;
use embassy_usb::Builder;
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
use portable_atomic::{AtomicBool, Ordering};
use profont::PROFONT_18_POINT;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

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
    let mut config = spi::Config::default();
    config.mode = spi::Mode {
        polarity: spi::Polarity::IdleLow,
        phase: spi::Phase::CaptureOnFirstTransition,
    };
    config.frequency = mhz(30);

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

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = embassy_stm32::Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: Hertz(25_000_000),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll_src = PllSource::HSE;
        config.rcc.pll = Some(Pll {
            prediv: PllPreDiv::DIV25,  // 1Mhz
            mul: PllMul::MUL240,       // 240Mhz
            divp: Some(PllPDiv::DIV4), // 240MHz / 3 = 60MHz SYSCLK
            divq: Some(PllQDiv::DIV5), // 240MHz / 5 = 48MHz USB CLK
            divr: None,
        });
        config.rcc.sys = Sysclk::PLL1_P; // SYSCLK = PLL1_P (60MHz)
        config.rcc.ahb_pre = AHBPrescaler::DIV1; // AHB = SYSCLK     (60MHz)
        config.rcc.apb1_pre = APBPrescaler::DIV2; // APB1 = SYSCLK/2  (30MHz)
        config.rcc.apb2_pre = APBPrescaler::DIV2; // APB2 = SYSCLK/2  (30MHz)
        config.rcc.mux.clk48sel = mux::Clk48sel::PLL1_Q;
    }

    let p = embassy_stm32::init(config);
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

    // Create the USB driver, from the HAL.
    let mut ep_out_buffer = [0u8; 256];
    let mut config = embassy_stm32::usb::Config::default();

    // Do not enable vbus_detection. This is a safe default that works in all boards.
    // However, if your USB device is self-powered (can stay powered on if USB is unplugged), you need
    // to enable vbus_detection to comply with the USB spec. If you enable it, the board
    // has to support it or USB won't work at all. See docs on `vbus_detection` for details.
    config.vbus_detection = false;

    let driver = Driver::new_fs(
        p.USB_OTG_FS,
        Irqs,
        p.PA12,
        p.PA11,
        &mut ep_out_buffer,
        config,
    );

    // Create embassy-usb Config
    let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("USB-serial example");
    config.serial_number = Some("_stm32f401");

    // Required for windows compatibility.
    // https://developer.nordicsemi.com/nRF_Connect_SDK/doc/1.9.1/kconfig/CONFIG_CDC_ACM_IAD.html#help
    config.device_class = 0xEF;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;
    config.composite_with_iads = true;

    // Create embassy-usb DeviceBuilder using the driver and config.
    // It needs some buffers for building the descriptors.
    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut control_buf = [0; 64];

    let mut state = State::new();

    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut [], // no msos descriptors
        &mut control_buf,
    );

    // Create classes on the builder.
    let mut class = CdcAcmClass::new(&mut builder, &mut state, 64);

    // Build the builder.
    let mut usb = builder.build();

    // Run the USB device.
    let usb_fut = usb.run();

    // Do stuff with the class!
    let echo_fut = async {
        loop {
            class.wait_connection().await;
            info!("Connected");
            let mut buf = [0; 64];
            let mut nread: usize = 0;
            loop {
                match class.read_packet(&mut buf).await {
                    Ok(n) => {
                        nread = n;
                        info!("data: [{}] >>{:x}<<", nread, &buf[..n]);
                    }
                    Err(_) => break,
                };
                match class.write_packet(&buf[..nread]).await {
                    Ok(_) => {}
                    Err(_) => break,
                };
            }
            info!("Disconnected");
        }
    };

    // Run everything concurrently.
    // If we had made everything `'static` above instead, we could do this using separate tasks instead.
    join(usb_fut, echo_fut).await;
    loop {
        Timer::after_millis(100).await;
    }
}
