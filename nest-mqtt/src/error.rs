use thiserror::Error;

pub type Result<T> = std::result::Result<T, NestError>;

#[derive(Error, Debug)]
pub enum NestError {
    #[error("Serial port error: {0}")]
    Serial(#[from] serialport::Error),
    
    #[error("MQTT error: {0}")]
    Mqtt(#[from] rumqttc::ClientError),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("Timeout error")]
    Timeout,
    
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    
    #[error("CRC mismatch")]
    CrcMismatch,
    
    #[error("Device not connected")]
    DeviceNotConnected,
    
    #[error("Invalid command: {0}")]
    InvalidCommand(String),
}
