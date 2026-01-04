# BiBi-Sync

**A high-performance, lock-free inter-process communication library for single-host robotic systems.**

Built for latency-critical robotics applications where ROS overhead is unacceptable. Currently powering **Marco**, an Autonomous Underwater Vehicle (AUV).

---

## Why BiBi-Sync?

### The Problem with ROS on Single-Host Systems

ROS (Robot Operating System) was designed for distributed robotic systems where nodes may run on different machines. Even when all nodes run on the **same host**, ROS uses:

- **TCP/IP sockets** for inter-node communication
- **Serialization/deserialization** of every message
- **Kernel context switches** for each send/receive
- **Network stack overhead** (even on localhost)

This design makes sense for distributed systems but introduces unnecessary latency for single-host setups like embedded robotics.

### BiBi-Sync's Approach

BiBi-Sync takes a different approach optimized for single-host communication:

```
┌─────────────────────────────────────────────────────────────────┐
│                    ROS (localhost)                               │
│                                                                 │
│  Node A ──► Serialize ──► TCP Socket ──► Kernel ──►            │
│             Deserialize ◄── TCP Socket ◄── Kernel ◄── Node B   │
│                                                                 │
│  Overhead: ~100+ µs per message                                 │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                    BiBi-Sync                                     │
│                                                                 │
│  Thread A ──► Lock-Free Ring Buffer ──► Thread B               │
│               (Shared Memory)                                   │
│                                                                 │
│  Overhead: Single-digit µs per message                         │
└─────────────────────────────────────────────────────────────────┘
```

### Key Architectural Differences

| Aspect | ROS | BiBi-Sync |
|--------|-----|-----------|
| **Transport** | TCP/IP sockets | Shared memory |
| **Synchronization** | Kernel locks | Lock-free atomics |
| **Serialization** | Required | Zero-copy possible |
| **Context switches** | Multiple per message | None |
| **Designed for** | Distributed systems | Single-host systems |

---

## Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                         BiBi-Sync Architecture                        │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │                     TopicRegistry                            │    │
│  │  (Central registry for all named topics)                     │    │
│  └─────────────────────────────────────────────────────────────┘    │
│           │                            │                             │
│           ▼                            ▼                             │
│  ┌─────────────────┐          ┌─────────────────┐                   │
│  │  Topic<T>       │          │  ByteTopic       │                   │
│  │  (Typed)        │          │  (Variable-len)  │                   │
│  └────────┬────────┘          └────────┬────────┘                   │
│           │                            │                             │
│           ▼                            ▼                             │
│  ┌─────────────────┐          ┌─────────────────┐                   │
│  │  RingBuffer<T>  │          │  ByteRingBuffer  │                   │
│  │  (Lock-free)    │          │  (Lock-free)     │                   │
│  └─────────────────┘          └─────────────────┘                   │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │                      UART Bridge                             │    │
│  │  (Serial communication with microcontrollers)                │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

### Core Components

#### 1. Lock-Free Ring Buffer
The heart of BiBi-Sync is a **lock-free SPSC (Single-Producer Single-Consumer) ring buffer** based on the algorithm described in:

