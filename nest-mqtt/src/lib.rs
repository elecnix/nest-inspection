//! Nest MQTT Bridge
//! 
//! This library provides an interface to the Nest Learning Thermostat backplate protocol
//! and bridges it to MQTT for home automation integration.

pub mod backplate;
pub mod mqtt;
pub mod protocol;
pub mod thermostat;
pub mod error;

pub use error::Result;
pub use thermostat::{ThermostatState, ThermostatController};
pub use backplate::BackplateConnection;
