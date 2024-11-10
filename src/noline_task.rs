use defmt::*;
use embassy_stm32::usb::{Driver, Instance};
use embassy_usb::class::cdc_acm::CdcAcmClass;
use noline::builder::EditorBuilder;

struct Io<'a, T: Instance + 'a> {
    class: &'a mut CdcAcmClass<'a, Driver<'a, T>>,
    buf: heapless::Deque<u8, 64>,
}

impl<'a, T> Io<'a, T>
where
    T: Instance,
{
    fn new(class: &'a mut CdcAcmClass<'a, Driver<'a, T>>) -> Self {
        return Io {
            class,
            buf: heapless::Deque::new(),
        };
    }
}

#[derive(Debug)]
struct IoError(());

impl embedded_io_async::Error for IoError {
    fn kind(&self) -> embedded_io_async::ErrorKind {
        embedded_io_async::ErrorKind::Other
    }
}

impl<'a, T> embedded_io_async::ErrorType for Io<'a, T>
where
    T: Instance,
{
    type Error = IoError;
}

impl<'a, T> embedded_io_async::Read for Io<'a, T>
where
    T: Instance,
{
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        // Read into buffer
        while self.buf.is_empty() {
            let mut buf: [u8; 64] = [0; 64];
            let len = self
                .class
                .read_packet(&mut buf)
                .await
                .map_err(|_| IoError(()))?;
            for i in buf.iter().take(len) {
                self.buf.push_back(*i).expect("Buffer overflow");
            }
        }
        if let Some(b) = self.buf.pop_front() {
            buf[0] = b;
            Ok(1)
        } else {
            Err(IoError(()))
        }
    }
}

impl<'a, T> embedded_io_async::Write for Io<'a, T>
where
    T: Instance,
{
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.class
            .write_packet(buf)
            .await
            .map_err(|_| IoError(()))?;
        Ok(buf.len())
    }
    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

const MAX_LINE_SIZE: usize = 64;

async fn noline_handler<'a, T: Instance + 'a>(class: &'a mut CdcAcmClass<'a, Driver<'a, T>>) {
    let mut buffer = [0; MAX_LINE_SIZE];
    let mut history = [0; MAX_LINE_SIZE];

    let prompt = ">> ";
    class.wait_connection().await;
    info!("Connected");
    let mut io = Io::new(class);
    let mut editor = EditorBuilder::from_slice(&mut buffer)
        .with_slice_history(&mut history)
        .build_async(&mut io)
        .await
        .unwrap();

    while let Ok(line) = editor.readline(prompt, &mut io).await {
        info!("Line >>{}<<", line);
    }
    info!("Disconnected");
}