> **"Simple, Fast, and Practical Non-Blocking and Blocking Concurrent Queue Algorithms"**  
> Maged M. Michael and Michael L. Scott  
> PODC 1996  
> [https://www.cs.rochester.edu/~scott/papers/1996_PODC_queues.pdf](https://www.cs.rochester.edu/~scott/papers/1996_PODC_queues.pdf)

Key properties:
- **No locks** - Uses atomic operations only
- **No memory allocation** on the hot path
- **Cache-friendly** - Contiguous memory layout
- **Epoch-based versioning** for change detection

#### 2. Topic Registry
A thread-safe registry that maps topic names to shared ring buffers:
- `get_or_create<T>(name)` - Get or create a typed topic
- `get_or_create_byte(name)` - Get or create a byte topic

#### 3. Publisher/Subscriber
Lightweight handles to topics:
- **Publisher**: `publish(msg)` - Non-blocking, O(1)
- **Subscriber**: `receive()`, `peek_latest()`, `has_new()`

---

## Installation

### Rust
```toml
# Cargo.toml
[dependencies]
bibi-sync = { path = "../bibi-sync-rust" }
```

### Python
```bash
cd bibi-sync-rust
maturin build --release
pip install target/wheels/*.whl
```

### C/C++
```bash
cd bibi-sync-rust
cargo build --release
# Link against target/release/libbibi_sync.dylib (macOS)
# or target/release/libbibi_sync.so (Linux)
```

---

## Usage

### Rust

```rust
use bibi_sync::{TopicRegistry, Publisher, Subscriber};
use std::sync::Arc;

// Create a shared registry
let registry = Arc::new(TopicRegistry::new());

// Create a typed topic
let topic = registry.get_or_create::<f32>("sensor/temperature", 16);

// Publisher
let mut publisher = topic.publisher();
publisher.publish(25.5);

// Subscriber
let mut subscriber = topic.subscriber();
if subscriber.has_new() {
    if let Some(temp) = subscriber.receive() {
        println!("Temperature: {}", temp);
    }
}
```

### Python

```python
import bibi_sync

# Create registry
registry = bibi_sync.PyBibiRegistry()

# Create a byte topic
topic = registry.get_byte_topic("/sensors/imu", capacity=16)

# Publish
data = struct.pack('fff', 1.0, 2.0, 9.8)  # accel x, y, z
topic.publish(data)

# Receive
result = topic.receive()
if result:
    data, epoch = result
    ax, ay, az = struct.unpack('fff', bytes(data))
```

### C/C++

```c
#include "bibi_sync.h"

// Create registry
BibiRegistry* registry = bibi_registry_new();

// Create a byte topic
BibiByteTopic* topic = bibi_registry_get_byte_topic(registry, "/sensors/depth", 16);

// Publish
float depth = 2.5f;
bibi_byte_topic_publish(topic, (uint8_t*)&depth, sizeof(depth));

// Receive
uint8_t buffer[64];
size_t len;
uint64_t epoch;
if (bibi_byte_topic_receive(topic, buffer, sizeof(buffer), &len, &epoch) == 0) {
    float received_depth = *(float*)buffer;
}

// Cleanup
bibi_byte_topic_free(topic);
bibi_registry_free(registry);
```

---

## UART Bridge

BiBi-Sync includes a UART bridge for communicating with microcontrollers like STM32.

### Protocol Format

```
┌──────────┬──────────┬──────────┬─────────────────┬──────────────┐
│  SYNC    │   TYPE   │   LEN    │     PAYLOAD     │   CHECKSUM   │
│  0xAA    │  1 byte  │  1 byte  │   LEN bytes     │    1 byte    │
└──────────┴──────────┴──────────┴─────────────────┴──────────────┘
```

- **SYNC**: `0xAA` - Frame start marker
- **TYPE**: Message type identifier (see table below)
- **LEN**: Payload length (0-244 bytes)
- **PAYLOAD**: Message data
- **CHECKSUM**: Sum of TYPE + LEN + PAYLOAD bytes (mod 256)

### Message Types

| Type | Value | Direction | Description |
|------|-------|-----------|-------------|
| `MSG_IMU` | `0x01` | STM32 → Host | IMU sensor data (9 floats) |
| `MSG_DEPTH` | `0x02` | STM32 → Host | Depth sensor (1 float) |
| `MSG_THRUSTER` | `0x03` | Host → STM32 | Thruster PWM commands (6 int32s) |
| `MSG_HEARTBEAT` | `0x04` | Bidirectional | Heartbeat/status |
| `MSG_ORIENTATION` | `0x05` | STM32 → Host | Roll, pitch, yaw (3 floats) |
| `MSG_LED` | `0x12` | Host → STM32 | LED control (1 int16) |
| `MSG_CALIBRATION` | `0x13` | Host → STM32 | Calibration trigger (1 bool) |

### Usage (Rust)

```rust
use bibi_sync::{UartBridge, TopicRegistry, MsgType, ThrusterPwmCmd};
use std::sync::Arc;

// Create registry
let registry = Arc::new(TopicRegistry::new());

// Create UART bridge
let bridge = UartBridge::new("/dev/ttyACM0", 9600, Arc::clone(&registry))?;

// Start the bridge (spawns a background thread)
let (handle, running) = bridge.start();

// Received messages are automatically published to topics:
// - /stm32/imu
// - /stm32/depth
// - /stm32/orientation
// etc.

// Subscribe to sensor data
let imu_topic = registry.get_or_create_byte("/stm32/imu", 16);
let imu_sub = imu_topic.subscriber();

// Send thruster commands
let pwm_topic = registry.get_or_create_byte("/stm32/thruster", 8);
let pwm = ThrusterPwmCmd::new([1500, 1500, 1500, 1500, 1500, 1500]);
pwm_topic.publish(&pwm.to_bytes());

// Stop the bridge
bibi_sync::stop_bridge(&running);
handle.join().unwrap();
```

---

## STM32 Integration

BiBi-Sync provides a lightweight protocol library for STM32 microcontrollers.

### Installation

Copy these files to your STM32 project:
- `include/bibi_protocol.hpp`
- `src/bibi_protocol.cpp`

### Example (STM32)

```cpp
#include "bibi_protocol.hpp"

// Initialize
bibi_init(&Serial, 9600);

// Send IMU data
ImuMsg imu = {
    .accel_x = ax * G,
    .accel_y = ay * G,
    .accel_z = az * G,
    .gyro_x = gx,
    .gyro_y = gy,
    .gyro_z = gz,
    .mag_x = mx,
    .mag_y = my,
    .mag_z = mz
};
bibi_send(MSG_IMU, (uint8_t*)&imu, sizeof(imu));

// Process incoming commands (call in loop)
bibi_process();

// Implement callbacks
void onThrusterCmd(const ThrusterPwmCmd& cmd) {
    for (int i = 0; i < 6; i++) {
        thrusters[i].writeMicroseconds(cmd.pwm[i]);
    }
}

void onLedCmd(const LedCmd& cmd) {
    setLED(cmd.indicator);
}

void onCalibrationCmd(const CalibrationCmd& cmd) {
    if (cmd.enable) calibrate();
}
```

### Message Structures (C++)

```cpp
#pragma pack(push, 1)

struct ImuMsg {
    float accel_x, accel_y, accel_z;  // m/s²
    float gyro_x, gyro_y, gyro_z;     // rad/s
    float mag_x, mag_y, mag_z;        // µT
};

struct OrientationMsg {
    float roll, pitch, yaw;           // degrees
};

struct DepthMsg {
    float depth;                       // meters
};

struct ThrusterPwmCmd {
    int32_t pwm[6];                   // 1000-2000 µs
};

#pragma pack(pop)
```

---

## Project Structure

```
bibi-sync-rust/
├── src/
│   ├── lib.rs              # Library entry point
│   ├── ring_buffer/        # Lock-free ring buffer implementations
│   │   ├── mod.rs          # RingBuffer<T> (typed)
│   │   └── byte_buffer.rs  # ByteRingBuffer (variable-length)
│   ├── pubsub/             # Publisher/Subscriber abstraction
│   │   ├── topic.rs        # Topic<T> and ByteTopic
│   │   ├── publisher.rs    # Publisher handles
│   │   ├── subscriber.rs   # Subscriber handles
│   │   └── registry.rs     # TopicRegistry
│   ├── uart/               # UART bridge for microcontrollers
│   │   ├── mod.rs          # UartBridge
│   │   └── protocol.rs     # Message definitions
│   ├── ffi/                # C FFI bindings
│   └── python/             # Python bindings (PyO3)
├── include/
│   └── bibi_sync.h         # C header file
└── tests/
```

---

## License

MIT License

Copyright (c) 2024 Abinav

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

---

## Author

**Abinav**

Built for **Marco** - an Autonomous Underwater Vehicle (AUV).

---

## References

- Michael, M. M., & Scott, M. L. (1996). *Simple, Fast, and Practical Non-Blocking and Blocking Concurrent Queue Algorithms*. PODC '96.  
  [https://www.cs.rochester.edu/~scott/papers/1996_PODC_queues.pdf](https://www.cs.rochester.edu/~scott/papers/1996_PODC_queues.pdf)

---

## Acknowledgments

This project was built as a lightweight alternative to ROS for single-host robotic systems, specifically for underwater robotics where latency matters.