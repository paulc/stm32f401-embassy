use defmt::*;
use ds323x::{DateTimeAccess, Ds323x, NaiveDate, NaiveDateTime, NaiveTime, Rtcc, Timelike};
use embassy_stm32::i2c::I2c;
use embassy_stm32::time::Hertz;
use embassy_sync::pubsub::WaitResult;
use embassy_time::Timer;

pub type I2cDevice = embassy_stm32::peripherals::I2C1;
pub type I2cSclPin = embassy_stm32::peripherals::PB8;
pub type I2cSdaPin = embassy_stm32::peripherals::PB9;

const _DS3231_ADDRESS: u8 = 0x68;
const _DS3231_CONTROL: u8 = 0x0E;
const _DS3231_STATUS: u8 = 0x0F;

#[embassy_executor::task]
pub async fn rtc(i2cdev: I2cDevice, scl: I2cSclPin, sda: I2cSdaPin) {
    let i2c = I2c::new_blocking(i2cdev, scl, sda, Hertz(400_000), Default::default());
    let rtc_time_tx = crate::RTC_TIME.sender();
    let mut sub = crate::MSG_BUS.subscriber().unwrap();
    let mut rtc = Ds323x::new_ds3231(i2c);
    info!(
        "RTC: stopped={} running={} temperature={}",
        rtc.has_been_stopped().ok(),
        rtc.running().ok(),
        rtc.temperature().ok(),
    );
    loop {
        // Check message bus
        while let Some(msg) = sub.try_next_message() {
            match msg {
                WaitResult::Lagged(_) => {}
                WaitResult::Message(crate::Msg::SetTime(h, m, s)) => {
                    let t = NaiveTime::from_hms_opt(h as u32, m as u32, s as u32).unwrap();
                    let d = NaiveDate::from_ymd_opt(2024, 12, 7).unwrap();
                    let dt = NaiveDateTime::new(d, t);
                    rtc.set_datetime(&dt).ok();
                }
            }
        }
        let time = rtc.time().unwrap();
        rtc_time_tx.send((time.hour() as u8, time.minute() as u8, time.second() as u8));
        /*
        info!(
            "Time: {:02}:{:02}:{:02}",
            time.hour(),
            time.minute(),
            time.second()
        );
        */
        Timer::after_millis(1000).await;
    }
}
