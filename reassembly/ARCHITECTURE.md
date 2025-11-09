# Architecture Overview

## System Layers

```
┌─────────────────────────────────────────────────────────┐
│                     Application                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │  Main Loop   │  │   Screens    │  │    Events    │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────┐
│                     UI Framework                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │  Layer Mgr   │  │  Screen Mgr  │  │  Widgets     │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────┐
│                   State Management                       │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │  HVAC State  │  │   Schedule   │  │   Settings   │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────┐
│              Hardware Abstraction Layer                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │   Display    │  │   Encoder    │  │   Sensors    │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────┐
│                       Hardware                           │
│   LCD/OLED    Rotary Encoder    Temp/Humidity Sensors   │
└─────────────────────────────────────────────────────────┘
```

## Component Interaction

```
┌──────────────┐
│    Input     │  (Encoder Rotate)
│   Hardware   │
└──────┬───────┘
       │
       ↓
┌──────────────┐
│    Event     │  Event::EncoderRotate { delta: 1 }
│   System     │
└──────┬───────┘
       │
       ↓
┌──────────────┐
│   Screen     │  Receives event, routes to layers
│   Manager    │
└──────┬───────┘
       │
       ↓
┌──────────────┐
│    Layer     │  TemperatureSlider handles event
│   (Active)   │  Adjusts temperature +0.5°C
└──────┬───────┘
       │
       ↓
┌──────────────┐
│  State Mgr   │  Updates HvacState.target_temp
└──────┬───────┘
       │
       ↓
┌──────────────┐
│   Render     │  EventResult::NeedsRedraw
│   Pipeline   │  Redraws affected layers
└──────┬───────┘
       │
       ↓
┌──────────────┐
│   Display    │  Pixels written to screen
│   Hardware   │
└──────────────┘
```

## Layer Hierarchy Example

```
SettingsScreen
├── BackgroundLayer (z-order: 0)
│   └── Renders: gradient background
├── TitleLayer (z-order: 10)
│   └── Renders: "Settings" text
├── MenuLayer (z-order: 20)
│   └── Renders: menu items
│   └── Handles: encoder rotation, selection
└── StatusLayer (z-order: 100)
    └── Renders: WiFi icon, battery
```

## State Machine: HVAC Mode

```
        ┌───────┐
        │  Off  │
        └───┬───┘
            │
    ┌───────┼───────┬────────┐
    │       │       │        │
    ↓       ↓       ↓        ↓
┌──────┐ ┌────┐ ┌──────┐ ┌─────┐
│ Heat │ │Cool│ │Heat/ │ │ Eco │
│      │ │    │ │Cool  │ │     │
└──────┘ └────┘ └──────┘ └─────┘
    │       │       │        │
    └───────┴───────┴────────┘
            │
            ↓
        ┌───────┐
        │  Off  │
        └───────┘
```

## Event Flow

```
User Action → Hardware → Driver → Event
                                    ↓
                            ┌───────────────┐
                            │ Event Queue   │
                            └───────┬───────┘
                                    ↓
                            ┌───────────────┐
                            │ Screen Manager│
                            └───────┬───────┘
                                    ↓
                    ┌───────────────┴───────────────┐
                    ↓                               ↓
            ┌───────────────┐              ┌───────────────┐
            │ Active Screen │              │  Background   │
            │    Layers     │              │   Processes   │
            └───────┬───────┘              └───────────────┘
                    ↓
        ┌───────────┴───────────┐
        ↓           ↓           ↓
    Layer 1     Layer 2     Layer 3
        │           │           │
        └───────────┴───────────┘
                    ↓
            EventResult::Handled
                    │
                    ↓
            Update Display
```

## Thread Architecture (Async)

