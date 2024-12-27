use core::fmt::Write;
use defmt::info;
// use embassy_stm32::mode::Async;
use embassy_stm32::mode::Blocking;
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

    // let spi_bus = spi::Spi::new(spi, d.sck, d.mosi, d.miso, d.txdma, d.rxdma, config);
    let spi_bus = spi::Spi::new_blocking(spi, d.sck, d.mosi, d.miso, config);

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
    // spi: Spi<'a, Async>,
    spi: Spi<'a, Blocking>,
    dc: Output<'a>,
    cs: Output<'a>,
    reset: Output<'a>,
}

const NOOP: u8 = 0x00;
const COLUMN_ADDRESS_SET: u8 = 0x2A;
const PAGE_ADDRESS_SET: u8 = 0x2B;
const MEMORY_WRITE: u8 = 0x2C;

const ILI9341_INIT: [(u8, &[u8]); 21] = [
    (0xEF, &[0x03, 0x80, 0x02]),
    (0xCF, &[0x00, 0xc1, 0x30]),
    (0xED, &[0x64, 0x03, 0x12, 0x81]),
    (0xE8, &[0x85, 0x00, 0x78]),
    (0xCB, &[0x39, 0x2c, 0x00, 0x34, 0x02]),
    (0xF7, &[0x20]),
    (0xEA, &[0x00, 0x00]),
    (0xC0, &[0x23]),             // Power Control 1, VRH[5:0]
    (0xC1, &[0x10]),             // Power Control 2, SAP[2:0], BT[3:0]
    (0xC5, &[0x3e, 0x28]),       // VCM Control 1
    (0xC7, &[0x86]),             // VCM Control 2
    (0x36, &[0x48]),             // Memory Access Control
    (0x3A, &[0x55]),             // Pixel Format
    (0xB1, &[0x00, 0x18]),       // FRMCTR1
    (0xB6, &[0x08, 0x82, 0x27]), // Display Function Control
    (0xF2, &[0x00]),             // 3Gamma Function Disable
    (0x26, &[0x01]),             // Gamma Curve Selected
    (
        0xE0,
        &[
            0x0f, 0x31, 0x2b, 0x0c, 0x0e, 0x08, 0x4e, 0xf1, 0x37, 0x07, 0x10, 0x03, 0x0e, 0x09,
            0x00,
        ],
    ), // Set Gamma
    (
        0xE1,
        &[
            0x00, 0x0e, 0x14, 0x03, 0x11, 0x07, 0x31, 0xc1, 0x48, 0x08, 0x0f, 0x0c, 0x31, 0x36,
            0x0f,
        ],
    ), // Set Gamma
    (0x11, &[]),
    (0x29, &[]),
];

fn encode_coords(x: u16, y: u16) -> [u8; 4] {
    let x = x.to_be_bytes();
    let y = y.to_be_bytes();
    [x[0], x[1], y[0], y[1]]
}

fn _fmt_buf(buf: &[u8]) -> heapless::String<64> {
    let mut out = heapless::String::new();
    for b in buf.iter().rev() {
        let _ = write!(out, "{:08b} ", b);
    }
    out
}

impl<'a> AsyncIli9341<'a> {
    pub async fn new(
        // spi: Spi<'a, Async>,
        spi: Spi<'a, Blocking>,
        dc: Output<'a>,
        cs: Output<'a>,
        reset: Output<'a>,
    ) -> Result<Self, Error> {
        let mut ili9341 = Self { spi, dc, cs, reset };
        ili9341.init().await?;
        Ok(ili9341)
    }

    async fn read_register(&mut self, register: u8, length: usize, text: &'static str) {
        let mut buf_r: [u8; 16] = [0; 16];
        let buf_w: [u8; 16] = [0; 16];
        self.dc.set_low();
        self.cs.set_low();
        // self.spi.write(&[register]).await.unwrap();
        self.spi.blocking_write(&[register]).unwrap();
        self.dc.set_high();
        let r = &mut buf_r[..length];
        let w = &buf_w[..length];
        // self.spi.transfer(r, w).await.unwrap();
        self.spi.blocking_transfer(r, w).unwrap();
        self.cs.set_high();
        info!("Register: {} {}", register, text);
        for i in 0..length {
            info!("   [{}] >>> {:08b}", i, r[i]);
        }
    }

    async fn command(&mut self, command: u8, data: &[u8]) -> Result<(), Error> {
        self.dc.set_low();
        self.cs.set_low();
        // self.spi.write(&[command]).await?;
        self.spi.blocking_write(&[command])?;
        self.dc.set_high();
        if !data.is_empty() {
            // self.spi.write(data).await?;
            self.spi.blocking_write(data)?;
        }
        self.cs.set_high();
        Ok(())
    }

    async fn hw_reset(&mut self) {
        self.reset.set_low();
        Timer::after_millis(50).await;
        self.reset.set_high();
        Timer::after_millis(50).await;
    }

    async fn sw_reset(&mut self) -> Result<(), Error> {
        self.command(0x01, &[]).await?; // SW Reset
        Timer::after_millis(120).await;
        Ok(())
    }

    async fn init(&mut self) -> Result<(), Error> {
        self.dc.set_high();
        self.cs.set_high();
        // HW reset
        self.hw_reset().await;

        // SW reset
        self.sw_reset().await?;

        self.read_register(0x09, 5, "Status -> Reset").await;

        for (cmd, data) in ILI9341_INIT.into_iter() {
            self.command(cmd, data).await?;
        }

        self.read_register(0x09, 5, "Status -> Init").await;
        self.clear(10, 10, 20, 50, Rgb565::GREEN).await?;

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

    /*
    async fn _read_data(&mut self, command: u8, read: &mut [u8]) -> Result<(), Error> {
        self.dc.set_low();
        self.cs.set_low();
        self.spi.write(&[command]).await?;
        self.dc.set_high();
        self.spi.read(read).await?;
        self.cs.set_high();
        Ok(())
    }
    */

    async fn write_data(&mut self, data: &[u8]) -> Result<(), Error> {
        self.dc.set_high();
        self.cs.set_low();
        // self.spi.write(data).await?;
        self.spi.blocking_write(data)?;
        self.cs.set_high();
        Ok(())
    }
}
