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

struct LineInput<const N: usize> {
    buf: heapless::String<N>,
    escape_buf: heapless::String<8>,
    escape: bool,
    cursor_pos: usize,
}

impl<const N: usize> LineInput<N> {
    fn new() -> Self {
        LineInput {
            buf: heapless::String::new(),
            escape_buf: heapless::String::new(),
            escape: false,
            cursor_pos: 0,
        }
    }
    fn insert(&mut self, c: char) -> Result<(), ()> {
        if self.cursor_pos == self.buf.len() {
            self.buf.push(c).and_then(|_| {
                self.cursor_pos += 1;
                Ok(())
            })
        } else {
            let mut out: heapless::String<N> = heapless::String::new();
            for (i, cx) in self.buf.chars().enumerate() {
                if i < self.cursor_pos {
                    out.push(cx)?;
                } else if i == self.cursor_pos {
                    out.push(c)?;
                    out.push(cx)?;
                    self.cursor_pos += 1;
                } else {
                    out.push(cx)?;
                }
            }
            Ok(())
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
        let mut escape = false;
        let mut escape_buf: heapless::String<8> = heapless::String::new();
        for chunk in data.utf8_chunks() {
            for (_, c) in chunk.valid().chars().enumerate() {
                match c {
                    '\n' | '\r' => {
                        // EOL - handle line
                        info!(
                            "Line >>{}<< [{}] {}",
                            line_buffer.as_str(),
                            line_buffer.len(),
                            line_buffer.as_bytes()
                        );
                        if n > 0 {
                            // If CR in chunk write line buffer now
                            class.write_packet(&PROMPT).await?;
                            for pkt in line_buffer.as_bytes().chunks(64) {
                                class.write_packet(pkt).await?;
                            }
                        }
                        // Call cli parser and write response
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
                        // Clear line buffer
                        line_buffer.clear();
                        // Clear ESC flag if needed
                        escape = false;
                    }
                    '\x7f' | '\x08' => {
                        // BS
                        line_buffer.pop();
                    }
                    '\t' => {
                        // Ignore TAB
                    }
                    '\x1b' => {
                        // Escape
                        escape_buf.clear();
                        escape = true;
                    }
                    '\x00'..='\x1f' => {
                        // Ignore control characters
                    }
                    c => {
                        if escape {
                            escape_buf.push(c).ok();
                            if c.is_ascii_alphabetic() {
                                escape = false;
                                info!("ESCAPE: {}", escape_buf.as_str());
                            }
                        } else {
                            line_buffer.push(c).ok();
                        }
                    }
                }
                info!(
                    "Line: <{}> {}",
                    line_buffer.as_str(),
                    line_buffer.as_bytes()
                );
            }
        }
        class.write_packet(&PROMPT).await?;
        for pkt in line_buffer.as_bytes().chunks(64) {
            class.write_packet(pkt).await?;
        }
    }
}
