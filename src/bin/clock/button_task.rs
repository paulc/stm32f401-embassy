use defmt::*;
use embassy_stm32::{
    exti::{AnyChannel, ExtiInput},
    gpio::{AnyPin, Pull},
};
use portable_atomic::Ordering;

#[embassy_executor::task]
pub async fn button(button: AnyPin, exti: AnyChannel) {
    let mut button = ExtiInput::new(button, exti, Pull::Up);
    loop {
        button.wait_for_falling_edge().await;
        info!("Clearing Alarm");
        crate::ALARM.store(false, Ordering::Relaxed);
    }
}