```
┌─────────────────────────────────────────────┐
│           Main Tokio Runtime                │
│                                             │
│  ┌─────────────┐  ┌──────────────────────┐ │
│  │  UI Loop    │  │  Background Tasks    │ │
│  │  (60 FPS)   │  │                      │ │
│  │             │  │  - Network Sync      │ │
│  │  - Events   │  │  - Sensor Reading    │ │
│  │  - Update   │  │  - API Calls         │ │
│  │  - Render   │  │  - Schedule Check    │ │
│  └─────────────┘  └──────────────────────┘ │
└─────────────────────────────────────────────┘
            ↓                    ↓
    ┌──────────────┐    ┌──────────────┐
    │   Display    │    │   Network    │
    │   Hardware   │    │   Hardware   │
    └──────────────┘    └──────────────┘
```

## Memory Layout (Embedded Target)

```
┌─────────────────────────────────────┐
│             Flash ROM               │  ← Program code
│  - Rust binary (~500KB optimized)  │
│  - Fonts, icons, UI assets         │
│  - Configuration tables            │
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│               SRAM                  │  ← Runtime data
│  - Stack (32KB)                    │
│  - Heap (dynamic allocations)      │
│  - Layer objects                   │
│  - State machines                  │
│  - Frame buffer (if used)          │
└─────────────────────────────────────┘
```

## Build Pipeline

```
Rust Source
    ↓
┌─────────────┐
│  rustc      │  Compile with optimizations
└──────┬──────┘
       ↓
┌─────────────┐
│    LTO      │  Link-Time Optimization
└──────┬──────┘
       ↓
┌─────────────┐
│   Strip     │  Remove debug symbols
└──────┬──────┘
       ↓
┌─────────────┐
│  ARM Binary │  Ready for embedded target
└─────────────┘
```

## Original vs. Reimplementation

```
┌─────────────────────────────────────────────────────┐
│              Original (nlclient)                    │
├─────────────────────────────────────────────────────┤
│ Language:    C++ with Boost                         │
│ Size:        5.1 MB (stripped)                      │
│ Safety:      Manual memory management               │
│ Async:       Custom event loop                      │
│ Build:       Complex makefiles                      │
│ Deps:        ~30 system libraries                   │
└─────────────────────────────────────────────────────┘
                        ↓ Modernized
┌─────────────────────────────────────────────────────┐
│            Rust Reimplementation                    │
├─────────────────────────────────────────────────────┤
│ Language:    Rust 2021 Edition                      │
│ Size:        ~500 KB (optimized)                    │
│ Safety:      Compile-time memory safety             │
│ Async:       Tokio runtime                          │
│ Build:       Cargo (simple)                         │
│ Deps:        Modern Rust crates                     │
└─────────────────────────────────────────────────────┘
```

## Key Design Patterns

### 1. Trait-Based Polymorphism
```rust
trait Layer {
    fn render(&self, display: &mut Display) -> Result<()>;
}
```

### 2. Composition Over Inheritance
```rust
struct Screen {
    layers: LayerManager,  // Composes multiple layers
}
```

### 3. Event-Driven Architecture
```rust
enum Event {
    EncoderRotate { delta: i32 },
    TemperatureUpdate { celsius: f32 },
}
```

### 4. State Machine Pattern
```rust
enum HvacMode {
    Off, Heat, Cool, HeatCool, Eco
}

impl HvacState {
    fn transition(&mut self, new_mode: HvacMode) { ... }
}
```

### 5. Dependency Injection
```rust
struct App {
    display: Box<dyn Display>,
    sensors: Box<dyn Sensors>,
}
```

## Performance Characteristics

```
Operation              | Time        | Notes
-----------------------|-------------|------------------
Frame render           | < 16ms      | 60 FPS target
Event processing       | < 1ms       | Instant response
State update           | < 100µs     | Minimal work
Sensor reading         | < 10ms      | I2C/SPI bound
Network sync           | async       | Non-blocking
```

## Scalability

```
Current:  ~1,500 LOC, 6 modules
Target:   ~50,000 LOC, 50+ modules

Maintainable because:
- Clear module boundaries
- Type-safe interfaces
- Compile-time checks
- Comprehensive tests
- Good documentation
```
