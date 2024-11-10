use core::fmt::Write;
// use defmt::info;

pub async fn cli(line: &heapless::String<128>) -> heapless::String<128> {
    let mut out: heapless::String<128> = heapless::String::new();
    if line.is_empty() {
        return out;
    }
    let s = line.as_str();
    if s.starts_with("hello") {
        out.push_str("Hello!").ok();
    } else if s.starts_with("get time") {
        let mut rtc_time_rx = crate::RTC_TIME.receiver().unwrap();
        let (h, m, s) = rtc_time_rx.get().await;
        write!(out, "{:02}:{:02}:{02}", h, m, s).ok();
    } else if s.starts_with("set time") {
        if s.len() < 17 {
            out.push_str("Expected <set time hh:mm:ss>").ok();
        } else {
            let (h, m, s) = (
                &s[9..11].parse::<u8>().unwrap(),
                &s[12..14].parse::<u8>().unwrap(),
                &s[15..17].parse::<u8>().unwrap(),
            );
            let msg_pub = crate::MSG_BUS.publisher().unwrap();
            msg_pub.publish(crate::Msg::SetTime(*h, *m, *s)).await;
        }
    } else {
        out.push_str("Invalid Command <").ok();
        out.push_str(s).ok();
        out.push_str(">").ok();
    }
    out
}
