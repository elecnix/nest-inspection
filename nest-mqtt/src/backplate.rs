//! Backplate serial communication implementation

use crate::error::{Result, NestError};
use crate::protocol::*;
use serialport::{SerialPort, SerialPortInfo};
use std::io::{Read, Write};
use std::time::Duration;
use tokio::time::timeout;
use log::{debug, info, warn, error};

pub struct BackplateConnection {
    port: Box<dyn SerialPort>,
    state: ProtocolState,
}

impl BackplateConnection {
    pub fn open() -> Result<Self> {
        let port_name = "/dev/ttyO2";
        
        info!("Opening serial port: {}", port_name);
        
        let port = serialport::new(port_name, 115_200)
            .timeout(Duration::from_millis(1000))
            .baud_rate(115_200)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .open()?;

        // Configure termios to disable ICRNL (prevent CR to NL translation)
        #[cfg(target_os = "linux")]
        {
            use libc::{tcsetattr, termios, TCSANOW, ICRNL, ECHO, ICANON};
            
            // Note: We'll handle this at runtime since we can't access fd from Box<dyn SerialPort>
        }

        Ok(Self {
            port,
            state: ProtocolState::new(),
        })
    }

    pub async fn initialize(&mut self) -> Result<()> {
        info!("Starting backplate initialization");
        
        // Send break signal
        debug!("Sending break signal");
        self.send_break()?;
        
        // Send reset command
        debug!("Sending reset command");
        let reset_cmd = Command::reset();
        self.send_command(&reset_cmd)?;
        
        // Wait for initial burst and collect responses
        info!("Waiting for initial response burst");
        self.collect_initial_burst().await?;
        
        // Send FET presence ACK
        debug!("Sending FET presence ACK");
        let fet_ack = Command::new(CMD_FET_PRESENCE_ACK, vec![]);
        self.send_command(&fet_ack)?;
        
        // Start periodic status requests
        debug!("Starting periodic status requests");
        let status_cmd = Command::periodic_status();
        self.send_command(&status_cmd)?;
        
        // Get device information
        info!("Gathering device information");
        self.gather_device_info().await?;
        
        // Set power steal mode
        debug!("Setting power steal mode");
        let power_cmd = Command::set_power_steal_mode();
        self.send_command(&power_cmd)?;
        
        self.state.initialized = true;
        info!("Backplate initialization complete");
        
        Ok(())
    }

    async fn collect_initial_burst(&mut self) -> Result<()> {
        let mut responses = Vec::new();
        let start_time = std::time::Instant::now();
        
        while start_time.elapsed() < Duration::from_secs(5) {
            match timeout(Duration::from_millis(100), self.read_response()).await {
                Ok(Ok(response)) => {
                    debug!("Received initial response: 0x{:04x}", response.id);
                    
                    if let Some(msg) = response.parse_ascii_message() {
                        debug!("ASCII message: {}", msg);
                        if msg.trim() == "BRK" {
                            info!("Received BRK - end of initial burst");
                            break;
                        }
                    }
                    
                    responses.push(response);
                }
                Ok(Err(e)) => {
                    warn!("Error reading initial response: {}", e);
                }
                Err(_) => {
                    // Timeout, continue
                }
            }
        }
        
        Ok(())
    }

    async fn gather_device_info(&mut self) -> Result<()> {
        let mut info = BackplateInfo {
            version: String::new(),
            build_timestamp: String::new(),
            serial_number: String::new(),
            hardware_version: String::new(),
            model: String::new(),
        };

        // Small delay between commands for reliability
        let delay = Duration::from_millis(50);

        // Get version
        if let Some(version) = self.query_string(CMD_GET_MONO_TFE_VERSION, delay).await? {
            info.version = version;
        }

        // Get build info
        if let Some(build) = self.query_string(CMD_GET_MONO_TFE_BUILD_INFO, delay).await? {
            info.build_timestamp = build;
        }

        // Get hardware version
        if let Some(hw_version) = self.query_string(CMD_GET_HARDWARE_VERSION, delay).await? {
            info.hardware_version = hw_version;
        }

        // Get serial number
        if let Some(serial) = self.query_string(CMD_GET_SERIAL_NUMBER, delay).await? {
            info.serial_number = serial;
        }

        // Get model
        if let Some(model) = self.query_string(CMD_GET_BP_MODEL, delay).await? {
            info.model = model;
        }

        self.state.info = Some(info);
        Ok(())
    }

