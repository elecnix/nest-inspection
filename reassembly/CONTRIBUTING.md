# Contributing to Nest UI Rust

## Getting Started

### Prerequisites
- Rust 1.70 or later
- Basic understanding of embedded systems
- Familiarity with async Rust (tokio)

### Building
```bash
cd nest-ui-rs
cargo build --release
```

### Running Tests
```bash
cargo test
```

### Running the Demo
```bash
cargo run --release
```

## Adding a New UI Layer

To add a new layer (e.g., a menu layer):

1. **Create the layer file**: `src/ui/menu.rs`

```rust
use anyhow::Result;
use async_trait::async_trait;

use crate::core::{
    display::SimulatorDisplay,
    event::{Event, EventResult},
    layer::{Layer, LayerId},
};

pub struct MenuLayer {
    id: LayerId,
    visible: bool,
    enabled: bool,
    selected_item: usize,
    items: Vec<String>,
}

impl MenuLayer {
    pub fn new(id: LayerId, items: Vec<String>) -> Self {
        Self {
            id,
            visible: true,
            enabled: true,
            selected_item: 0,
            items,
        }
    }
}

#[async_trait]
impl Layer for MenuLayer {
    fn id(&self) -> &LayerId {
        &self.id
    }
    
    fn render(&self, display: &mut SimulatorDisplay) -> Result<()> {
        // Implement rendering logic here
        Ok(())
    }
    
    fn handle_event(&mut self, event: &Event) -> Result<EventResult> {
        match event {
            Event::EncoderRotate { delta } => {
                // Navigate menu
                let new_pos = (self.selected_item as i32 + delta)
                    .max(0)
                    .min((self.items.len() - 1) as i32);
                self.selected_item = new_pos as usize;
                Ok(EventResult::NeedsRedraw)
            }
            Event::EncoderPress => {
                // Select item
                Ok(EventResult::Handled)
            }
            _ => Ok(EventResult::NotHandled),
        }
    }
    
    fn is_visible(&self) -> bool {
        self.visible
    }
    
    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}
```

2. **Export from module**: Add to `src/ui/mod.rs`
```rust
pub mod menu;
pub use menu::MenuLayer;
```

3. **Use in your application**:
```rust
let menu = MenuLayer::new(
    "main_menu".to_string(),
    vec!["Schedule".to_string(), "Settings".to_string()],
);
layer_manager.add_layer(Box::new(menu));
```

## Adding a New Screen

Screens are collections of layers:

```rust
use crate::core::{
    display::SimulatorDisplay,
    event::{Event, EventResult},
    layer::LayerManager,
    screen::{Screen, ScreenId},
};

pub struct SettingsScreen {
    id: ScreenId,
    layers: LayerManager,
}

impl SettingsScreen {
    pub fn new() -> Self {
        let mut layers = LayerManager::new();
        
        // Add background layer
        let bg = BackgroundLayer::new("background".to_string());
        layers.add_layer(Box::new(bg));
        
        // Add menu layer
        let menu = MenuLayer::new(
            "settings_menu".to_string(),
            vec!["Language".to_string(), "Brightness".to_string()],
        );
        layers.add_layer(Box::new(menu));
        
        Self {
            id: "settings".to_string(),
            layers,
        }
    }
}

#[async_trait]
impl Screen for SettingsScreen {
    fn id(&self) -> &ScreenId {
        &self.id
    }
    
    async fn init(&mut self) -> Result<()> {
        Ok(())
    }
    
    fn layers(&self) -> &LayerManager {
        &self.layers
    }
    
    fn layers_mut(&mut self) -> &mut LayerManager {
        &mut self.layers
    }
    
    async fn on_enter(&mut self) -> Result<()> {
        // Initialize when screen becomes active
        Ok(())
    }
    
    async fn on_exit(&mut self) -> Result<()> {
        // Cleanup when leaving screen
        Ok(())
    }
}
```

## Adding New Events

1. **Define event** in `src/core/event.rs`:
```rust
pub enum Event {
    // ... existing events
    
    /// New custom event
    CustomAlert { message: String },
}
```

2. **Handle in layers**:
```rust
fn handle_event(&mut self, event: &Event) -> Result<EventResult> {
    match event {
        Event::CustomAlert { message } => {
            log::info!("Alert: {}", message);
            Ok(EventResult::Handled)
        }
        _ => Ok(EventResult::NotHandled),
    }
}
```

## Hardware Integration

### Adding a Display Driver

1. **Create driver** in `src/hardware/display.rs`:
```rust
pub struct Lcd320x480 {
    // Hardware-specific fields
}

impl DrawTarget for Lcd320x480 {
    type Color = Rgb565;
    type Error = DisplayError;
    
    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        // Write pixels to hardware
        Ok(())
    }
}
```

2. **Update core** to use concrete type or add trait bounds

### Adding Sensor Support

1. **Create sensor driver** in `src/hardware/sensors.rs`:
```rust
pub struct TemperatureSensor {
    // I2C or ADC interface
}

impl TemperatureSensor {
    pub async fn read_celsius(&mut self) -> Result<f32> {
        // Read from hardware
        Ok(25.0)
    }
}
```

2. **Use in main loop**:
```rust
let mut sensor = TemperatureSensor::new(i2c);
let temp = sensor.read_celsius().await?;
let event = Event::TemperatureUpdate { celsius: temp };
```

## Testing

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_temperature_range() {
        let mut state = HvacState::default();
        state.current_temp = 18.0;
        state.target_temp = Some(21.0);
        
        assert!(state.needs_heating());
        assert!(!state.needs_cooling());
    }
}
```

### Integration Tests
Create `tests/integration_test.rs`:
```rust
use nest_ui::core::*;

#[tokio::test]
async fn test_screen_transition() {
    let mut manager = ScreenManager::new();
    // Add test screens
    // Test transitions
}
```

## Code Style

- Follow Rust conventions (rustfmt)
- Run `cargo clippy` before committing
- Add documentation for public APIs
- Keep functions focused and small
- Use meaningful variable names

## Performance Considerations

- Profile with `cargo flamegraph`
- Minimize allocations in render loop
- Use `&str` over `String` where possible
- Consider `no_std` compatibility for embedded

## Cross-Compilation for ARM

```bash
# Add target
rustup target add armv7-unknown-linux-gnueabihf

# Build
cargo build --release --target armv7-unknown-linux-gnueabihf
```

## Documentation

Generate and view docs:
```bash
cargo doc --open
```

## Questions?

See the main README.md and ANALYSIS.md for more details.
