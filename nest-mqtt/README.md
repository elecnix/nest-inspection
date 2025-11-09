# Nest MQTT Bridge

A Rust-based bridge that connects to a Nest Learning Thermostat via the backplate serial protocol and exposes thermostat controls and sensor data through MQTT.

## Features

- **Direct Hardware Access**: Communicates directly with the Nest backplate via `/dev/ttyO2`
- **Real-time Sensor Data**: Reads temperature and humidity from the thermostat
- **MQTT Integration**: Publishes state and subscribes to control commands via MQTT
- **Comprehensive Control**: Set temperature, HVAC mode, fan mode, and more
- **Test Suite**: Full test suite for validating functionality
- **Robust Protocol**: Implements the complete Nest backplate protocol with CRC validation

## Protocol Documentation

This implementation is based on the [Nest Backplate Protocol](https://github.com/cuckoo-nest/wiki/blob/main/backplate/Protocol.md) documentation.

## Hardware Requirements

- Nest Learning Thermostat (2nd generation or later)
- Root access to the thermostat's Linux system
- Access to the backplate serial device `/dev/ttyO2`

## Installation

### On the Nest Thermostat

1. Install Rust (if not already installed):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

2. Clone and build:
```bash
cd /tmp
git clone <repository-url>
cd nest-mqtt
cargo build --release
```

### MQTT Broker Setup

Install and start Mosquitto:
```bash
# Install Mosquitto
opkg update
opkg install mosquitto-ssl mosquitto-client-ssl

# Start the broker
/etc/init.d/mosquitto start
# or run directly:
mosquitto -d -p 1883
```

## Usage

### Running the Bridge

```bash
# Basic usage with default settings
./target/release/nest-mqtt

# Custom MQTT broker
./target/release/nest-mqtt --broker 192.168.1.100:1883

# Custom topic prefix
./target/release/nest-mqtt --topic-prefix "home/nest"

# Full configuration
./target/release/nest-mqtt \
  --broker localhost:1883 \
  --client-id "nest-thermostat-1" \
  --topic-prefix "nest/livingroom"
```

### MQTT Topics

#### State Publishing (Thermostat → MQTT)

- `nest/thermostat/state` - Full JSON state
- `nest/thermostat/temperature` - Current temperature (°C)
- `nest/thermostat/humidity` - Current humidity (%)
- `nest/thermostat/target_temperature` - Target temperature (°C)
- `nest/thermostat/mode` - HVAC mode (`Off`, `Heat`, `Cool`, `HeatCool`, `Eco`)
- `nest/thermostat/fan_mode` - Fan mode (`Auto`, `On`, `Circulate`)
- `nest/thermostat/hvac_state` - HVAC state (`heating=X,cooling=Y,fan=Z`)
- `nest/thermostat/status` - Connection status
- `nest/thermostat/error` - Error messages

#### Control Commands (MQTT → Thermostat)

- `nest/thermostat/mode/set` - Set HVAC mode (`off`, `heat`, `cool`, `heatcool`, `eco`)
- `nest/thermostat/temperature/set` - Set target temperature (numeric)
- `nest/thermostat/temperature_range/set` - Set range (`low,high`)
- `nest/thermostat/fan/set` - Set fan mode (`auto`, `on`, `circulate`)
- `nest/thermostat/eco/set` - Set eco mode (`true`/`false`)

### Example MQTT Commands

```bash
# Set temperature to 22°C
mosquitto_pub -t nest/thermostat/temperature/set -m "22"

# Set mode to heat
mosquitto_pub -t nest/thermostat/mode/set -m "heat"

# Set temperature range for heat/cool mode
mosquitto_pub -t nest/thermostat/temperature_range/set -m "18,24"

# Turn fan on
mosquitto_pub -t nest/thermostat/fan/set -m "on"

# Enable eco mode
mosquitto_pub -t nest/thermostat/eco/set -m "true"
```

### Monitoring

```bash
# Subscribe to all topics
mosquitto_sub -t "nest/thermostat/#" -v

# Monitor temperature only
mosquitto_sub -t "nest/thermostat/temperature"
```

## Testing

Run the comprehensive test suite:

```bash
# Run all tests
./target/release/nest-test --test all

# Run specific tests
./target/release/nest-test --test serial
./target/release/nest-test --test sensor
./target/release/nest-test --test mqtt
./target/release/nest-test --test integration
```

### Test Categories

- **serial** - Tests serial port connection
- **init** - Tests backplate initialization
- **sensor** - Tests temperature/humidity reading
- **keepalive** - Tests keep-alive functionality
- **controller** - Tests thermostat control logic
- **mqtt** - Tests MQTT connectivity
- **integration** - Tests full system integration

## Configuration

### Command Line Options

| Option | Default | Description |
|--------|---------|-------------|
| `--broker` | `localhost:1883` | MQTT broker address |
| `--client-id` | `nest-thermostat` | MQTT client ID |
| `--topic-prefix` | `nest/thermostat` | MQTT topic prefix |
| `--serial-device` | `/dev/ttyO2` | Serial device path |

### Environment Variables

- `RUST_LOG` - Logging level (`debug`, `info`, `warn`, `error`)
- `MQTT_BROKER` - MQTT broker address (overrides `--broker`)

## Example Home Assistant Integration

```yaml
# configuration.yaml
mqtt:
  broker: 192.168.2.113
  port: 1883

climate:
  - platform: mqtt
    name: "Nest Thermostat"
    temperature_command_topic: "nest/thermostat/temperature/set"
    temperature_state_topic: "nest/thermostat/temperature"
    mode_command_topic: "nest/thermostat/mode/set"
    mode_state_topic: "nest/thermostat/mode"
    modes: ["off", "heat", "cool", "heatcool", "eco"]
    current_temperature_topic: "nest/thermostat/temperature"
    min_temp: 10
    max_temp: 35
    temp_step: 0.5
```

## Architecture

```
┌─────────────┐    ┌──────────────┐    ┌─────────────┐
│   MQTT      │    │   Nest MQTT  │    │   Nest      │
│   Broker    │◄──►│   Bridge     │◄──►│ Backplate   │
│             │    │              │    │             │
│ Mosquitto   │    │   Rust App   │    │ Serial      │
└─────────────┘    └──────────────┘    └─────────────┘
```

### Components

1. **Backplate Connection** - Serial communication with Nest backplate
2. **Protocol Handler** - Implements Nest binary protocol with CRC validation
3. **Thermostat Controller** - State management and control logic
4. **MQTT Client** - Publishes state and subscribes to commands
5. **Test Suite** - Comprehensive testing framework

## Troubleshooting

### Common Issues

1. **Permission denied on /dev/ttyO2**
   ```bash
   # Add user to dialout group or run as root
   sudo usermod -a -G dialout $USER
   # or
   sudo ./nest-mqtt
   ```

2. **MQTT connection failed**
   ```bash
   # Check if Mosquitto is running
   ps aux | grep mosquitto
   # Check broker connectivity
   telnet localhost 1883
   ```

3. **No sensor data**
   - Ensure thermostat is attached to backplate
   - Check serial logs with `RUST_LOG=debug ./nest-mqtt`
   - Verify initialization completed successfully

### Debug Logging

Enable debug logging:
```bash
RUST_LOG=debug ./nest-mqtt
```

### Logs

Monitor logs for:
- Protocol initialization sequence
- Sensor reading timestamps
- MQTT publish/subscribe events
- Error conditions and recovery

## Development

### Building

```bash
cargo build --release
```

### Testing

```bash
cargo test
./target/release/nest-test --test all
```

### Code Structure

- `src/main.rs` - Main application entry point
- `src/backplate.rs` - Serial communication implementation
- `src/protocol.rs` - Nest protocol definitions and parsing
- `src/thermostat.rs` - Thermostat state management
- `src/mqtt.rs` - MQTT client integration
- `src/test_suite.rs` - Comprehensive test suite

## License

MIT License - see LICENSE file for details.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## Support

For issues and questions:
- Check the test suite output for diagnostic information
- Enable debug logging for detailed protocol traces
- Review the protocol documentation for implementation details
