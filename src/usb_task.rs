use defmt::{panic, *};
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
    static STATE: StaticCell<State> = StaticCell::new();
    let state = STATE.init(State::new());
    let mut class = CdcAcmClass::new(&mut builder, state, 64);

    // Build the builder.
    let usb = builder.build();

    // Run the USB device.
    // let usb_fut = usb.run();

    // Run USB Device
    spawner.spawn(usb_task(usb)).unwrap();

    // Do stuff with the class!
    loop {
        class.wait_connection().await;
        info!("Connected");
        match echo(&mut class).await {
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

async fn echo<'d, T: Instance + 'd>(
    class: &mut CdcAcmClass<'d, Driver<'d, T>>,
) -> Result<(), Disconnected> {
    let mut buf = [0; 64];
    loop {
        let n = class.read_packet(&mut buf).await?;
        let data = &buf[..n];
        info!("data: {:x}", data);
        class.write_packet(data).await?;
    }
}
