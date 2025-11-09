//! Nest MQTT Bridge Main Application
//! 
//! Bridges Nest Learning Thermostat backplate communication to MQTT

use clap::{Arg, Command};
use log::{info, warn, error, debug};
use nest_mqtt::{backplate::BackplateConnection, mqtt::MqttClient, thermostat::ThermostatController};
use std::time::Duration;
use tokio::signal;
use tokio::time::{interval, sleep};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let matches = Command::new("nest-mqtt")
        .version("0.1.0")
        .about("MQTT bridge for Nest Learning Thermostat")
        .arg(
            Arg::new("broker")
                .long("broker")
                .value_name("HOST:PORT")
                .help("MQTT broker address")
                .default_value("localhost:1883"),
        )
        .arg(
            Arg::new("client-id")
                .long("client-id")
                .value_name("ID")
                .help("MQTT client ID")
                .default_value("nest-thermostat"),
        )
        .arg(
            Arg::new("topic-prefix")
                .long("topic-prefix")
                .value_name("PREFIX")
                .help("MQTT topic prefix")
                .default_value("nest/thermostat"),
        )
        .arg(
            Arg::new("serial-device")
                .long("serial-device")
                .value_name("DEVICE")
                .help("Serial device path")
                .default_value("/dev/ttyO2"),
        )
        .get_matches();

    info!("Starting Nest MQTT Bridge");

    // Parse MQTT configuration
    let broker_addr = matches.get_one::<String>("broker").unwrap();
    let (broker_host, broker_port) = if broker_addr.contains(':') {
        let parts: Vec<&str> = broker_addr.split(':').collect();
        (parts[0].to_string(), parts[1].parse::<u16>().unwrap_or(1883))
    } else {
        (broker_addr.to_string(), 1883)
    };

    let mqtt_config = nest_mqtt::mqtt::MqttConfig {
        broker_host,
        broker_port,
        client_id: matches.get_one::<String>("client-id").unwrap().clone(),
        username: None,
        password: None,
        topic_prefix: matches.get_one::<String>("topic-prefix").unwrap().clone(),
    };

    // Initialize MQTT client
    let mut mqtt_client = MqttClient::new(mqtt_config)?;
    mqtt_client.connect().await?;

    // Initialize backplate connection
    info!("Connecting to Nest backplate");
    let mut backplate = match BackplateConnection::open() {
        Ok(bp) => {
            info!("Successfully opened backplate connection");
            bp
        }
        Err(e) => {
            error!("Failed to open backplate connection: {}", e);
            mqtt_client.publish_error(&format!("Backplate connection failed: {}", e)).await?;
            return Err(e.into());
        }
    };

    // Initialize backplate protocol
    match backplate.initialize().await {
        Ok(()) => {
            info!("Backplate initialization complete");
            mqtt_client.publish_status("Backplate initialized").await?;
        }
        Err(e) => {
            error!("Backplate initialization failed: {}", e);
            mqtt_client.publish_error(&format!("Backplate initialization failed: {}", e)).await?;
            return Err(e.into());
        }
    }

    // Initialize thermostat controller
    let mut thermostat = ThermostatController::new();

    // Set up periodic tasks
    let mut sensor_interval = interval(Duration::from_secs(30));
    let mut keepalive_interval = interval(Duration::from_secs(15));
    let mut publish_interval = interval(Duration::from_secs(5));

    info!("Starting main operation loop");

    // Set up graceful shutdown
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        signal::ctrl_c().await.unwrap();
        info!("Received shutdown signal");
        let _ = shutdown_tx.send(true);
    });

    loop {
        tokio::select! {
            // Check for shutdown
            _ = shutdown_rx.changed() => {
                info!("Shutting down...");
                break;
            }

            // Sensor data polling (every 30 seconds)
            _ = sensor_interval.tick() => {
                debug!("Polling sensor data");
                match backplate.request_sensor_data().await {
                    Ok(readings) => {
                        if let Some(reading) = readings.first() {
                            thermostat.update_sensor_data(reading.clone());
                            thermostat.simulate_hvac_control();
                            info!("Temperature: {}°C, Humidity: {}%", 
                                  reading.temperature_celsius, reading.humidity_percent);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read sensor data: {}", e);
                        mqtt_client.publish_error(&format!("Sensor read failed: {}", e)).await?;
                    }
                }
            }

            // Keep-alive (every 15 seconds)
            _ = keepalive_interval.tick() => {
                debug!("Sending keep-alive");
                if let Err(e) = backplate.keep_alive().await {
                    warn!("Keep-alive failed: {}", e);
                }
            }

            // MQTT state publishing (every 5 seconds)
            _ = publish_interval.tick() => {
                debug!("Publishing state to MQTT");
                if let Err(e) = mqtt_client.publish_state(thermostat.get_state()).await {
                    warn!("Failed to publish state: {}", e);
                }
            }

            // MQTT command handling
            commands = async {
                let mut all_commands = Vec::new();
                for _ in 0..10 {
                    match mqtt_client.poll_commands().await {
                        Ok(mut cmds) => all_commands.append(&mut cmds),
                        Err(e) => {
                            warn!("Error polling MQTT commands: {}", e);
                            break;
                        }
                    }
                }
                all_commands
            } => {
                for command in commands {
                    debug!("Received command: {}", command.command_type);
                    
                    match command.command_type.as_str() {
                        "set_mode" => {
                            if let Some(mode_value) = command.parameters.get("mode") {
                                let mode_str = mode_value.as_str().unwrap_or("Off");
                                let mode = match mode_str {
                                    "Off" => nest_mqtt::thermostat::HvacMode::Off,
                                    "Heat" => nest_mqtt::thermostat::HvacMode::Heat,
                                    "Cool" => nest_mqtt::thermostat::HvacMode::Cool,
                                    "HeatCool" => nest_mqtt::thermostat::HvacMode::HeatCool,
                                    "Eco" => nest_mqtt::thermostat::HvacMode::Eco,
                                    _ => continue,
                                };
                                if let Err(e) = thermostat.set_mode(mode) {
                                    error!("Failed to set mode: {}", e);
                                }
                            }
                        }
                        "set_temperature" => {
                            if let Some(temp_value) = command.parameters.get("temperature") {
                                if let Some(temp) = temp_value.as_f64() {
                                    if let Err(e) = thermostat.set_temperature(temp as f32) {
                                        error!("Failed to set temperature: {}", e);
                                    }
                                }
                            }
                        }
                        "set_temperature_range" => {
                            if let (Some(low_value), Some(high_value)) = (
                                command.parameters.get("low"),
                                command.parameters.get("high")
                            ) {
                                if let (Some(low), Some(high)) = (low_value.as_f64(), high_value.as_f64()) {
                                    if let Err(e) = thermostat.set_temperature_range(low as f32, high as f32) {
                                        error!("Failed to set temperature range: {}", e);
                                    }
                                }
                            }
                        }
                        "set_fan_mode" => {
                            if let Some(fan_value) = command.parameters.get("fan_mode") {
                                let fan_str = fan_value.as_str().unwrap_or("Auto");
                                let fan_mode = match fan_str {
                                    "Auto" => nest_mqtt::thermostat::FanMode::Auto,
                                    "On" => nest_mqtt::thermostat::FanMode::On,
                                    "Circulate" => nest_mqtt::thermostat::FanMode::Circulate,
                                    _ => continue,
                                };
                                if let Err(e) = thermostat.set_fan_mode(fan_mode) {
                                    error!("Failed to set fan mode: {}", e);
                                }
                            }
                        }
                        _ => {
                            debug!("Unknown command: {}", command.command_type);
                        }
                    }
                }
            }
        }
    }

    info!("Nest MQTT Bridge stopped");
    Ok(())
}
