#![no_std]

use serde::{Deserialize, Serialize};

// device parameters. These wont change for now
pub const CMD_PACKET_SIZE: usize = 16;

pub const VID: u16 = 0x69;
pub const PID: u16 = 0x69;

pub const IN_ENDPOINT: u8 = 0x1;
pub const OUT_ENDPOINT: u8 = 0x81;

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
