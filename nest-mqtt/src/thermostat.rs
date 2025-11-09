//! Thermostat state management and control

use crate::error::Result;
use crate::protocol::TemperatureReading;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use log::{debug, info};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum HvacMode {
    Off,
    Heat,
    Cool,
    HeatCool,
    Eco,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum FanMode {
    Auto,
    On,
    Circulate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermostatState {
    pub current_temperature: f32,
    pub target_temperature: Option<f32>,
    pub target_temperature_low: Option<f32>,
    pub target_temperature_high: Option<f32>,
    pub humidity: Option<f32>,
    pub mode: HvacMode,
    pub fan_mode: FanMode,
    pub is_heating: bool,
    pub is_cooling: bool,
    pub is_fan_running: bool,
    pub eco_mode: bool,
    pub last_update: DateTime<Utc>,
    pub device_info: Option<DeviceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub version: String,
    pub build_timestamp: String,
    pub serial_number: String,
    pub hardware_version: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureSetpoint {
    pub temperature: f32,
    pub mode: HvacMode,
}

impl Default for ThermostatState {
    fn default() -> Self {
        Self {
            current_temperature: 20.0,
            target_temperature: Some(21.0),
            target_temperature_low: Some(18.0),
            target_temperature_high: Some(24.0),
            humidity: None,
            mode: HvacMode::Off,
            fan_mode: FanMode::Auto,
            is_heating: false,
            is_cooling: false,
            is_fan_running: false,
            eco_mode: false,
            last_update: Utc::now(),
            device_info: None,
        }
    }
}

impl ThermostatState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_temperature(&mut self, reading: &TemperatureReading) {
        self.current_temperature = reading.temperature_celsius;
        self.humidity = Some(reading.humidity_percent);
        self.last_update = reading.timestamp;
        
        debug!("Updated temperature: {}°C, {}% RH", 
               self.current_temperature, 
               self.humidity.unwrap_or(0.0));
    }

    pub fn set_mode(&mut self, mode: HvacMode) {
        info!("Setting HVAC mode to {:?}", mode);
        self.mode = mode;
        self.eco_mode = mode == HvacMode::Eco;
    }

    pub fn set_target_temperature(&mut self, temperature: f32) {
        info!("Setting target temperature to {}°C", temperature);
        
        match self.mode {
            HvacMode::Heat | HvacMode::Cool => {
                self.target_temperature = Some(temperature);
            }
            HvacMode::HeatCool => {
                // Adjust the range, keeping the spread
                if let (Some(low), Some(high)) = (self.target_temperature_low, self.target_temperature_high) {
                    let spread = high - low;
                    self.target_temperature_low = Some(temperature - spread / 2.0);
                    self.target_temperature_high = Some(temperature + spread / 2.0);
                }
            }
            _ => {}
        }
    }

    pub fn set_temperature_range(&mut self, low: f32, high: f32) {
        info!("Setting temperature range: {}°C to {}°C", low, high);
        self.target_temperature_low = Some(low);
        self.target_temperature_high = Some(high);
        
        // Update single target as average
        self.target_temperature = Some((low + high) / 2.0);
    }

    pub fn set_fan_mode(&mut self, mode: FanMode) {
        info!("Setting fan mode to {:?}", mode);
        self.fan_mode = mode;
    }

    pub fn needs_heating(&self) -> bool {
        match self.mode {
            HvacMode::Heat => {
                if let Some(target) = self.target_temperature {
                    self.current_temperature < target - 0.5
                } else {
                    false
                }
            }
            HvacMode::HeatCool => {
                if let Some(low) = self.target_temperature_low {
                    self.current_temperature < low - 0.5
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn needs_cooling(&self) -> bool {
        match self.mode {
            HvacMode::Cool => {
                if let Some(target) = self.target_temperature {
                    self.current_temperature > target + 0.5
                } else {
                    false
                }
            }
            HvacMode::HeatCool => {
                if let Some(high) = self.target_temperature_high {
                    self.current_temperature > high + 0.5
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn update_hvac_state(&mut self, heating: bool, cooling: bool, fan: bool) {
        self.is_heating = heating;
        self.is_cooling = cooling;
        self.is_fan_running = fan;
    }

    pub fn to_mqtt_payload(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_mqtt_payload(payload: &str) -> Result<Self> {
        Ok(serde_json::from_str(payload)?)
    }
}

pub struct ThermostatController {
    state: ThermostatState,
    command_history: Vec<ThermostatCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermostatCommand {
    pub timestamp: DateTime<Utc>,
    pub command_type: String,
    pub parameters: HashMap<String, serde_json::Value>,
}

impl ThermostatController {
    pub fn new() -> Self {
        Self {
            state: ThermostatState::new(),
            command_history: Vec::new(),
        }
    }

    pub fn get_state(&self) -> &ThermostatState {
        &self.state
    }

    pub fn get_state_mut(&mut self) -> &mut ThermostatState {
        &mut self.state
    }

    pub fn set_temperature(&mut self, temperature: f32) -> Result<()> {
        self.state.set_target_temperature(temperature);
        
        let mut params = HashMap::new();
        params.insert("temperature".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(temperature as f64).unwrap_or_else(|| serde_json::Number::from(0))));
        
        self.log_command("set_temperature", params);
        Ok(())
    }

    pub fn set_mode(&mut self, mode: HvacMode) -> Result<()> {
        self.state.set_mode(mode);
        
        let mut params = HashMap::new();
        params.insert("mode".to_string(), serde_json::Value::String(format!("{:?}", mode)));
        
        self.log_command("set_mode", params);
        Ok(())
    }

    pub fn set_fan_mode(&mut self, mode: FanMode) -> Result<()> {
        self.state.set_fan_mode(mode);
        
        let mut params = HashMap::new();
        params.insert("fan_mode".to_string(), serde_json::Value::String(format!("{:?}", mode)));
        
        self.log_command("set_fan_mode", params);
        Ok(())
    }

    pub fn set_temperature_range(&mut self, low: f32, high: f32) -> Result<()> {
        self.state.set_temperature_range(low, high);
        
        let mut params = HashMap::new();
        params.insert("low".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(low as f64).unwrap_or_else(|| serde_json::Number::from(0))));
        params.insert("high".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(high as f64).unwrap_or_else(|| serde_json::Number::from(0))));
        
        self.log_command("set_temperature_range", params);
        Ok(())
    }

    pub fn update_sensor_data(&mut self, reading: TemperatureReading) {
        self.state.update_temperature(&reading);
    }

    fn log_command(&mut self, command_type: &str, parameters: HashMap<String, serde_json::Value>) {
        let command = ThermostatCommand {
            timestamp: Utc::now(),
            command_type: command_type.to_string(),
            parameters,
        };
        
        self.command_history.push(command);
        
        // Keep only last 100 commands
        if self.command_history.len() > 100 {
            self.command_history.remove(0);
        }
    }

    pub fn get_command_history(&self) -> &[ThermostatCommand] {
        &self.command_history
    }

    pub fn simulate_hvac_control(&mut self) {
        // Simulate what the real HVAC control would do
        let heating = self.state.needs_heating();
        let cooling = self.state.needs_cooling();
        let fan = self.state.fan_mode == FanMode::On || heating || cooling;
        
        self.state.update_hvac_state(heating, cooling, fan);
    }
}
