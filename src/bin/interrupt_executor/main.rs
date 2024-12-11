#![no_std]
#![no_main]

use cortex_m_rt::entry;
use defmt::*;
use display_task::DisplayPins;
use embassy_executor::{Executor, InterruptExecutor};
use embassy_stm32::interrupt;
use embassy_stm32::interrupt::{InterruptExt, Priority};
use embassy_stm32::{gpio::Pin, time::Hertz};
use portable_atomic::AtomicBool;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

mod display_task;
mod led_task;

static ALARM: AtomicBool = AtomicBool::new(true);

static INTERRUPT_EXECUTOR: InterruptExecutor = InterruptExecutor::new();
static EXECUTOR: StaticCell<Executor> = StaticCell::new();

#[interrupt]
unsafe fn SPI4() {
    INTERRUPT_EXECUTOR.on_interrupt()
}

#[entry]
fn main() -> ! {
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

    let display_pins = DisplayPins {
        sck: p.PB13,
        mosi: p.PB15,
        dc: p.PB0.degrade(),
        cs: p.PB1.degrade(),
        reset: p.PB2.degrade(),
        backlight: p.PB12.degrade(),
    };

    // Spawn tasks
    interrupt::SPI4.set_priority(Priority::P6);
    let interrupt_spawner = INTERRUPT_EXECUTOR.start(interrupt::SPI4);
    interrupt_spawner.must_spawn(led_task::blink(p.PC13.degrade()));
    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        spawner.must_spawn(display_task::display(display_pins, p.SPI2, p.DMA1_CH4));
    });
}
