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

/*
const EP_BUFFER_LEN: usize = 64;

use core::future::Future;

struct LineInput<'a, const N: usize, const M: usize, T, F, Fut>
where
    T: Instance + 'a,
    F: for<'r> Fn(&'r heapless::String<N>) -> Fut,
    Fut: Future<Output = heapless::String<M>>,
{
    buf: heapless::String<N>,
    escape_buf: heapless::String<8>,
    escape: bool,
    cursor_pos: usize,
    endpoint: Option<&'a mut CdcAcmClass<'a, Driver<'a, T>>>,
    f: F,
}

impl<'a, const N: usize, const M: usize, T, F, Fut> LineInput<'a, N, M, T, F, Fut>
where
    T: Instance + 'a,
    F: Fn(&heapless::String<N>) -> Fut,
    Fut: Future<Output = heapless::String<M>>,
{
    fn new(endpoint: &'a mut CdcAcmClass<'a, Driver<'a, T>>, f: F) -> Self {
        LineInput {
            buf: heapless::String::new(),
            escape_buf: heapless::String::new(),
            escape: false,
            cursor_pos: 0,
            endpoint: Some(endpoint),
            f,
        }
    }

    fn release(&mut self) -> Option<&'a mut CdcAcmClass<'a, Driver<'a, T>>> {
        self.endpoint.take()
    }

    async fn input(&mut self) -> Result<(), ()> {
        let mut buf = [0; 128];
        loop {
            let n = self.read(&mut buf).await?;
            let data = &buf[..n];
            for chunk in data.utf8_chunks() {
                for (_, c) in chunk.valid().chars().enumerate() {
                    match c {
                        c => {
                            info!(">> {}", c);
                            self.write(&[b'>', b'>']).await?;
                            self.write(&[c as u8]).await?;
                            self.write(&[b'\r', b'\n']).await?;
                        }
                    }
                }
            }
        }
    }

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, ()> {
        if let Some(ep) = self.endpoint.as_mut() {
            Ok(ep.read_packet(buf).await.map_err(|_| ())?)
        } else {
            Err(())
        }
    }

    async fn write(&mut self, buf: &[u8]) -> Result<(), ()> {
        for pkt in buf.chunks(64) {
            if let Some(ep) = self.endpoint.as_mut() {
                ep.write_packet(pkt).await.map_err(|_| ())?;
            }
        }
        Ok(())
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

    fn chunks(&self, n: usize) -> core::slice::Chunks<u8> {
        self.buf.as_bytes().chunks(n)
    }

    fn pop(&mut self) {
        if let Some(_) = self.buf.pop() {
            self.cursor_pos -= 1;
        }
    }

    fn clear(&mut self) {
        self.buf.clear();
        self.escape_buf.clear();
        self.escape = false;
        self.cursor_pos = 0;
    }
    /*
    fn push(&mut self, c: char) -> Result<
        let mut out: Option<heapless::String<N>> = None;
                match c {
                    '\n' | '\r' => {
                        if n > 0 {
                            // If CR in chunk write line buffer now
                            class.write_packet(&PROMPT).await?;
                            for pkt in line.chunks(64) {
                                class.write_packet(pkt).await?;
                            }
                        }
                        // Call cli parser and write response
                        class.write_packet(&CRNL).await?;
                        let mut has_output = false;
                        for pkt in crate::cli::cli(&line.buf).await.as_bytes().chunks(64) {
                            has_output = true;
                            class.write_packet(pkt).await?;
                        }
                        if has_output {
                            class.write_packet(&NL).await?;
                        }
                        class.write_packet(&PROMPT).await?;
                        // Clear line buffer
                        line.clear();
                    }
                    '\x7f' | '\x08' => {
                        // BS
                        line.pop();
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
                            line_buffer.buf.push(c).ok();
                        }
                    }
        Err(())
        }
    */
}
pub async fn line_input<'a, T: Instance + 'a>(
    class: &'a mut CdcAcmClass<'a, Driver<'a, T>>,
) -> Result<(), Disconnected> {
    let _input = LineInput::new(class, crate::cli::cli);
    Ok(())
}
*/

pub async fn line_input<'a, T: Instance + 'a>(
    class: &mut CdcAcmClass<'a, Driver<'a, T>>,
) -> Result<(), Disconnected> {
    let mut buf = [0; 128];
    let mut line_buffer: heapless::String<128> = heapless::String::new();
    let mut escape = false;
    let mut escape_buf: heapless::String<8> = heapless::String::new();
    loop {
        let n = class.read_packet(&mut buf).await?;
        let data = &buf[..n];
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
                /*
                info!(
                    "Input: <{}> {} [{} <{}>]",
                    line_buffer.as_str(),
                    line_buffer.as_bytes(),
                    escape,
                    escape_buf.as_str(),
                );
                */
            }
        }
        // Rewrite full line
        class.write_packet(&PROMPT).await?;
        for pkt in line_buffer.as_bytes().chunks(64) {
            class.write_packet(pkt).await?;
        }
    }
}
