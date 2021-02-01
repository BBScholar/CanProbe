#![no_std]

use serde::{Deserialize, Serialize};

use getset::{Getters, Setters};

pub const VENDOR_ID: u16 = 0x69;
pub const CMD_PACKET_SIZE: usize = 16;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Setters, Getters)]
pub struct AdaptorSettings {
    #[getset(get, set)]
    update_freq: f32,

    #[getset(get, set)]
    rx_mask: u32,

    #[getset(get, set)]
    leds: bool,

    #[getset(get, set)]
    can_id: u32,
}

impl Default for AdaptorSettings {
    fn default() -> Self {
        Self {
            update_freq: 1000.0,
            rx_mask: 0,
            leds: true,
            can_id: 0,
        }
    }
}

// Can Frame impl
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct CANFrame {
    pub id: u32,
    pub dlc: u8,
    pub data: [u8; 8],
    pub is_rtr: bool,
    pub is_err: bool,
    pub is_ext: bool,
}

impl CANFrame {
    pub fn new(id: u32, dlc: u8, data: [u8; 8], is_rtr: bool, is_err: bool, is_ext: bool) -> Self {
        Self {
            id,
            dlc,
            data,
            is_rtr,
            is_err,
            is_ext,
        }
    }
}
