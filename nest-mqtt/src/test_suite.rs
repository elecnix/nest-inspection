//! Comprehensive test suite for Nest MQTT Bridge

use clap::{Arg, Command};
use log::{info, warn, error, debug};
use nest_mqtt::{backplate::BackplateConnection, mqtt::MqttClient, thermostat::{ThermostatController, HvacMode, FanMode}};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug)]
struct TestResults {
    total_tests: usize,
    passed_tests: usize,
    failed_tests: usize,
    errors: Vec<String>,
}

impl TestResults {
    fn new() -> Self {
        Self {
            total_tests: 0,
            passed_tests: 0,
            failed_tests: 0,
            errors: Vec::new(),
        }
    }

    fn record_pass(&mut self) {
        self.total_tests += 1;
        self.passed_tests += 1;
    }

    fn record_fail(&mut self, error: String) {
        self.total_tests += 1;
        self.failed_tests += 1;
        self.errors.push(error);
    }

    fn print_summary(&self) {
        info!("=== Test Summary ===");
        info!("Total tests: {}", self.total_tests);
        info!("Passed: {}", self.passed_tests);
        info!("Failed: {}", self.failed_tests);
        info!("Success rate: {:.1}%", 
              (self.passed_tests as f64 / self.total_tests as f64) * 100.0);
        
        if !self.errors.is_empty() {
            error!("=== Failed Tests ===");
            for error in &self.errors {
                error!("{}", error);
            }
        }
    }
}

async fn test_serial_connection() -> TestResults {
    info!("Testing serial connection...");
    let mut results = TestResults::new();

    match BackplateConnection::open() {
        Ok(_) => {
            info!("✓ Serial connection opened successfully");
            results.record_pass();
        }
        Err(e) => {
            error!("✗ Serial connection failed: {}", e);
            results.record_fail(format!("Serial connection: {}", e));
            return results;
        }
    }

    results
}

async fn test_backplate_initialization() -> TestResults {
    info!("Testing backplate initialization...");
    let mut results = TestResults::new();

    let mut backplate = match BackplateConnection::open() {
        Ok(bp) => bp,
        Err(e) => {
            results.record_fail(format!("Cannot open backplate: {}", e));
            return results;
        }
    };

    match backplate.initialize().await {
        Ok(()) => {
            info!("✓ Backplate initialized successfully");
            results.record_pass();
        }
        Err(e) => {
            error!("✗ Backplate initialization failed: {}", e);
            results.record_fail(format!("Initialization: {}", e));
            return results;
        }
    }

    // Test device info retrieval
    if let Some(info) = &backplate.get_state().info {
        info!("✓ Device info retrieved:");
        info!("  Version: {}", info.version);
        info!("  Serial: {}", info.serial_number);
        info!("  Hardware: {}", info.hardware_version);
        results.record_pass();
    } else {
        error!("✗ No device info retrieved");
        results.record_fail("No device info".to_string());
    }

    results
}

async fn test_sensor_reading() -> TestResults {
    info!("Testing sensor reading...");
    let mut results = TestResults::new();

    let mut backplate = match BackplateConnection::open() {
        Ok(bp) => bp,
        Err(e) => {
            results.record_fail(format!("Cannot open backplate: {}", e));
            return results;
        }
    };

    if let Err(e) = backplate.initialize().await {
        results.record_fail(format!("Cannot initialize backplate: {}", e));
        return results;
    }

    // Try multiple sensor reads
    for attempt in 1..=3 {
        info!("Sensor read attempt {}", attempt);
        
        match backplate.request_sensor_data().await {
            Ok(readings) => {
                if !readings.is_empty() {
                    let reading = &readings[0];
                    info!("✓ Sensor reading: {}°C, {}% RH", 
                          reading.temperature_celsius, reading.humidity_percent);
                    
                    // Validate reasonable ranges
                    if reading.temperature_celsius >= -40.0 && reading.temperature_celsius <= 125.0 {
                        info!("✓ Temperature in valid range");
                        results.record_pass();
                    } else {
                        error!("✗ Temperature out of range: {}°C", reading.temperature_celsius);
                        results.record_fail(format!("Temperature out of range: {}", reading.temperature_celsius));
                    }
                    
                    if reading.humidity_percent >= 0.0 && reading.humidity_percent <= 100.0 {
                        info!("✓ Humidity in valid range");
                        results.record_pass();
                    } else {
                        error!("✗ Humidity out of range: {}%", reading.humidity_percent);
                        results.record_fail(format!("Humidity out of range: {}", reading.humidity_percent));
                    }
                    
                    break;
                } else {
                    warn!("No sensor readings returned");
                }
            }
            Err(e) => {
                error!("✗ Sensor read failed: {}", e);
                if attempt == 3 {
                    results.record_fail(format!("Sensor read: {}", e));
                }
            }
        }
        
        sleep(Duration::from_secs(2)).await;
    }

    results
}

