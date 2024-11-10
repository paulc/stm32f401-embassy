use defmt::{info, Format};
use embassy_stm32::usb::{Driver, Instance};
use embassy_usb::{class::cdc_acm::CdcAcmClass, driver::EndpointError};

const NL: [u8; 1] = [b'\n'];
const CRNL: [u8; 2] = [b'\r', b'\n'];
const PROMPT: [u8; 8] = [0x1b, b'[', b'2', b'K', b'\r', b'>', b'>', b' '];

#[derive(Format)]
pub struct Disconnected {}

impl From<EndpointError> for Disconnected {
    fn from(val: EndpointError) -> Self {
        match val {
            EndpointError::BufferOverflow => defmt::panic!("Buffer overflow"),
            EndpointError::Disabled => Disconnected {},
        }
    }
}

pub async fn line_input<'a, T: Instance + 'a>(
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
                    for pkt in crate::cli::cli(&line_buffer).await.as_bytes().chunks(64) {
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
