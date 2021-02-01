use crate::AdaptorError;
use crate::Result;

use common::{AdaptorSettings, CANFrame};
use common::{CMD_PACKET_SIZE, VENDOR_ID};

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
    pub const VID: u16 = common::VENDOR_ID;
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
use std::sync::{atomic::AtomicBool, Arc};
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

    rx_thread_running: Arc<AtomicBool>,
    rx_poll_handle: std::thread::JoinHandle<()>,

    rx_callback: Option<Box<dyn Fn(CANFrame)>>,
    // #[cfg(not(feature = "threaded"))]
    // phantom: std::marker::PhantomData<&'a ()>,
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
        let mut has_swo_ep = false;

        if let Some(iface) = config.interfaces().next() {
            if let Some(desc) = iface.descriptors().next() {
                for endpoint in desc.endpoint_descriptors() {
                    let addr = endpoint.address();
                    if addr == info.out_ep {
                        has_out_ep = true;
                    } else if addr == info.in_ep {
                        has_in_ep = true;
                    } else if addr == info.swo_ep {
                        has_swo_ep = true;
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

        if !has_swo_ep {
            return Err(AdaptorError::NoEndpointError);
        }

        #[cfg(feature = "threaded")]
        {
            let rx_callback = |frame: CANFrame| {};

            let rx_thread_running = Arc::new(AtomicBool::new(true));
            let internal_running = Arc::clone(rx_thread_running);
            let thread_handle = std::thread::spawn(move || {
                internal_running;
                while internal_running.load(std::sync::atomic::Ordering::Acquire) {
                    // check for frames
                }
            });
            Ok(Self {
                handle,
                settings,
                info,
                rx_thread_running,
                thread_handle,
            })
        }

        #[cfg(not(feature = "threaded"))]
        Ok(Self {
            handle,
            settings,
            info,
        })
    }

    #[cfg(feature = "threaded")]
    pub fn register_rx_callback<F: Fn(CANFrame)>(&mut self, f: F) {}

    pub fn config_settings<F: Fn(&mut AdaptorSettings) -> ()>(&mut self, f: F) -> Result<()> {
        f(&mut self.settings);
        self.send_settings()?;
        Ok(())
    }

    fn send_settings(&self) -> Result<()> {
        Ok(())
    }

    fn write(
        &mut self,
        cmd: &[u8],
        write_bytes: &[u8],
        timeout: std::time::Duration,
    ) -> Result<()> {
        assert!(cmd.len() <= CMD_PACKET_SIZE);
        let mut padded_cmd = [0_u8; CMD_PACKET_SIZE];
        padded_cmd[..cmd.len()].copy_from_slice(cmd);

        let written_bytes = self
            .handle
            .write_bulk(self.info.out_ep, &padded_cmd, timeout)?;

        if written_bytes != CMD_PACKET_SIZE {
            // create an actual error type for this
            return Err(AdaptorError::SettingsError);
        }

        if write_bytes.len() > 0 {
            let _written_bytes = self
                .handle
                .write_bulk(self.info.out_ep, &write_bytes, timeout)?;

            if written_bytes <= write_bytes.len() {
                // throw error
            }
        }

        Ok(())
    }

    pub fn write_frame(&mut self, frame: CANFrame) -> Result<()> {
        let mut buf = Vec::with_capacity(1024);
        postcard::to_slice(&frame, buf.as_mut_slice()).expect("Buffer is the wrong size");

        let cmd = [0_u8; 16];
        self.write(&cmd, &buf, std::time::Duration::from_secs(1))?;

        Ok(())
    }

    // TODO: We may want to make a seperate command that sends an entire batch
    pub fn write_frames<T: IntoIterator<Item = CANFrame>>(&mut self, t: T) -> Result<usize> {
        let mut count = 0;

        for frame in t.into_iter() {
            self.write_frame(frame)?;
            count += 1;
        }

        Ok(count)
    }

    fn read(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn read_frame(&mut self) -> Result<CANFrame> {
        let buf = Vec::new();

        self.read()?;

        let frame: CANFrame = postcard::from_bytes(&buf[..])?;
        Ok(frame)
    }

    pub fn reset(&mut self) -> Result<()> {
        Ok(self.handle.reset()?)
    }
}

impl Drop for AdaptorHandle {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(0);
    }
}
