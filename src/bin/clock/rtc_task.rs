use core::fmt::Write;
use defmt::{error, info};
use ds323x::ic::DS3231;
use ds323x::interface::I2cInterface;
use ds323x::{DateTimeAccess, Ds323x, Error, NaiveDateTime, Timelike};
use embassy_stm32::i2c::I2c;
use embassy_stm32::mode::Blocking;
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

type RtcInstance<'a> = Ds323x<I2cInterface<I2c<'a, Blocking>>, DS3231>;

/*

type RtcFn<'a, CommE, PinE> = fn(
    &mut Ds323x<I2cInterface<I2c<'a, Blocking>>, DS3231>,
) -> Result<(), Error<Error<CommE, PinE>, ()>>;


fn retry<'a, CommE, PinE>(
    rtc: &'a mut RtcInstance<'a>,
    f: RtcFn<'a, CommE, PinE>,
) -> Result<(), Error<Error<CommE, PinE>, ()>> {
    f(rtc)
}
*/

#[embassy_executor::task]
pub async fn rtc(i2cdev: I2cDevice, scl: I2cSclPin, sda: I2cSdaPin) {
    let i2c = I2c::new_blocking(i2cdev, scl, sda, Hertz(400_000), Default::default());
    let rtc_time_tx = crate::RTC_TIME.sender();
    let rtc_temp_tx = crate::RTC_TEMP.sender();
    let alarm1_time_tx = crate::ALARM1_TIME.sender();
    let alarm1_match_tx = crate::ALARM1_MATCH.sender();
    let mut sub = crate::MSG_BUS.subscriber().unwrap();
    let mut rtc = Ds323x::new_ds3231(i2c);

    // Configure RTC
    let rtc_setup = [
        |rtc: &mut RtcInstance| rtc.enable(),
        |rtc: &mut RtcInstance| rtc.clear_alarm1_matched_flag(),
        |rtc: &mut RtcInstance| rtc.clear_alarm2_matched_flag(),
        |rtc: &mut RtcInstance| rtc.enable_alarm1_interrupts(),
        |rtc: &mut RtcInstance| rtc.enable_alarm2_interrupts(),
        |rtc: &mut RtcInstance| rtc.use_int_sqw_output_as_interrupt(),
    ];

    // Use wrapper to call setup functions
    for f in rtc_setup {
        while let Err(e) = f(&mut rtc) {
            match e {
                Error::Comm(e) => {
                    error!("RTC Comm Error (retrying): {:?}", e);
                    Timer::after_micros(100).await;
                }
                _ => panic!("Error configuring RTC: {:?}", e),
            }
        }
    }

    info!(
        "RTC: stopped={} running={} temperature={}",
        rtc.has_been_stopped().ok(),
        rtc.running().ok(),
        rtc.temperature().ok(),
    );

    // Set initial temp
    if let Ok(temp) = rtc.temperature() {
        rtc_temp_tx.send(temp);
    }
    loop {
        // Check message bus
        while let Some(msg) = sub.try_next_message() {
            match msg {
                WaitResult::Lagged(_) => {}
                WaitResult::Message(crate::Msg::SetTime(t)) => {
                    match rtc.datetime().and_then(|now| {
                        let d = now.date();
                        let dt = NaiveDateTime::new(d, t);
                        rtc.set_datetime(&dt)
                            .and_then(|_| rtc.clear_has_been_stopped_flag())
                    }) {
                        Ok(_) => {}
                        Err(_) => error!("Error setting clock"),
                    }
                }
                WaitResult::Message(crate::Msg::SetDate(d)) => {
                    match rtc.datetime().and_then(|now| {
                        let t = now.time();
                        let dt = NaiveDateTime::new(d, t);
                        rtc.set_datetime(&dt)
                    }) {
                        Ok(_) => {}
                        Err(_) => error!("Error setting clock"),
                    }
                }
                WaitResult::Message(crate::Msg::SetAlarm1(t)) => match rtc
                    .clear_alarm1_matched_flag()
                    .and_then(|_| rtc.set_alarm1_hms(t))
                {
                    Ok(_) => {
                        alarm1_time_tx.send(Some(t));
                        alarm1_match_tx.send(false);
                    }
                    Err(_) => error!("Error setting alarm"),
                },
                // WaitResult::Message(_) => {} // Ignore other messages
            }
        }
        // Update global time
        match rtc.datetime() {
            Ok(time) => {
                rtc_time_tx.send(time);
                // Update temperature every minute
                if time.second() == 0 {
                    if let Ok(temp) = rtc.temperature() {
                        rtc_temp_tx.send(temp);
                    }
                }
            }
            Err(e) => {
                let mut s: String<32> = String::new();
                write!(s, "{:?}", e).ok();
                error!("rtc.gettime: {}", s.as_str());
            }
        }
        Timer::after_millis(1000).await;
    }
}
