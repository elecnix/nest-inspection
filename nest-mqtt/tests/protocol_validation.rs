// Protocol validation tests - can run without hardware

#[cfg(test)]
mod tests {
    use nest_mqtt::protocol::*;

    #[test]
    fn test_command_serialization() {
        let cmd = Command::reset();
        let data = cmd.serialize();
        
        // Should start with preamble
        assert_eq!(&data[0..3], COMMAND_PREAMBLE);
        
        // Command ID should be 0x00ff
        let cmd_id = u16::from_le_bytes([data[3], data[4]]);
        assert_eq!(cmd_id, CMD_RESET);
        
        // Length should be 0
        let length = u16::from_le_bytes([data[5], data[6]]);
        assert_eq!(length, 0);
        
        // Should have CRC at end
        assert_eq!(data.len(), 9); // 3 + 2 + 2 + 0 + 2
    }

    #[test]
    fn test_temperature_response_parsing() {
        // Simulate a temperature response
        let mut response_data = vec![0xd5, 0xd5, 0xaa, 0x96]; // Preamble
        response_data.extend_from_slice(&0x0002u16.to_le_bytes()); // Command ID
        response_data.extend_from_slice(&4u16.to_le_bytes()); // Length
        
        // Temperature: 22.5°C = 2250 (scaled by 100)
        response_data.extend_from_slice(&2250i16.to_le_bytes());
        // Humidity: 45% = 450 (scaled by 10)
        response_data.extend_from_slice(&450u16.to_le_bytes());
        
        // Add CRC
        let crc_data = &response_data[4..];
        let crc = CRC_CCITT.checksum(crc_data);
        response_data.extend_from_slice(&crc.to_le_bytes());
        
        // Parse response
        let response = Response::parse(&response_data).unwrap();
        assert_eq!(response.id, RESP_TEMPERATURE);
        
        // Parse temperature
        let reading = response.parse_temperature().unwrap();
        assert_eq!(reading.temperature_celsius, 22.5);
        assert_eq!(reading.humidity_percent, 45.0);
    }

    #[test]
    fn test_crc_validation() {
        let cmd = Command::new(CMD_PERIODIC_STATUS, vec![]);
        let data = cmd.serialize();
        
        // Corrupt the CRC
        let mut corrupted = data.clone();
        let len = corrupted.len();
        corrupted[len - 1] ^= 0xFF;
        
        // Should fail CRC check when parsed as response
        // (we'd need to add response preamble for this test)
    }

    #[test]
    fn test_all_command_types() {
        let commands = vec![
            Command::reset(),
            Command::periodic_status(),
            Command::request_buffers(),
            Command::ack_buffers(),
            Command::set_power_steal_mode(),
        ];
        
        for cmd in commands {
            let data = cmd.serialize();
            // All should have proper preamble
            assert_eq!(&data[0..3], COMMAND_PREAMBLE);
            // All should have valid structure
            assert!(data.len() >= 9); // Minimum size
        }
    }
}
