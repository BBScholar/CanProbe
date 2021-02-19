use crate::AdaptorError;
use crate::Result;

use adaptor_common::{AdaptorSettings, CANFrame, UsbRequests};
use adaptor_common::{CMD_PACKET_SIZE, VENDOR_ID};

use lazy_static::lazy_static;

use rusb;

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(crate) struct AdaptorInfo {
    pub version: String,
    pub pid: u16,
    pub in_ep: u8,
    pub out_ep: u8,
    pub swo_ep: u8,
}

impl AdaptorInfo {
    pub const VID: u16 = adaptor_common::VENDOR_ID;
    pub(crate) fn new(version: String, pid: u16, in_ep: u8, out_ep: u8, swo_ep: u8) -> Self {
        Self {
            version,
            pid,
            in_ep,
            out_ep,
            swo_ep,
        }
    }
}

lazy_static! {
    /// Map of the versions of the probe hardware/firmware
    /// since we may use different end points and product ids
    /// in new versions
    pub(crate) static ref VERSIONS: HashMap<u16, AdaptorInfo> = {
        let mut m = HashMap::new();
        m.insert(
            0x69,
            AdaptorInfo::new("V1".to_owned(), 0x69, 0x1, 0x81, 0x82),
        );
        m
    };
}

use getset::{Getters, Setters};

/// Adaptor handle struct
/// this is more or less a wrapper around `rusb::DeviceHandle`
/// with aditional fields for settings and info
#[derive(Getters, Setters)]
pub struct AdaptorHandle {
    handle: rusb::DeviceHandle<rusb::Context>,

    #[getset(get)]
    settings: AdaptorSettings,

    #[getset(get)]
    info: AdaptorInfo,

    #[getset(get)]
    running: bool,
}

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
            .filter(|d| {
                if let Ok(desc) = d.device_descriptor() {
                    desc.vendor_id() == AdaptorInfo::VID
                        && VERSIONS.contains_key(&desc.product_id())
                } else {
                    false
                }
            })
            .find_map(|d| {
                if let Ok(desc) = d.device_descriptor() {
                    if desc.vendor_id() == AdaptorInfo::VID {
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

        let info = VERSIONS
            .get(&descriptor.product_id())
            .expect("This should never happen")
            .clone();

        handle.claim_interface(0)?;

        let mut has_in_ep = false;
        let mut has_out_ep = false;

        if let Some(iface) = config.interfaces().next() {
            if let Some(desc) = iface.descriptors().next() {
                for endpoint in desc.endpoint_descriptors() {
                    let addr = endpoint.address();
                    if addr == info.out_ep {
                        has_out_ep = true;
                    } else if addr == info.in_ep {
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

        let settings = AdaptorSettings::default();

        let temp = Self {
            handle,
            settings,
            info,
            running: false,
        };

        temp.write_settings(std::time::Duration::from_secs_f32(1.0))?;

        Ok(temp)
    }

    pub fn read_frame(&mut self, timeout: std::time::Duration) -> Result<CANFrame> {
        let mut buffer = [0_u8; 256];
        let _bytes = self
            .handle
            .read_bulk(self.info.in_ep, &mut buffer, timeout)?;

        let (frame, _) = postcard::take_from_bytes(&buffer)?;

        log::trace!("Recieved frame: {:?}", frame);

        Ok(frame)
    }

    pub fn write_frame(&mut self, frame: CANFrame, timeout: std::time::Duration) -> Result<()> {
        let vec = postcard::to_stdvec(&frame)?;
        let bytes = self
            .handle
            .write_bulk(self.info.out_ep, vec.as_slice(), timeout)?;
        log::info!("wrote {:?} bytes", bytes);
        Ok(())
    }

    pub fn set_settings(
        &mut self,
        settings: AdaptorSettings,
        timeout: std::time::Duration,
    ) -> Result<()> {
        self.modify_settings(timeout, |s| *s = settings)
    }

    pub fn modify_settings<F>(&mut self, timeout: std::time::Duration, f: F) -> Result<()>
    where
        F: Fn(&mut AdaptorSettings) -> (),
    {
        f(&mut self.settings);
        self.write_settings(timeout)
    }

    fn write_settings(&self, timeout: std::time::Duration) -> Result<()> {
        use rusb::{Direction, Recipient, RequestType};
        let req_type = rusb::request_type(Direction::Out, RequestType::Vendor, Recipient::Device);
        let vec = postcard::to_stdvec(&self.settings)?;
        self.handle.write_control(
            req_type,
            UsbRequests::Settings.into(),
            0x00,
            0x00,
            vec.as_slice(),
            timeout,
        )?;
        Ok(())
    }

    fn write_running(&mut self, running: bool, timeout: std::time::Duration) -> Result<()> {
        use rusb::{Direction, Recipient, RequestType};
        let req_type = rusb::request_type(Direction::Out, RequestType::Vendor, Recipient::Device);
        self.running = running;
        self.handle.write_control(
            req_type,
            UsbRequests::Run.into(),
            0x00,
            0x00,
            &[running as u8],
            timeout,
        )?;
        Ok(())
    }

    #[inline]
    pub fn start(&mut self, timeout: std::time::Duration) -> Result<()> {
        self.write_running(true, timeout)
    }

    #[inline]
    pub fn stop(&mut self, timeout: std::time::Duration) -> Result<()> {
        self.write_running(false, timeout)
    }

    pub fn reset(&self, timeout: std::time::Duration) -> Result<()> {
        use rusb::{Direction, Recipient, RequestType};
        let req_type = rusb::request_type(Direction::Out, RequestType::Vendor, Recipient::Device);
        let _bytes =
            self.handle
                .write_control(req_type, UsbRequests::Reset.into(), 0, 0, &[], timeout)?;
        Ok(())
    }

    pub fn get_error(&self, timeout: std::time::Duration) -> Result<u8> {
        use rusb::{Direction, Recipient, RequestType};
        let req_type = rusb::request_type(Direction::In, RequestType::Vendor, Recipient::Device);
        let mut buf = [0_u8];
        let bytes = self.handle.read_control(
            req_type,
            UsbRequests::GetError.into(),
            0x00,
            0x00,
            &mut buf,
            timeout,
        )?;

        debug_assert!(bytes == 1);

        Ok(buf[0])
    }
}

impl Drop for AdaptorHandle {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(0);
    }
}
