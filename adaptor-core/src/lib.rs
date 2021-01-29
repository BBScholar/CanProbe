#![feature(negative_impls)]

use rusb;

use common::*;

use quick_error::quick_error;

quick_error! {
    #[derive(Debug)]
    pub enum AdaptorError {
        RusbError {
            from(rusb::Error)
        }
        NoDeviceError {}
        NoEndpointError {}
        ConnectionError {}
        SettingsError {}
    }
}

pub type Result<T> = std::result::Result<T, AdaptorError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AdaptorSettings;

pub struct AdaptorHandle {
    handle: rusb::DeviceHandle<rusb::Context>,
    settings: AdaptorSettings,
}

// not completely sure, but I dont think the usb handle
// is thread safe
// Plus, an extra mutex never hurt anyone
impl !Sync for AdaptorHandle {}

impl AdaptorHandle {
    pub fn new_with_default_settings() -> Result<Self> {
        let settings = AdaptorSettings::default();
        Ok(Self::new(settings)?)
    }

    pub fn new(settings: AdaptorSettings) -> Result<Self> {
        use rusb::UsbContext;
        let context = rusb::Context::new()?;
        let device = context
            .devices()?
            .iter()
            .find_map(|d| {
                if let Ok(desc) = d.device_descriptor() {
                    if desc.vendor_id() == VID && desc.product_id() == PID {
                        Some(d)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .map_or(Err(AdaptorError::NoDeviceError), Ok)?;

        let mut handle = device.open()?;
        let config = device.active_config_descriptor()?;
        let descriptor = device.device_descriptor()?;

        handle.claim_interface(0)?;

        let mut has_in_ep = false;
        let mut has_out_ep = false;

        if let Some(iface) = config.interfaces().next() {
            if let Some(desc) = iface.descriptors().next() {
                for endpoint in desc.endpoint_descriptors() {
                    let addr = endpoint.address();
                    if addr == OUT_ENDPOINT {
                        has_out_ep = true;
                    } else if addr == IN_ENDPOINT {
                        has_in_ep = true;
                    }
                }
            }
        }

        if !has_in_ep {
            return Err(AdaptorError::NoEndpointError);
        }

        if !has_out_ep {
            return Err(AdaptorError::NoEndpointError);
        }

        Ok(Self { handle, settings })
    }

    pub fn settings(&self) -> &AdaptorSettings {
        &self.settings
    }

    pub fn config_settings<F: Fn(&mut AdaptorSettings) -> ()>(&mut self, f: F) {
        f(&mut self.settings);
        self.send_settings();
    }

    fn send_settings(&self) -> Result<()> {
        Ok(())
    }

    fn write(&mut self, cmd: &[u8], timeout: std::time::Duration) -> Result<()> {
        assert!(cmd.len() <= CMD_PACKET_SIZE);
        let mut padded_cmd = [0_u8; CMD_PACKET_SIZE];
        padded_cmd[..cmd.len()].copy_from_slice(cmd);

        let written_bytes = self.handle.write_bulk(OUT_ENDPOINT, &padded_cmd, timeout)?;

        if written_bytes != CMD_PACKET_SIZE {
            // create an actual error type for this
            return Err(AdaptorError::SettingsError);
        }

        Ok(())
    }

    pub fn write_frame(&mut self, frame: CANFrame) -> Result<()> {
        let buf: heapless::Vec<u8, heapless::consts::U1024> = postcard::to_vec(&frame).expect("");

        self.handle
            .write_bulk(OUT_ENDPOINT, &buf, std::time::Duration::from_secs(1))?;

        Ok(())
    }
}

impl Drop for AdaptorHandle {
    fn drop(&mut self) {
        self.handle.release_interface(0);
    }
}
