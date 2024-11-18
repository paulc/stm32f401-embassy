use chrono::{Datelike, Timelike};
use chrono::{NaiveDate, NaiveTime};
use core::fmt::Write;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{multispace0, multispace1, one_of};
use nom::combinator::{map_opt, rest, value};
use nom::error::Error;
use nom::sequence::{pair, tuple};
use nom::IResult;
// use defmt::info;

#[derive(Clone, Debug)]
enum CliMsg {
    Hello,
    GetTime,
    GetDate,
    SetTime(NaiveTime),
    SetDate(NaiveDate),
}

fn _digit_parser(input: &str) -> IResult<&str, u32, Error<&str>> {
    map_opt(
        pair(one_of("0123456789"), one_of("0123456789")),
        |(d1, d2)| {
            d1.to_digit(10)
                .and_then(|n1| d2.to_digit(10).and_then(|n2| Some(n1 * 10 + n2)))
        },
    )(input)
}

fn set_date_parser(input: &str) -> IResult<&str, CliMsg, Error<&str>> {
    map_opt(
        tuple((
            multispace0,
            tag("set"),
            multispace1,
            tag("date"),
            multispace1,
            rest,
        )),
        |(_, _, _, _, _, date): (_, _, _, _, _, &str)| match NaiveDate::parse_from_str(
            date.trim(),
            "%d/%m/%Y",
        ) {
            Ok(d) => Some(CliMsg::SetDate(d)),
            Err(_) => None,
        },
    )(input)
}

fn set_time_parser(input: &str) -> IResult<&str, CliMsg, Error<&str>> {
    map_opt(
        tuple((
            multispace0,
            tag("set"),
            multispace1,
            tag("time"),
            multispace1,
            rest,
        )),
        |(_, _, _, _, _, time): (_, _, _, _, _, &str)| match NaiveTime::parse_from_str(
            time.trim(),
            "%H:%M:%S",
        ) {
            Ok(t) => Some(CliMsg::SetTime(t)),
            Err(_) => None,
        },
    )(input)
}

fn get_time_parser(input: &str) -> IResult<&str, CliMsg, Error<&str>> {
    value(
        CliMsg::GetTime,
        tuple((
            multispace0,
            tag("get"),
            multispace1,
            tag("time"),
            multispace0,
        )),
    )(input)
}

fn get_date_parser(input: &str) -> IResult<&str, CliMsg, Error<&str>> {
    value(
        CliMsg::GetDate,
        tuple((
            multispace0,
            tag("get"),
            multispace1,
            tag("date"),
            multispace0,
        )),
    )(input)
}

fn hello_parser(input: &str) -> IResult<&str, CliMsg, Error<&str>> {
    value(
        CliMsg::Hello,
        tuple((multispace0, tag("hello"), multispace0)),
    )(input)
}

fn cli_parser(input: &str) -> IResult<&str, CliMsg, Error<&str>> {
    alt((
        hello_parser,
        get_time_parser,
        get_date_parser,
        set_time_parser,
        set_date_parser,
    ))(input)
}

pub async fn cli(line: &heapless::String<128>) -> heapless::String<128> {
    let mut out: heapless::String<128> = heapless::String::new();
    if line.is_empty() {
        return out;
    }
    match cli_parser(line.as_str()) {
        Ok((_, CliMsg::Hello)) => {
            out.push_str("Hello!").ok();
        }
        Ok((_, CliMsg::GetTime)) => {
            let mut rtc_time_rx = crate::RTC_TIME.receiver().unwrap();
            let time = rtc_time_rx.get().await.time();
            write!(
                out,
                "{:02}:{:02}:{:02}",
                time.hour(),
                time.minute(),
                time.second()
            )
            .ok();
        }
        Ok((_, CliMsg::GetDate)) => {
            let mut rtc_time_rx = crate::RTC_TIME.receiver().unwrap();
            let date = rtc_time_rx.get().await.date();
            write!(
                out,
                "{:02}/{:02}/{:04}",
                date.day(),
                date.month(),
                date.year()
            )
            .ok();
        }
        Ok((_, CliMsg::SetTime(t))) => {
            let msg_pub = crate::MSG_BUS.publisher().unwrap();
            msg_pub.publish(crate::Msg::SetTime(t)).await;
            out.push_str("OK").ok();
        }
        Ok((_, CliMsg::SetDate(d))) => {
            let msg_pub = crate::MSG_BUS.publisher().unwrap();
            msg_pub.publish(crate::Msg::SetDate(d)).await;
            out.push_str("OK").ok();
        }
        Err(nom::Err::Error(e)) | Err(nom::Err::Failure(e)) => {
            out.push_str("Parse Error <<").ok();
            out.push_str(e.input).ok();
            out.push_str(">").ok();
        }
        Err(_) => {
            out.push_str("Error").ok();
        }
    }
    out
}
