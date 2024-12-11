use core::fmt::Write;
use defmt::info;
use embassy_stm32::mode::Async;
use embassy_stm32::spi::{Error, Spi};
use embassy_stm32::{
    gpio::{AnyPin, Level, Output, Speed},
    spi,
    time::Hertz,
};
use embassy_time::Timer;
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};

pub type DisplaySpi = embassy_stm32::peripherals::SPI2;
pub type DisplaySpiSck = embassy_stm32::peripherals::PB13;
pub type DisplaySpiMosi = embassy_stm32::peripherals::PB15;
pub type DisplaySpiMiso = embassy_stm32::peripherals::PB14;
pub type DisplaySpiRxDma = embassy_stm32::peripherals::DMA1_CH3;
pub type DisplaySpiTxDma = embassy_stm32::peripherals::DMA1_CH4;

pub struct DisplayConfig {
    pub sck: DisplaySpiSck,
    pub mosi: DisplaySpiMosi,
    pub miso: DisplaySpiMiso,
    pub txdma: DisplaySpiTxDma,
    pub rxdma: DisplaySpiRxDma,
    pub dc: AnyPin,
    pub cs: AnyPin,
    pub reset: AnyPin,
    pub backlight: AnyPin,
}

#[embassy_executor::task]
pub async fn display(d: DisplayConfig, spi: DisplaySpi) {
    info!("Starting Display");
    let mut config = spi::Config::default();
    config.mode = spi::Mode {
        polarity: spi::Polarity::IdleLow,
        phase: spi::Phase::CaptureOnFirstTransition,
    };
    config.frequency = Hertz(4_000_000);

    let mut _delay = embassy_time::Delay;

    let spi_bus = spi::Spi::new(spi, d.sck, d.mosi, d.miso, d.txdma, d.rxdma, config);

    let lcd_dc = Output::new(d.dc, Level::Low, Speed::High);
    let lcd_cs = Output::new(d.cs, Level::High, Speed::High);
    let lcd_reset = Output::new(d.reset, Level::High, Speed::Low);
    let mut lcd_backlight = Output::new(d.backlight, Level::Low, Speed::Low);

    lcd_backlight.set_high();

    let mut display = AsyncIli9341::new(spi_bus, lcd_dc, lcd_cs, lcd_reset)
        .await
        .unwrap();

    loop {
        for c in [Rgb565::RED, Rgb565::BLUE, Rgb565::GREEN] {
            info!("Clear....");
            display.clear(0, 0, 63, 63, c).await.unwrap();
            info!("Done....");
            Timer::after_millis(1000).await;
        }
    }
    /*
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

    info!("Starting Display");
    lcd_backlight.set_high();

    // Loop - update every second (await RTC_TIME update)
    loop {
        for c in [Rgb565::RED, Rgb565::BLUE, Rgb565::GREEN] {
            info!("Clear....");
            display.clear(c).ok();
            info!("Done....");
            Timer::after_millis(1000).await;
        }
    }
    */
}

struct AsyncIli9341<'a> {
    spi: Spi<'a, Async>,
    dc: Output<'a>,
    cs: Output<'a>,
    reset: Output<'a>,
}

const NOOP: u8 = 0x00;
const COLUMN_ADDRESS_SET: u8 = 0x2a;
const PAGE_ADDRESS_SET: u8 = 0x2b;
const MEMORY_WRITE: u8 = 0x2c;

fn encode_coords(x: u16, y: u16) -> [u8; 4] {
    let x = x.to_be_bytes();
    let y = y.to_be_bytes();
    [x[0], x[1], y[0], y[1]]
}

fn fmt_buf(buf: &[u8]) -> heapless::String<64> {
    let mut out = heapless::String::new();
    for b in buf.iter().rev() {
        let _ = write!(out, "{:08b} ", b);
    }
    out
}

impl<'a> AsyncIli9341<'a> {
    pub async fn new(
        spi: Spi<'a, Async>,
        dc: Output<'a>,
        cs: Output<'a>,
        reset: Output<'a>,
    ) -> Result<Self, Error> {
        let mut ili9341 = Self { spi, dc, cs, reset };
        ili9341.init().await?;
        Ok(ili9341)
    }

