use core::fmt::Write;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{char, multispace0, multispace1, one_of};
use nom::combinator::map_opt;
use nom::combinator::value;
use nom::error::Error;
use nom::sequence::{pair, tuple};
use nom::IResult;
// use defmt::info;

#[derive(Clone, Debug)]
enum CliMsg {
    Hello,
    GetTime,
    SetTime(u8, u8, u8),
}

fn digit_parser(input: &str) -> IResult<&str, u32, Error<&str>> {
    map_opt(
        pair(one_of("0123456789"), one_of("0123456789")),
        |(d1, d2)| {
            d1.to_digit(10)
                .and_then(|n1| d2.to_digit(10).and_then(|n2| Some(n1 * 10 + n2)))
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
            digit_parser,
            char(':'),
            digit_parser,
            char(':'),
            digit_parser,
        )),
        |(_, _, _, _, _, hh, _, mm, _, ss)| {
            if hh < 24 && mm < 60 && ss < 60 {
                Some(CliMsg::SetTime(hh as u8, mm as u8, ss as u8))
            } else {
                None
            }
        },
    )(input)
}

fn get_time_parser(input: &str) -> IResult<&str, CliMsg, Error<&str>> {
    value(
        CliMsg::GetTime,
        tuple((multispace0, tag("get"), multispace1, tag("time"))),
    )(input)
}

fn hello_parser(input: &str) -> IResult<&str, CliMsg, Error<&str>> {
    value(CliMsg::Hello, tuple((multispace0, tag("hello"))))(input)
}

fn cli_parser(input: &str) -> IResult<&str, CliMsg, Error<&str>> {
    alt((hello_parser, get_time_parser, set_time_parser))(input)
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
            let (h, m, s) = rtc_time_rx.get().await;
            write!(out, "{:02}:{:02}:{:02}", h, m, s).ok();
        }
        Ok((_, CliMsg::SetTime(h, m, s))) => {
            let msg_pub = crate::MSG_BUS.publisher().unwrap();
            msg_pub.publish(crate::Msg::SetTime(h, m, s)).await;
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
