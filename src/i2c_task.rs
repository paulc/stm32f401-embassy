use core::fmt::Write;
use defmt::{error, info};
use ds323x::{DateTimeAccess, Ds323x, NaiveDateTime};
use embassy_stm32::i2c::I2c;
use embassy_stm32::time::Hertz;
use embassy_sync::pubsub::WaitResult;
use embassy_time::Timer;
use heapless::String;

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
    match rtc.enable() {
        Ok(_) => {}
        Err(_) => panic!("Error enabling RTC"),
    }
    rtc.use_int_sqw_output_as_square_wave().ok();
    rtc.enable_32khz_output().ok();
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
                WaitResult::Message(crate::Msg::SetTime(t)) => match rtc.datetime() {
                    Ok(now) => {
                        let d = now.date();
                        let dt = NaiveDateTime::new(d, t);
                        rtc.set_datetime(&dt).ok();
                    }
                    Err(_) => error!("Error setimng clock"),
                },
                WaitResult::Message(crate::Msg::SetDate(d)) => match rtc.datetime() {
                    Ok(now) => {
                        let t = now.time();
                        let dt = NaiveDateTime::new(d, t);
                        rtc.set_datetime(&dt).ok();
                    }
                    Err(_) => error!("Error setimng clock"),
                },
            }
        }
        match rtc.datetime() {
            Ok(time) => rtc_time_tx.send(time),
            Err(e) => {
                let mut s: String<32> = String::new();
                write!(s, "{:?}", e).ok();
                error!("rtc.gettime: {}", s.as_str());
            }
        }
        Timer::after_millis(1000).await;
    }
}