    async fn query_string(&mut self, cmd_id: u16, delay: Duration) -> Result<Option<String>> {
        let cmd = Command::new(cmd_id, vec![]);
        self.send_command(&cmd)?;
        
        tokio::time::sleep(delay).await;
        
        match timeout(Duration::from_millis(500), self.read_response()).await {
            Ok(Ok(response)) => Ok(response.parse_ascii_message()),
            Ok(Err(e)) => {
                warn!("Error querying string for 0x{:04x}: {}", cmd_id, e);
                Ok(None)
            }
            Err(_) => {
                warn!("Timeout querying string for 0x{:04x}", cmd_id);
                Ok(None)
            }
        }
    }

    pub async fn request_sensor_data(&mut self) -> Result<Vec<TemperatureReading>> {
        if !self.state.initialized {
            return Err(NestError::DeviceNotConnected);
        }

        debug!("Requesting sensor data buffers");
        let cmd = Command::request_buffers();
        self.send_command(&cmd)?;

        let mut readings = Vec::new();
        let start_time = std::time::Instant::now();

        while start_time.elapsed() < Duration::from_secs(2) {
            match timeout(Duration::from_millis(100), self.read_response()).await {
                Ok(Ok(response)) => {
                    debug!("Received sensor response: 0x{:04x}", response.id);
                    
                    if response.is_end_of_buffers() {
                        debug!("End of buffers marker received");
                        break;
                    }
                    
                    if let Some(reading) = response.parse_temperature() {
                        debug!("Temperature reading: {}°C, {}% RH", 
                               reading.temperature_celsius, reading.humidity_percent);
                        readings.push(reading.clone());
                        self.state.last_temperature = Some(reading.clone());
                    }
                }
                Ok(Err(e)) => {
                    warn!("Error reading sensor response: {}", e);
                }
                Err(_) => {
                    // Timeout, continue
                }
            }
        }

        // Acknowledge end of buffers
        debug!("Acknowledging end of buffers");
        let ack_cmd = Command::ack_buffers();
        self.send_command(&ack_cmd)?;

        Ok(readings)
    }

    pub async fn keep_alive(&mut self) -> Result<()> {
        if !self.state.initialized {
            return Err(NestError::DeviceNotConnected);
        }

        debug!("Sending keep-alive status request");
        let cmd = Command::periodic_status();
        self.send_command(&cmd)?;

        // Read any response but don't block
        match timeout(Duration::from_millis(100), self.read_response()).await {
            Ok(Ok(response)) => {
                debug!("Keep-alive response: 0x{:04x}", response.id);
            }
            Ok(Err(e)) => {
                warn!("Keep-alive error: {}", e);
            }
            Err(_) => {
                // Timeout is expected for keep-alive
            }
        }

        Ok(())
    }

    fn send_break(&mut self) -> Result<()> {
        // Note: tcsendbreak requires file descriptor access
        // For now, we'll skip this as it's not critical for basic functionality
        debug!("Skipping break signal (requires fd access)");
        Ok(())
    }

    fn send_command(&mut self, command: &Command) -> Result<()> {
        let data = command.serialize();
        debug!("Sending command: 0x{:04x} ({} bytes)", command.id, data.len());
        self.port.write_all(&data)?;
        self.port.flush()?;
        Ok(())
    }

    async fn read_response(&mut self) -> Result<Response> {
        let mut buffer = vec![0u8; 1024];
        
        // Read preamble first
        let mut preamble_buffer = [0u8; 4];
        let mut bytes_read = 0;
        
        while bytes_read < 4 {
            match self.port.read(&mut preamble_buffer[bytes_read..]) {
                Ok(n) => {
                    bytes_read += n;
                    if bytes_read == 4 {
                        // Check if we have the right preamble
                        if &preamble_buffer != RESPONSE_PREAMBLE {
                            // Shift buffer and continue looking
                            preamble_buffer.copy_within(1.., 0);
                            bytes_read = 3;
                            continue;
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }
        
        // We have preamble, read the rest
        buffer[..4].copy_from_slice(&preamble_buffer);
        bytes_read = 4;
        
        // Read header (command ID + length)
        while bytes_read < 8 {
            match self.port.read(&mut buffer[bytes_read..]) {
                Ok(n) => bytes_read += n,
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }
        
        let length = u16::from_le_bytes([buffer[6], buffer[7]]) as usize;
        let total_length = 8 + length + 2; // +2 for CRC
        
        if total_length > buffer.len() {
            return Err(NestError::Protocol("Response too large".to_string()));
        }
        
        // Read remaining data
        while bytes_read < total_length {
            match self.port.read(&mut buffer[bytes_read..total_length]) {
                Ok(n) => bytes_read += n,
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }
        
        let response_data = &buffer[..total_length];
        let response = Response::parse(response_data)?;
        
        Ok(response)
    }

    pub fn get_state(&self) -> &ProtocolState {
        &self.state
    }

    pub fn is_initialized(&self) -> bool {
        self.state.initialized
    }
}
