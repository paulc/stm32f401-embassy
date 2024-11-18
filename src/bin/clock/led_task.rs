use defmt::*;
use embassy_stm32::gpio::{AnyPin, Level, Output, Speed};
use embassy_time::Timer;
use portable_atomic::Ordering;

#[embassy_executor::task]
pub async fn blink(led: AnyPin) {
    let mut led = Output::new(led, Level::High, Speed::Low);
    led.set_high();

    loop {
        if crate::FLASH.load(Ordering::Relaxed) {
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
