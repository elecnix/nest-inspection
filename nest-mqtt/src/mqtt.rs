//! MQTT client integration

use crate::error::Result;
use crate::thermostat::{ThermostatState, HvacMode, FanMode, ThermostatCommand};
use rumqttc::{AsyncClient, Event, EventLoop, Packet, QoS};
use serde_json;
use std::time::Duration;
use tokio::time;
use log::{debug, info, warn, error};

pub struct MqttClient {
    client: AsyncClient,
    eventloop: EventLoop,
    config: MqttConfig,
}

#[derive(Debug, Clone)]
pub struct MqttConfig {
    pub broker_host: String,
    pub broker_port: u16,
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub topic_prefix: String,
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            broker_host: "localhost".to_string(),
            broker_port: 1883,
            client_id: "nest-thermostat".to_string(),
            username: None,
            password: None,
            topic_prefix: "nest/thermostat".to_string(),
        }
    }
}

impl MqttClient {
    pub fn new(config: MqttConfig) -> Result<Self> {
        let broker_addr = format!("{}:{}", config.broker_host, config.broker_port);
        
        let mut client_opts = rumqttc::MqttOptions::new(
            config.client_id.clone(),
            config.broker_host.clone(),
            config.broker_port,
        );
        
        client_opts.set_keep_alive(Duration::from_secs(60));
        
        if let (Some(username), Some(password)) = (&config.username, &config.password) {
            client_opts.set_credentials(username, password);
        }
        
        let (client, eventloop) = AsyncClient::new(client_opts, 10);
        
        Ok(Self {
            client,
            eventloop,
            config,
        })
    }

    pub async fn connect(&mut self) -> Result<()> {
        info!("Connecting to MQTT broker at {}:{}", 
              self.config.broker_host, self.config.broker_port);
        
        // Subscribe to control topics
        let topics = [
            (format!("{}/mode/set", self.config.topic_prefix), QoS::AtMostOnce),
            (format!("{}/temperature/set", self.config.topic_prefix), QoS::AtMostOnce),
            (format!("{}/temperature_range/set", self.config.topic_prefix), QoS::AtMostOnce),
            (format!("{}/fan/set", self.config.topic_prefix), QoS::AtMostOnce),
            (format!("{}/eco/set", self.config.topic_prefix), QoS::AtMostOnce),
        ];
        
        for (topic, qos) in topics {
            let topic_clone = topic.clone();
            self.client.subscribe(topic, qos).await?;
            debug!("Subscribed to topic: {}", topic_clone);
        }
        
        info!("MQTT client connected and subscribed");
        Ok(())
    }

    pub async fn publish_state(&mut self, state: &ThermostatState) -> Result<()> {
        let payload = state.to_mqtt_payload();
        
        // Publish full state
        self.client.publish(
            format!("{}/state", self.config.topic_prefix),
            QoS::AtMostOnce,
            false,
            payload.clone(),
        ).await?;
        
        // Publish individual values for easier consumption
        self.client.publish(
            format!("{}/temperature", self.config.topic_prefix),
            QoS::AtMostOnce,
            false,
            format!("{:.2}", state.current_temperature),
        ).await?;
        
        if let Some(target) = state.target_temperature {
            self.client.publish(
                format!("{}/target_temperature", self.config.topic_prefix),
                QoS::AtMostOnce,
                false,
                format!("{:.2}", target),
            ).await?;
        }
        
        if let Some(humidity) = state.humidity {
            self.client.publish(
                format!("{}/humidity", self.config.topic_prefix),
                QoS::AtMostOnce,
                false,
                format!("{:.1}", humidity),
            ).await?;
        }
        
        self.client.publish(
            format!("{}/mode", self.config.topic_prefix),
            QoS::AtMostOnce,
            false,
            format!("{:?}", state.mode),
        ).await?;
        
        self.client.publish(
            format!("{}/fan_mode", self.config.topic_prefix),
            QoS::AtMostOnce,
            false,
            format!("{:?}", state.fan_mode),
        ).await?;
        
        self.client.publish(
            format!("{}/hvac_state", self.config.topic_prefix),
            QoS::AtMostOnce,
            false,
            format!("heating={},cooling={},fan={}", 
                   state.is_heating, state.is_cooling, state.is_fan_running),
        ).await?;
        
        debug!("Published thermostat state to MQTT");
        Ok(())
    }