    async fn read_register(&mut self, register: u8, length: usize, text: &'static str) {
        let mut buf: [u8; 16] = [0; 16];
        self.dc.set_low();
        self.cs.set_low();
        self.spi.write(&[register]).await.unwrap();
        //self.dc.set_high();
        self.spi.read(&mut buf[..length]).await.unwrap();
        self.cs.set_high();
        info!("Register: {} {}", register, text);
        for i in 0..length {
            info!("   [{}] >>> {:08b}", i, buf[i]);
        }
    }

    async fn init(&mut self) -> Result<(), Error> {
        // HW reset
        self.reset.set_high();
        Timer::after_millis(5).await;
        self.reset.set_low();
        Timer::after_millis(10).await;
        self.reset.set_high();
        Timer::after_millis(5).await;

        self.command(0x01, &[]).await?; // SW Reset
        Timer::after_millis(120).await;

        self.read_register(0x09, 5, "Display Status >> SW Reset")
            .await;

        /*
        self.command(0x36, &[0x48]).await?; // Memory Access Control
        */

        self.command(0x3a, &[0x55]).await?; // Pixel Format
        self.read_register(0x09, 5, "Display Status >> Pixel Format")
            .await;

        self.read_register(0x0a, 2, "Display Power Mode").await;
        self.command(0x11, &[]).await?; // Sleep Out
        Timer::after_millis(100).await;
        self.read_register(0x09, 5, "Display Status >> Sleep Out")
            .await;

        self.read_register(0x0a, 2, "Display Power Mode").await;
        self.command(0x29, &[]).await?; // Display On
        self.read_register(0x09, 5, "Display Status >> Display On")
            .await;

        self.read_register(0x0a, 2, "Display Power Mode").await;
        self.read_register(0x0b, 2, "MADCTL").await;
        self.read_register(0x0c, 2, "Pixel Data Format").await;
        self.read_register(0x0b, 2, "Disgnostic Result").await;

        Ok(())
    }

    async fn clear(
        &mut self,
        x0: u16,
        y0: u16,
        x1: u16,
        y1: u16,
        colour: Rgb565,
    ) -> Result<(), Error> {
        // Set address window
        self.command(COLUMN_ADDRESS_SET, &encode_coords(x0, y0))
            .await?;
        self.command(PAGE_ADDRESS_SET, &encode_coords(x1, y1))
            .await?;

        // Write data
        let mut buf: [u8; 64] = [0; 64];
        let colour_bytes = colour.to_be_bytes();
        let window_size = (x1 - x0 + 1) * (y1 - y0 + 1);
        info!("Window Size: {}", window_size);
        for bytes in buf.chunks_exact_mut(2) {
            bytes[0] = colour_bytes[0];
            bytes[1] = colour_bytes[1];
        }
        self.command(MEMORY_WRITE, &[]).await?;
        info!("Writing Buffer...");
        for _ in 0..window_size / 64 {
            self.write_data(&buf).await?;
        }
        self.command(NOOP, &[]).await?;
        info!("Done");
        Ok(())
    }

    async fn command(&mut self, command: u8, data: &[u8]) -> Result<(), Error> {
        self.dc.set_low();
        self.cs.set_low();
        self.spi.write(&[command]).await?;
        self.dc.set_high();
        self.spi.write(data).await?;
        self.cs.set_high();
        Ok(())
    }

    async fn _read_data(&mut self, command: u8, read: &mut [u8]) -> Result<(), Error> {
        self.dc.set_low();
        self.cs.set_low();
        self.spi.write(&[command]).await?;
        self.dc.set_high();
        self.spi.read(read).await?;
        self.cs.set_high();
        Ok(())
    }

    async fn write_data(&mut self, data: &[u8]) -> Result<(), Error> {
        self.dc.set_high();
        self.cs.set_low();
        self.spi.write(data).await?;
        self.cs.set_high();
        Ok(())
    }
}
