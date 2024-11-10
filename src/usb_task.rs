use core::fmt::Write;
use defmt::{info, panic, Format};
use embassy_executor::Spawner;
use embassy_stm32::usb::{Driver, Instance};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    driver::EndpointError,
    Builder, UsbDevice,
};
use static_cell::StaticCell;

use crate::Irqs;

pub type UsbOtgPeripheral = embassy_stm32::peripherals::USB_OTG_FS;
pub type UsbOtgDmPin = embassy_stm32::peripherals::PA11;
pub type UsbOtgDpPin = embassy_stm32::peripherals::PA12;

static STATE: StaticCell<State> = StaticCell::new();
static CLASS: StaticCell<CdcAcmClass<'static, Driver<'static, UsbOtgPeripheral>>> =
    StaticCell::new();

#[embassy_executor::task]
pub async fn usb_device(
    spawner: Spawner,
    usb_otg: UsbOtgPeripheral,
    usb_dp: UsbOtgDpPin,
    usb_dm: UsbOtgDmPin,
) {
    let mut config = embassy_stm32::usb::Config::default();

    // Do not enable vbus_detection. This is a safe default that works in all boards.
    // However, if your USB device is self-powered (can stay powered on if USB is unplugged), you need
    // to enable vbus_detection to comply with the USB spec. If you enable it, the board
    // has to support it or USB won't work at all. See docs on `vbus_detection` for details.
    config.vbus_detection = false;

    static EP_OUT_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();
    let driver = Driver::new_fs(
        usb_otg,
        Irqs,
        usb_dp,
        usb_dm,
        EP_OUT_BUFFER.init([0; 256]),
        config,
    );

    // Create embassy-usb Config
    let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("USB-Serial");
    config.serial_number = Some("12345678");

    // Required for windows compatibility.
    // https://developer.nordicsemi.com/nRF_Connect_SDK/doc/1.9.1/kconfig/CONFIG_CDC_ACM_IAD.html#help
    config.device_class = 0xEF;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;
    config.composite_with_iads = true;

    // Create embassy-usb DeviceBuilder using the driver and config.
    // It needs some buffers for building the descriptors.
    static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESCRIPTOR.init([0; 256]),
        BOS_DESCRIPTOR.init([0; 256]),
        &mut [], // no msos descriptors
        CONTROL_BUF.init([0; 64]),
    );

    // Create classes on the builder.
    let state = STATE.init(State::new());
    let class = CLASS.init(CdcAcmClass::new(&mut builder, state, 64));

    // Build the builder.
    let usb = builder.build();

    // Run USB Device
    spawner.spawn(usb_task(usb)).unwrap();

    loop {
        class.wait_connection().await;
        info!("Connected");
        match serial_handler(class).await {
            Ok(_) => info!("OK"),
            Err(e) => info!("ERROR: {:?}", e),
        };
        info!("Disconnected");
    }
}

#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice<'static, Driver<'static, UsbOtgPeripheral>>) -> ! {
    usb.run().await
}

#[derive(Format)]
struct Disconnected {}

impl From<EndpointError> for Disconnected {
    fn from(val: EndpointError) -> Self {
        match val {
            EndpointError::BufferOverflow => panic!("Buffer overflow"),
            EndpointError::Disabled => Disconnected {},
        }
    }
}

const NL: [u8; 1] = [b'\n'];
const CRNL: [u8; 2] = [b'\r', b'\n'];
const PROMPT: [u8; 8] = [0x1b, b'[', b'2', b'K', b'\r', b'>', b'>', b' '];

async fn serial_handler<'a, T: Instance + 'a>(
    class: &mut CdcAcmClass<'a, Driver<'a, T>>,
) -> Result<(), Disconnected> {
    let mut buf = [0; 128];
    let mut line_buffer: heapless::String<128> = heapless::String::new();
    loop {
        let n = class.read_packet(&mut buf).await?;
        let data = &buf[..n];
        for c in data.utf8_chunks() {
            match c.valid() {
                "\n" | "\r" => {
                    info!(
                        "Line >>{}<< [{}] {}",
                        line_buffer.as_str(),
                        line_buffer.len(),
                        line_buffer.as_bytes()
                    );
                    class.write_packet(&CRNL).await?;
                    let mut has_output = false;
                    for pkt in cmd_handler(&line_buffer).await.as_bytes().chunks(64) {
                        has_output = true;
                        class.write_packet(pkt).await?;
                    }
                    if has_output {
                        class.write_packet(&NL).await?;
                    }
                    class.write_packet(&PROMPT).await?;
                    line_buffer.clear();
                }
                "\t" => info!(">> TAB"),
                "\x7f" | "\x08" => {
                    line_buffer.pop();
                }
                s => {
                    // info!("Chunk: {}", s.as_bytes());
                    line_buffer.push_str(s).ok();
                }
            }
        }
        class.write_packet(&PROMPT).await?;
        for pkt in line_buffer.as_bytes().chunks(64) {
            class.write_packet(pkt).await?;
        }
    }
}

async fn cmd_handler(line: &heapless::String<128>) -> heapless::String<128> {
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
