use defmt::info;
use embassy_executor::Spawner;
use embassy_stm32::usb::Driver;
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    Builder, UsbDevice,
};
use static_cell::StaticCell;

use crate::Irqs;

pub type UsbOtgPeripheral = embassy_stm32::peripherals::USB_OTG_FS;
pub type UsbOtgDmPin = embassy_stm32::peripherals::PA11;
pub type UsbOtgDpPin = embassy_stm32::peripherals::PA12;

// USB EP Buffer
static EP_OUT_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();

// Create embassy-usb DeviceBuilder using the driver and config.
// It needs some buffers for building the descriptors.
static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static MSOS_DESCRIPTOR: StaticCell<[u8; 0]> = StaticCell::new(); // No MSOS descriptor
static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

static STATE: StaticCell<State> = StaticCell::new();

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

    let driver = Driver::new_fs(
        usb_otg,
        Irqs,
        usb_dp,
        usb_dm,
        EP_OUT_BUFFER.init([0; 256]),
        config,
    );

    // embassy-usb config
    let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("USB-Serial");
    config.serial_number = Some("_stm32");

    // Required for windows compatibility.
    // https://developer.nordicsemi.com/nRF_Connect_SDK/doc/1.9.1/kconfig/CONFIG_CDC_ACM_IAD.html#help
    config.device_class = 0xEF;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;
    config.composite_with_iads = true;

    // USB device builder
    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESCRIPTOR.init([0; 256]),
        BOS_DESCRIPTOR.init([0; 256]),
        MSOS_DESCRIPTOR.init([]),
        CONTROL_BUF.init([0; 64]),
    );

    // Create CDC ACM class on the builder.
    let state = STATE.init(State::new());
    let class = CdcAcmClass::new(&mut builder, state, 64);

    // Build the USB task
    let usb = builder.build();

    // Run USB Device
    spawner.spawn(usb_task(usb)).unwrap();
    spawner.spawn(cdc_acm_task(class)).unwrap();
}

#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice<'static, Driver<'static, UsbOtgPeripheral>>) -> ! {
    usb.run().await
}

#[embassy_executor::task]
async fn cdc_acm_task(mut class: CdcAcmClass<'static, Driver<'static, UsbOtgPeripheral>>) -> ! {
    loop {
        class.wait_connection().await;
        info!("Connected");
        match crate::line_input::line_input(&mut class).await {
            Ok(_) => info!("CDC_ADM ok"),
            Err(e) => info!("CDC_ACM error: {:?}", e),
        };
        info!("Disconnected");
    }
}
