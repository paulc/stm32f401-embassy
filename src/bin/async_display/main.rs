#![no_std]
#![no_main]

use defmt::*;
use display_task::DisplayConfig;
use embassy_executor::Spawner;
use embassy_stm32::{gpio::Pin, time::Hertz};
use portable_atomic::AtomicBool;
use {defmt_rtt as _, panic_probe as _};

mod display_task;
mod led_task;

static ALARM: AtomicBool = AtomicBool::new(false);

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Setup clocks
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
            divp: Some(PllPDiv::DIV4), // 240MHz / 4 = 60Mhz SYSCLK
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

    let display_config = DisplayConfig {
        sck: p.PB13,
        mosi: p.PB15,
        miso: p.PB14,
        txdma: p.DMA1_CH4,
        rxdma: p.DMA1_CH3,
        dc: p.PB0.degrade(),
        cs: p.PB1.degrade(),
        reset: p.PB2.degrade(),
        backlight: p.PB12.degrade(),
    };

    // Spawn tasks
    spawner.must_spawn(led_task::blink(p.PC13.degrade()));
    spawner.must_spawn(display_task::display(display_config, p.SPI2));
}
