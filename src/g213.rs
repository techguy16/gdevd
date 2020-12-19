use crate::usb_ext::DetachedHandle;
use crate::{Command, CommandError, CommandResult, GDevice, GDeviceModel, RgbColor, Speed};
use quick_error::ResultExt;
use rusb::{Context, Device, DeviceHandle, DeviceList, UsbContext};
use std::thread::sleep;
use std::time::Duration;

// Standard color, i found this color to produce a white color on my G213
//const STANDARD_COLOR_HEX: &str = "ffb4aa";
// The id of the Logitech company
const ID_VENDOR: u16 = 0x046d;
// The id of the G213
const ID_PRODUCT: u16 = 0xc336;
// Endpoint to read data back from
const ENDPOINT_ADDRESS: u8 = 0x82;
// --.
const REQUEST_TYPE: u8 = 0x21;
//    \ The control transfer
const REQUEST: u8 = 0x09;
//    / configuration for the G213
const VALUE: i32 = 0x0211;
// --'
const INTERFACE: u8 = 0x0001;

pub struct G213Model;

impl G213Model {
    pub fn new() -> Self {
        Self
    }
}

impl Default for G213Model {
    fn default() -> Self {
        Self
    }
}

impl G213Model {
    fn try_open_device(device: &Device<Context>) -> CommandResult<Box<dyn GDevice>> {
        Ok(Box::new(G213Device {
            handle: device.open().context("opening G213 USB device")?,
        }))
    }

    fn open_device(device: &Device<Context>) -> Option<Box<dyn GDevice>> {
        Self::try_open_device(device)
            .map_err(|err| {
                warn!("Failed to open G213 device: {:?}", err);
                err
            })
            .ok()
    }
}

impl GDeviceModel for G213Model {
    fn find(&self, devices: &DeviceList<Context>) -> Vec<Box<dyn GDevice>> {
        devices
            .iter()
            .filter(|device| {
                let device_descriptor = device.device_descriptor().unwrap();
                device_descriptor.product_id() == ID_PRODUCT
                    && device_descriptor.vendor_id() == ID_VENDOR
            })
            .flat_map(|device| Self::open_device(&device))
            .collect()
    }

    fn get_sectors(&self) -> u8 {
        5
    }

    fn get_default_color(&self) -> RgbColor {
        RgbColor(0xff, 0xb4, 0xaa)
    }

    fn get_name(&self) -> &'static str {
        "G213"
    }
}

pub struct G213Device {
    handle: DeviceHandle<Context>,
}

impl G213Device {
    fn send_data<'t, T: UsbContext>(handle: &mut DeviceHandle<T>, data: &str) -> CommandResult<()> {
        handle
            .write_control(
                REQUEST_TYPE,
                REQUEST,
                VALUE as u16,
                INTERFACE as u16,
                &hex::decode(data).unwrap(),
                Duration::from_secs(0),
            )
            .context("write_control")?;

        let mut data = [0u8; 20];
        handle
            .read_interrupt(ENDPOINT_ADDRESS, &mut data, Duration::from_secs(60))
            .context("read_interrupt")?;

        Ok(())
    }
}

fn check_speed(speed: Speed) -> CommandResult<()> {
    if speed.0 < 32 {
        Err(CommandError::InvalidArgument(
            "speed",
            format!("{} < 32", speed.0),
        ))
    } else {
        Ok(())
    }
}

impl GDevice for G213Device {
    fn get_debug_info(&self) -> String {
        unimplemented!()
    }

    fn send_command(&mut self, cmd: Command) -> CommandResult<()> {
        use Command::*;

        let mut handle = DetachedHandle::new(&mut self.handle, INTERFACE)
            .context("detaching USB device from kernel")?;
        match cmd {
            ColorSector(rgb, sector) => {
                if let Some(sector) = sector {
                    if sector > 4 {
                        return Err(CommandError::InvalidArgument(
                            "sector",
                            format!("{} > 4", sector),
                        ));
                    }
                    Self::send_data(
                        &mut handle,
                        &format!(
                            "11ff0c3a{:02x}01{:06x}0200000000000000000000",
                            sector + 1,
                            rgb.to_int()
                        ),
                    )
                } else {
                    Self::send_data(
                        &mut handle,
                        &format!(
                            "11ff0c3a{:02x}01{:06x}0200000000000000000000",
                            0,
                            rgb.to_int()
                        ),
                    )
                }
            }
            Breathe(rgb, speed) => {
                check_speed(speed)?;

                Self::send_data(
                    &mut handle,
                    &format!(
                        "11ff0c3a0002{:06x}{:04x}006400000000000000",
                        rgb.to_int(),
                        speed.0
                    ),
                )
            }
            Cycle(speed) => {
                check_speed(speed)?;

                Self::send_data(
                    &mut handle,
                    &format!("11ff0c3a0003ffffff0000{:04x}64000000000000", speed.0),
                )
            }
        }
    }
}