    pub async fn poll_commands(&mut self) -> Result<Vec<ThermostatCommand>> {
        let mut commands = Vec::new();
        
        // Poll for events with timeout
        match time::timeout(Duration::from_millis(100), self.eventloop.poll()).await {
            Ok(Ok(Event::Incoming(Packet::Publish(msg)))) => {
                debug!("Received MQTT message on topic: {}", msg.topic);
                
                if let Some(command) = self.parse_command(&msg.topic, &msg.payload)? {
                    commands.push(command);
                }
            }
            Ok(Ok(event)) => {
                debug!("Received MQTT event: {:?}", event);
            }
            Ok(Err(e)) => {
                warn!("MQTT error: {}", e);
            }
            Err(_) => {
                // Timeout is expected
            }
        }
        
        Ok(commands)
    }

    fn parse_command(&self, topic: &str, payload: &[u8]) -> Result<Option<ThermostatCommand>> {
        use crate::thermostat::ThermostatCommand;
        use std::collections::HashMap;
        
        let payload_str = String::from_utf8_lossy(payload);
        let topic_suffix = topic.strip_prefix(&format!("{}/", self.config.topic_prefix))
            .unwrap_or(topic);
        
        let (command_type, parameters) = match topic_suffix {
            "mode/set" => {
                let mode = match payload_str.trim() {
                    "off" => HvacMode::Off,
                    "heat" => HvacMode::Heat,
                    "cool" => HvacMode::Cool,
                    "heatcool" => HvacMode::HeatCool,
                    "eco" => HvacMode::Eco,
                    _ => {
                        warn!("Invalid HVAC mode: {}", payload_str);
                        return Ok(None);
                    }
                };
                
                let mut params = HashMap::new();
                params.insert("mode".to_string(), serde_json::Value::String(format!("{:?}", mode)));
                
                ("set_mode".to_string(), params)
            }
            "temperature/set" => {
                let temperature: f32 = payload_str.trim().parse()
                    .map_err(|_| crate::error::NestError::Protocol(
                        format!("Invalid temperature: {}", payload_str)
                    ))?;
                
                let mut params = HashMap::new();
                params.insert("temperature".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(temperature as f64).unwrap_or_else(|| serde_json::Number::from(0))));
                
                ("set_temperature".to_string(), params)
            }
            "temperature_range/set" => {
                let parts: Vec<&str> = payload_str.trim().split(',').collect();
                if parts.len() != 2 {
                    warn!("Invalid temperature range format: {}", payload_str);
                    return Ok(None);
                }
                
                let low: f32 = parts[0].trim().parse()
                    .map_err(|_| crate::error::NestError::Protocol(
                        format!("Invalid low temperature: {}", parts[0])
                    ))?;
                let high: f32 = parts[1].trim().parse()
                    .map_err(|_| crate::error::NestError::Protocol(
                        format!("Invalid high temperature: {}", parts[1])
                    ))?;
                
                let mut params = HashMap::new();
                params.insert("low".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(low as f64).unwrap_or_else(|| serde_json::Number::from(0))));
                params.insert("high".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(high as f64).unwrap_or_else(|| serde_json::Number::from(0))));
                
                ("set_temperature_range".to_string(), params)
            }
            "fan/set" => {
                let mode = match payload_str.trim() {
                    "auto" => FanMode::Auto,
                    "on" => FanMode::On,
                    "circulate" => FanMode::Circulate,
                    _ => {
                        warn!("Invalid fan mode: {}", payload_str);
                        return Ok(None);
                    }
                };
                
                let mut params = HashMap::new();
                params.insert("fan_mode".to_string(), serde_json::Value::String(format!("{:?}", mode)));
                
                ("set_fan_mode".to_string(), params)
            }
            "eco/set" => {
                let enabled: bool = payload_str.trim().parse()
                    .map_err(|_| crate::error::NestError::Protocol(
                        format!("Invalid eco setting: {}", payload_str)
                    ))?;
                
                let mode = if enabled { HvacMode::Eco } else { HvacMode::Off };
                let mut params = HashMap::new();
                params.insert("eco_mode".to_string(), serde_json::Value::Bool(enabled));
                
                ("set_mode".to_string(), params)
            }
            _ => {
                debug!("Unknown topic: {}", topic);
                return Ok(None);
            }
        };
        
        Ok(Some(ThermostatCommand {
            timestamp: chrono::Utc::now(),
            command_type,
            parameters,
        }))
    }

    pub async fn publish_status(&mut self, status: &str) -> Result<()> {
        self.client.publish(
            format!("{}/status", self.config.topic_prefix),
            QoS::AtMostOnce,
            false,
            status.to_string(),
        ).await?;
        
        debug!("Published status: {}", status);
        Ok(())
    }

    pub async fn publish_error(&mut self, error: &str) -> Result<()> {
        self.client.publish(
            format!("{}/error", self.config.topic_prefix),
            QoS::AtMostOnce,
            false,
            error.to_string(),
        ).await?;
        
        warn!("Published error: {}", error);
        Ok(())
    }
}

