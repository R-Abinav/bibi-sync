/**
 * End-to-End Test for BiBi-Sync UART Bridge
 * 
 * Tests:
 * 1. Open UART connection to STM32
 * 2. Send PWM commands
 * 3. Receive sensor data (IMU, Orientation, Depth)
 */

use bibi_sync::{
    TopicRegistry, MsgType, ThrusterPwmCmd, ImuMsg, OrientationMsg, DepthMsg,
    SYNC_BYTE, MAX_MSG_SIZE,
};
use std::io::{Read, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};
use serialport::SerialPort;

const BAUD_RATE: u32 = 9600;

fn calculate_checksum(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b))
}

fn send_frame(port: &mut Box<dyn SerialPort>, msg_type: MsgType, payload: &[u8]) -> std::io::Result<()> {
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.push(SYNC_BYTE);
    frame.push(msg_type as u8);
    frame.push(payload.len() as u8);
    frame.extend_from_slice(payload);
    
    let checksum = calculate_checksum(&frame[1..]);
    frame.push(checksum);
    
    port.write_all(&frame)?;
    port.flush()?;
    
    println!("[TX] Sent {:?} frame, {} bytes payload", msg_type, payload.len());
    Ok(())
}

fn try_parse_frame(buffer: &mut Vec<u8>) -> Option<(MsgType, Vec<u8>)> {
    if buffer.len() < 4 {
        return None;
    }
    
    // Find sync byte
    let sync_pos = buffer.iter().position(|&b| b == SYNC_BYTE)?;
    if sync_pos > 0 {
        buffer.drain(0..sync_pos);
    }
    
    if buffer.len() < 4 {
        return None;
    }
    
    let msg_type_byte = buffer[1];
    let len = buffer[2] as usize;
    
    if len > MAX_MSG_SIZE {
        buffer.remove(0);
        return None;
    }
    
    let frame_len = 4 + len;
    if buffer.len() < frame_len {
        return None;
    }
    
    // Verify checksum
    let checksum = buffer[3 + len];
    let calculated = calculate_checksum(&buffer[1..3 + len]);
    
    if checksum != calculated {
        println!("[RX] Checksum mismatch: expected {}, got {}", calculated, checksum);
        buffer.remove(0);
        return None;
    }
    
    let msg_type = match msg_type_byte {
        0x01 => MsgType::Imu,
        0x02 => MsgType::Depth,
        0x05 => MsgType::Orientation,
        _ => {
            println!("[RX] Unknown message type: 0x{:02X}", msg_type_byte);
            buffer.drain(0..frame_len);
            return None;
        }
    };
    
    let payload = buffer[3..3 + len].to_vec();
    buffer.drain(0..frame_len);
    
    Some((msg_type, payload))
}

fn main() {
    println!("==============================================");
    println!("  BiBi-Sync STM32 End-to-End Test");
    println!("==============================================\n");
    
    // Find available ports
    let ports = serialport::available_ports().expect("No serial ports found!");
    println!("Available ports:");
    for (i, port) in ports.iter().enumerate() {
        println!("  [{}] {}", i, port.port_name);
    }
    
    // Use first available port or /dev/ttyACM0
    let port_name = if !ports.is_empty() {
        ports.iter()
            .find(|p| p.port_name.contains("ACM") || p.port_name.contains("USB"))
            .map(|p| p.port_name.clone())
            .unwrap_or_else(|| ports[0].port_name.clone())
    } else {
        "/dev/ttyACM0".to_string()
    };
    
    println!("\nOpening port: {} at {} baud...", port_name, BAUD_RATE);
    
    let mut port = serialport::new(&port_name, BAUD_RATE)
        .timeout(Duration::from_millis(100))
        .open()
        .expect(&format!("Failed to open port {}", port_name));
    
    println!("✅ Port opened successfully!\n");
    
    // Give STM32 time to initialize
    std::thread::sleep(Duration::from_secs(2));
    
    // Test 1: Send neutral PWM values
    println!("--- Test 1: Sending neutral PWM (1500) ---");
    let pwm_cmd = ThrusterPwmCmd::new([1500, 1500, 1500, 1500, 1500, 1500]);
    send_frame(&mut port, MsgType::Thruster, &pwm_cmd.to_bytes()).expect("Failed to send PWM");
    
    // Receive sensor data for 5 seconds
    println!("\n--- Receiving sensor data for 10 seconds ---\n");
    
    let mut rx_buffer = Vec::new();
    let mut read_buf = [0u8; 256];
    let start = Instant::now();
    
    let mut imu_count = 0;
    let mut orientation_count = 0;
    let mut depth_count = 0;
    
    while start.elapsed() < Duration::from_secs(10) {
        match port.read(&mut read_buf) {
            Ok(n) if n > 0 => {
                rx_buffer.extend_from_slice(&read_buf[..n]);
                
                while let Some((msg_type, payload)) = try_parse_frame(&mut rx_buffer) {
                    match msg_type {
                        MsgType::Imu => {
                            if let Some(imu) = ImuMsg::from_bytes(&payload) {
                                imu_count += 1;
                                if imu_count % 10 == 1 {
                                    println!("[IMU] accel=({:.2}, {:.2}, {:.2}) gyro=({:.2}, {:.2}, {:.2})",
                                        imu.accel_x, imu.accel_y, imu.accel_z,
                                        imu.gyro_x, imu.gyro_y, imu.gyro_z);
                                }
                            }
                        }
                        MsgType::Orientation => {
                            if let Some(orient) = OrientationMsg::from_bytes(&payload) {
                                orientation_count += 1;
                                if orientation_count % 10 == 1 {
                                    println!("[ORIENT] roll={:.1}° pitch={:.1}° yaw={:.1}°",
                                        orient.roll, orient.pitch, orient.yaw);
                                }
                            }
                        }
                        MsgType::Depth => {
                            if let Some(depth) = DepthMsg::from_bytes(&payload) {
                                depth_count += 1;
                                if depth_count % 10 == 1 {
                                    println!("[DEPTH] {:.3} m", depth.depth);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(_) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => {
                eprintln!("Read error: {}", e);
            }
        }
    }
    
    println!("\n--- Test 2: Sending test PWM ramp ---");
    
    // Ramp up thrusters slightly
    for pwm in [1500, 1520, 1540, 1520, 1500] {
        let pwm_cmd = ThrusterPwmCmd::new([pwm; 6]);
        send_frame(&mut port, MsgType::Thruster, &pwm_cmd.to_bytes()).expect("Failed to send PWM");
        println!("[TX] PWM = {}", pwm);
        std::thread::sleep(Duration::from_millis(500));
    }
    
    println!("\n==============================================");
    println!("  Test Complete!");
    println!("==============================================");
    println!("  IMU messages received:         {}", imu_count);
    println!("  Orientation messages received: {}", orientation_count);
    println!("  Depth messages received:       {}", depth_count);
    println!("==============================================\n");
    
    if imu_count > 0 && orientation_count > 0 && depth_count > 0 {
        println!("✅ All sensor types received - SUCCESS!");
    } else {
        println!("⚠️  Some sensor data missing - check STM32 firmware");
    }
}