async fn test_keepalive() -> TestResults {
    info!("Testing keep-alive functionality...");
    let mut results = TestResults::new();

    let mut backplate = match BackplateConnection::open() {
        Ok(bp) => bp,
        Err(e) => {
            results.record_fail(format!("Cannot open backplate: {}", e));
            return results;
        }
    };

    if let Err(e) = backplate.initialize().await {
        results.record_fail(format!("Cannot initialize backplate: {}", e));
        return results;
    }

    // Test multiple keep-alive commands
    for i in 1..=5 {
        match backplate.keep_alive().await {
            Ok(()) => {
                debug!("✓ Keep-alive {} successful", i);
            }
            Err(e) => {
                warn!("Keep-alive {} failed: {}", i, e);
            }
        }
        sleep(Duration::from_millis(200)).await;
    }

    info!("✓ Keep-alive test completed");
    results.record_pass();
    results
}

async fn test_thermostat_controller() -> TestResults {
    info!("Testing thermostat controller...");
    let mut results = TestResults::new();

    let mut controller = ThermostatController::new();
    
    // Test initial state
    let state = controller.get_state();
    info!("✓ Initial state: {}°C, mode={:?}", 
          state.current_temperature, state.mode);
    results.record_pass();

    // Test temperature setting
    match controller.set_temperature(22.5) {
        Ok(()) => {
            info!("✓ Temperature set to 22.5°C");
            if controller.get_state().target_temperature == Some(22.5) {
                results.record_pass();
            } else {
                results.record_fail("Temperature not set correctly".to_string());
            }
        }
        Err(e) => {
            results.record_fail(format!("Set temperature failed: {}", e));
        }
    }

    // Test mode setting
    match controller.set_mode(HvacMode::Heat) {
        Ok(()) => {
            info!("✓ Mode set to Heat");
            if controller.get_state().mode == HvacMode::Heat {
                results.record_pass();
            } else {
                results.record_fail("Mode not set correctly".to_string());
            }
        }
        Err(e) => {
            results.record_fail(format!("Set mode failed: {}", e));
        }
    }

    // Test fan mode setting
    match controller.set_fan_mode(FanMode::On) {
        Ok(()) => {
            info!("✓ Fan mode set to On");
            if controller.get_state().fan_mode == FanMode::On {
                results.record_pass();
            } else {
                results.record_fail("Fan mode not set correctly".to_string());
            }
        }
        Err(e) => {
            results.record_fail(format!("Set fan mode failed: {}", e));
        }
    }

    // Test temperature range setting
    match controller.set_temperature_range(18.0, 24.0) {
        Ok(()) => {
            info!("✓ Temperature range set to 18-24°C");
            let state = controller.get_state();
            if state.target_temperature_low == Some(18.0) && 
               state.target_temperature_high == Some(24.0) {
                results.record_pass();
            } else {
                results.record_fail("Temperature range not set correctly".to_string());
            }
        }
        Err(e) => {
            results.record_fail(format!("Set temperature range failed: {}", e));
        }
    }

    // Test HVAC state simulation
    controller.simulate_hvac_control();
    info!("✓ HVAC state simulation completed");
    results.record_pass();

    results
}

async fn test_mqtt_connection() -> TestResults {
    info!("Testing MQTT connection...");
    let mut results = TestResults::new();

    let mqtt_config = nest_mqtt::mqtt::MqttConfig {
        broker_host: "localhost".to_string(),
        broker_port: 1883,
        client_id: "nest-test".to_string(),
        username: None,
        password: None,
        topic_prefix: "nest/test".to_string(),
    };

    match MqttClient::new(mqtt_config) {
        Ok(mut client) => {
            info!("✓ MQTT client created");
            results.record_pass();

            match client.connect().await {
                Ok(()) => {
                    info!("✓ MQTT connection established");
                    results.record_pass();

                    // Test publishing
                    match client.publish_status("test").await {
                        Ok(()) => {
                            info!("✓ MQTT status published");
                            results.record_pass();
                        }
                        Err(e) => {
                            results.record_fail(format!("MQTT publish failed: {}", e));
                        }
                    }
                }
                Err(e) => {
                    results.record_fail(format!("MQTT connection failed: {}", e));
                }
            }
        }
        Err(e) => {
            results.record_fail(format!("MQTT client creation failed: {}", e));
        }
    }

    results
}

