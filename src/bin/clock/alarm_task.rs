use defmt::*;
use embassy_stm32::{
    exti::{AnyChannel, ExtiInput},
    gpio::{AnyPin, Pull},
};

#[embassy_executor::task]
pub async fn alarm(button: AnyPin, exti: AnyChannel) {
    let mut button = ExtiInput::new(button, exti, Pull::Up);
    loop {
        button.wait_for_any_edge().await;
        info!("Alarm: {}", button.get_level());
    }
}
