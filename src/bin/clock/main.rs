#![no_std]
#![no_main]

use chrono::NaiveDateTime;
use chrono::{NaiveDate, NaiveTime};
use defmt::*;
use display_task::DisplayPins;
use embassy_executor::Spawner;
use embassy_stm32::{bind_interrupts, exti::Channel, gpio::Pin, time::Hertz, usb};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, pubsub::PubSubChannel, watch::Watch,
};
use embedded_graphics::draw_target::DrawTarget;
use portable_atomic::AtomicBool;
use {defmt_rtt as _, panic_probe as _};

mod alarm_task;
mod button_task;
mod cli;
mod display_task;
mod led_task;
mod line_input;
mod rtc_task;
mod usb_task;

bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<usb_task::UsbOtgPeripheral>;
});

#[derive(Clone, Debug)]
enum Msg {
    SetTime(NaiveTime),
    SetDate(NaiveDate),
    // SetBacklight(f32),
    SetAlarm1(NaiveTime),
}

impl defmt::Format for Msg {
    fn format(&self, fmt: Formatter) {
        match self {
            Msg::SetTime(_) => defmt::write!(fmt, "<SetTime>"),
            Msg::SetDate(_) => defmt::write!(fmt, "<SetDate>"),
            // Msg::SetBacklight(_) => defmt::write!(fmt, "<SetBacklight>"),
            Msg::SetAlarm1(_) => defmt::write!(fmt, "<SetAlarm1>"),
        }
    }
}

// Global values
static RTC_TIME: Watch<CriticalSectionRawMutex, NaiveDateTime, 4> = Watch::new();
static RTC_TEMP: Watch<CriticalSectionRawMutex, f32, 4> = Watch::new();
static MSG_BUS: PubSubChannel<CriticalSectionRawMutex, Msg, 4, 4, 4> = PubSubChannel::new();
static ALARM: AtomicBool = AtomicBool::new(false);
static ALARM1_TIME: Watch<CriticalSectionRawMutex, Option<NaiveTime>, 4> = Watch::new();
static ALARM1_MATCH: Watch<CriticalSectionRawMutex, bool, 4> = Watch::new();
// static ALARM2_TIME: Watch<CriticalSectionRawMutex, Option<NaiveTime>, 4> = Watch::new();
// static ALARM2_MATCH: Watch<CriticalSectionRawMutex, bool, 4> = Watch::new();

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

    let display_pins = DisplayPins {
        sck: p.PB13,
        mosi: p.PB15,
        dc: p.PB0.degrade(),
        cs: p.PB1.degrade(),
        reset: p.PB2.degrade(),
        backlight: p.PB12.degrade(),
    };

    // Spawn tasks
    spawner.must_spawn(rtc_task::rtc(p.I2C1, p.PB8, p.PB9));
    spawner.must_spawn(button_task::button(p.PA0.degrade(), p.EXTI0.degrade()));
    spawner.must_spawn(alarm_task::alarm(p.PA1.degrade(), p.EXTI1.degrade()));
    spawner.must_spawn(led_task::blink(p.PC13.degrade()));
    spawner.must_spawn(display_task::display(display_pins, p.SPI2, p.DMA1_CH4));
    spawner.must_spawn(usb_task::usb_device(spawner, p.USB_OTG_FS, p.PA12, p.PA11));
}
