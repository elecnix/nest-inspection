//! Nest Backplate Protocol Implementation
//! 
//! Based on the protocol documentation from cuckoo-nest/wiki

use bytes::{Buf, BufMut, BytesMut};
use crc::{Crc, CRC_16_XMODEM};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const CRC_CCITT: Crc<u16> = Crc::<u16>::new(&CRC_16_XMODEM);

pub const COMMAND_PREAMBLE: &[u8] = &[0xd5, 0xaa, 0x96];
pub const RESPONSE_PREAMBLE: &[u8] = &[0xd5, 0xd5, 0xaa, 0x96];

// Command IDs
pub const CMD_RESET: u16 = 0x00ff;
pub const CMD_FET_PRESENCE_ACK: u16 = 0x008f;
pub const CMD_PERIODIC_STATUS: u16 = 0x0083;
pub const CMD_GET_MONO_TFE_ID: u16 = 0x0090;
pub const CMD_GET_MONO_TFE_VERSION: u16 = 0x0098;
pub const CMD_GET_MONO_TFE_BUILD_INFO: u16 = 0x0099;
pub const CMD_GET_BP_MODEL: u16 = 0x009d;
pub const CMD_GET_BSL_VERSION: u16 = 0x009b;
pub const CMD_GET_COPROCESSOR_BSL: u16 = 0x009c;
pub const CMD_GET_SERIAL_NUMBER: u16 = 0x009f;
pub const CMD_GET_HARDWARE_VERSION: u16 = 0x009e;
pub const CMD_SET_POWER_STEAL_MODE: u16 = 0x00c0;
pub const CMD_REQUEST_BUFFERS: u16 = 0x00a2;
pub const CMD_ACK_BUFFERS: u16 = 0x00a3;

// Response IDs
pub const RESP_TEMPERATURE: u16 = 0x0002;
pub const RESP_BUFFERED_TEMP: u16 = 0x0022;
pub const RESP_BUFFERED_SOURCE_TEMP: u16 = 0x0023;
pub const RESP_AMBIENT_LIGHT: u16 = 0x000c;
pub const RESP_END_BUFFERS: u16 = 0x002f;
pub const RESP_ASCII_MESSAGE: u16 = 0x0001;
pub const RESP_MONO_TFE_ID: u16 = 0x0010;
pub const RESP_MONO_TFE_VERSION: u16 = 0x0018;
pub const RESP_MONO_TFE_BUILD_INFO: u16 = 0x0019;
pub const RESP_BP_MODEL: u16 = 0x001d;
pub const RESP_BSL_VERSION: u16 = 0x001b;
pub const RESP_COPROCESSOR_BSL: u16 = 0x001c;
pub const RESP_SERIAL_NUMBER: u16 = 0x001f;
pub const RESP_HARDWARE_VERSION: u16 = 0x001e;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub id: u16,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: u16,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureReading {
    pub temperature_celsius: f32,
    pub humidity_percent: f32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackplateInfo {
    pub version: String,
    pub build_timestamp: String,
    pub serial_number: String,
    pub hardware_version: String,
    pub model: String,
}

impl Command {
    pub fn new(id: u16, payload: Vec<u8>) -> Self {
        Self { id, payload }
    }

    pub fn reset() -> Self {
        Self::new(CMD_RESET, vec![])
    }

    pub fn periodic_status() -> Self {
        Self::new(CMD_PERIODIC_STATUS, vec![])
    }

    pub fn request_buffers() -> Self {
        Self::new(CMD_REQUEST_BUFFERS, vec![])
    }

    pub fn ack_buffers() -> Self {
        Self::new(CMD_ACK_BUFFERS, vec![])
    }

    pub fn set_power_steal_mode() -> Self {
        Self::new(CMD_SET_POWER_STEAL_MODE, vec![0x00, 0x00, 0x00, 0x00])
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = BytesMut::new();
        
        // Add preamble
        buffer.extend_from_slice(COMMAND_PREAMBLE);
        
        // Add command ID (little endian)
        buffer.put_u16_le(self.id);
        
        // Add data length (little endian)
        buffer.put_u16_le(self.payload.len() as u16);
        
        // Add payload
        buffer.extend_from_slice(&self.payload);
        
        // Calculate and add CRC
        let crc_data = &buffer[COMMAND_PREAMBLE.len()..];
        let crc = CRC_CCITT.checksum(crc_data);
        buffer.put_u16_le(crc);
        
        buffer.to_vec()
    }
}

impl Response {
    pub fn parse(data: &[u8]) -> crate::Result<Self> {
        if data.len() < 8 {
            return Err(crate::error::NestError::Protocol(
                "Response too short".to_string()
            ));
        }

        // Check preamble
        if &data[0..4] != RESPONSE_PREAMBLE {
            return Err(crate::error::NestError::Protocol(
                "Invalid preamble".to_string()
            ));
        }

        let id = u16::from_le_bytes([data[4], data[5]]);
        let length = u16::from_le_bytes([data[6], data[7]]) as usize;

        if data.len() < 8 + length + 2 {
            return Err(crate::error::NestError::Protocol(
                "Incomplete response".to_string()
            ));
        }

        let payload = data[8..8 + length].to_vec();
        let received_crc = u16::from_le_bytes([data[8 + length], data[8 + length + 1]]);

        // Verify CRC
        let crc_data = &data[4..8 + length];
        let calculated_crc = CRC_CCITT.checksum(crc_data);

        if received_crc != calculated_crc {
            return Err(crate::error::NestError::CrcMismatch);
        }

        Ok(Self { id, payload })
    }

    pub fn parse_temperature(&self) -> Option<TemperatureReading> {
        if self.id != RESP_TEMPERATURE && self.id != RESP_BUFFERED_TEMP {
            return None;
        }

        if self.payload.len() < 4 {
            return None;
        }

        let temp_raw = i16::from_le_bytes([self.payload[0], self.payload[1]]);
        let humidity_raw = u16::from_le_bytes([self.payload[2], self.payload[3]]);

        let temperature_celsius = temp_raw as f32 / 100.0;
        let humidity_percent = humidity_raw as f32 / 10.0;

        Some(TemperatureReading {
            temperature_celsius,
            humidity_percent,
            timestamp: chrono::Utc::now(),
        })
    }

    pub fn parse_ascii_message(&self) -> Option<String> {
        if self.id != RESP_ASCII_MESSAGE {
            return None;
        }

        String::from_utf8(self.payload.clone()).ok()
    }

    pub fn is_end_of_buffers(&self) -> bool {
        self.id == RESP_END_BUFFERS
    }
}

#[derive(Debug)]
pub struct ProtocolState {
    pub initialized: bool,
    pub info: Option<BackplateInfo>,
    pub last_temperature: Option<TemperatureReading>,
}

impl ProtocolState {
    pub fn new() -> Self {
        Self {
            initialized: false,
            info: None,
            last_temperature: None,
        }
    }
}
