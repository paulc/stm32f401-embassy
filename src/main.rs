#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::{bind_interrupts, exti::Channel, gpio::Pin, time::Hertz, usb};
use portable_atomic::AtomicBool;
use {defmt_rtt as _, panic_probe as _};

mod button_task;
mod display_task;
mod led_task;
mod usb_task;

use display_task::DisplayPins;

bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<usb_task::UsbOtgPeripheral>;
});

pub static FLASH: AtomicBool = AtomicBool::new(true);

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
            divp: Some(PllPDiv::DIV4), // 240MHz / 4 = 60MHz SYSCLK
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

    spawner
        .spawn(button_task::button(p.PA0.degrade(), p.EXTI0.degrade()))
        .unwrap();
    spawner.spawn(led_task::blink(p.PC13.degrade())).unwrap();
    spawner
        .spawn(display_task::display(display_pins, p.SPI2, p.DMA1_CH4))
        .unwrap();
    spawner
        .spawn(usb_task::usb_device(spawner, p.USB_OTG_FS, p.PA12, p.PA11))
        .unwrap();
}