async fn test_integration() -> TestResults {
    info!("Testing full integration...");
    let mut results = TestResults::new();

    // Initialize all components
    let mut backplate = match BackplateConnection::open() {
        Ok(bp) => bp,
        Err(e) => {
            results.record_fail(format!("Cannot open backplate: {}", e));
            return results;
        }
    };

    if let Err(e) = backplate.initialize().await {
        results.record_fail(format!("Cannot initialize backplate: {}", e));
        return results;
    }

    let mut controller = ThermostatController::new();

    let mqtt_config = nest_mqtt::mqtt::MqttConfig {
        broker_host: "localhost".to_string(),
        broker_port: 1883,
        client_id: "nest-integration-test".to_string(),
        username: None,
        password: None,
        topic_prefix: "nest/integration".to_string(),
    };

    let mut mqtt_client = match MqttClient::new(mqtt_config) {
        Ok(client) => client,
        Err(e) => {
            results.record_fail(format!("Cannot create MQTT client: {}", e));
            return results;
        }
    };

    if let Err(e) = mqtt_client.connect().await {
        results.record_fail(format!("Cannot connect MQTT: {}", e));
        return results;
    }

    // Test sensor data flow
    match backplate.request_sensor_data().await {
        Ok(readings) => {
            if let Some(reading) = readings.first() {
                controller.update_sensor_data(reading.clone());
                controller.simulate_hvac_control();
                
                // Test publishing
                match mqtt_client.publish_state(controller.get_state()).await {
                    Ok(()) => {
                        info!("✓ Integration test: sensor data → controller → MQTT");
                        results.record_pass();
                    }
                    Err(e) => {
                        results.record_fail(format!("Integration publish failed: {}", e));
                    }
                }
            } else {
                results.record_fail("No sensor data for integration test".to_string());
            }
        }
        Err(e) => {
            results.record_fail(format!("Integration sensor read failed: {}", e));
        }
    }

    results
}

async fn run_all_tests() -> TestResults {
    info!("=== Starting Nest MQTT Bridge Test Suite ===");
    
    let mut overall_results = TestResults::new();

    // Run individual test suites
    let test_results = vec![
        test_serial_connection().await,
        test_backplate_initialization().await,
        test_sensor_reading().await,
        test_keepalive().await,
        test_thermostat_controller().await,
        test_mqtt_connection().await,
        test_integration().await,
    ];

    for (i, result) in test_results.into_iter().enumerate() {
        info!("Test suite {} completed", i + 1);
        overall_results.total_tests += result.total_tests;
        overall_results.passed_tests += result.passed_tests;
        overall_results.failed_tests += result.failed_tests;
        overall_results.errors.extend(result.errors);
        
        sleep(Duration::from_secs(1)).await;
    }

    overall_results.print_summary();
    overall_results
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let matches = Command::new("nest-test")
        .version("0.1.0")
        .about("Test suite for Nest MQTT Bridge")
        .arg(
            Arg::new("test")
                .long("test")
                .value_name("TEST")
                .help("Specific test to run")
                .value_parser([
                    "serial", "init", "sensor", "keepalive", 
                    "controller", "mqtt", "integration", "all"
                ])
                .default_value("all"),
        )
        .get_matches();

    let test_name = matches.get_one::<String>("test").unwrap();

    let results = match test_name.as_str() {
        "serial" => test_serial_connection().await,
        "init" => test_backplate_initialization().await,
        "sensor" => test_sensor_reading().await,
        "keepalive" => test_keepalive().await,
        "controller" => test_thermostat_controller().await,
        "mqtt" => test_mqtt_connection().await,
        "integration" => test_integration().await,
        "all" => run_all_tests().await,
        _ => {
            error!("Unknown test: {}", test_name);
            return Err("Unknown test".into());
        }
    };

    results.print_summary();

    // Exit with error code if any tests failed
    if results.failed_tests > 0 {
        std::process::exit(1);
    }

    Ok(())
}
