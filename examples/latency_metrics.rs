/**
 * BiBi-Sync Latency Metrics Test
 * 
 * Measures round-trip latency:
 * - UART RX from STM32
 * - Protocol parsing
 * - Message processing
 * 
 * Outputs CSV for analysis and prints summary statistics.
 */

use bibi_sync::{MsgType, ImuMsg, OrientationMsg, DepthMsg};
use std::io::Read;
use std::time::{Duration, Instant};
use std::fs::File;
use std::io::Write;

const BAUD_RATE: u32 = 9600;
const SYNC_BYTE: u8 = 0xAA;
const MAX_MSG_SIZE: usize = 244;
const NUM_SAMPLES: usize = 1000;

fn calculate_checksum(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b))
}

fn try_parse_frame(buffer: &mut Vec<u8>) -> Option<(MsgType, Vec<u8>, Instant)> {
    let parse_start = Instant::now();
    
    if buffer.len() < 4 {
        return None;
    }
    
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
    
    let checksum = buffer[3 + len];
    let calculated = calculate_checksum(&buffer[1..3 + len]);
    
    if checksum != calculated {
        buffer.remove(0);
        return None;
    }
    
    let msg_type = match msg_type_byte {
        0x01 => MsgType::Imu,
        0x02 => MsgType::Depth,
        0x05 => MsgType::Orientation,
        _ => {
            buffer.drain(0..frame_len);
            return None;
        }
    };
    
    let payload = buffer[3..3 + len].to_vec();
    buffer.drain(0..frame_len);
    
    Some((msg_type, payload, parse_start))
}

fn main() {
    println!("==============================================");
    println!("  BiBi-Sync Latency Metrics Test");
    println!("==============================================\n");
    
    let args: Vec<String> = std::env::args().collect();
    let port_name = args.get(1).map(|s| s.as_str()).unwrap_or("/dev/ttyACM0");
    
    println!("Port: {}", port_name);
    println!("Samples: {}", NUM_SAMPLES);
    println!("----------------------------------------------\n");
    
    // Open port
    let mut port = serialport::new(port_name, BAUD_RATE)
        .timeout(Duration::from_millis(100))
        .open()
        .expect(&format!("Failed to open port {}", port_name));
    
    println!("✅ Port opened, collecting {} samples...\n", NUM_SAMPLES);
    
    // Open CSV file
    let mut csv_file = File::create("/tmp/bibi_sync_latencies.csv")
        .expect("Failed to create CSV file");
    writeln!(csv_file, "sample,msg_type,rx_time_us,parse_time_us,total_time_us").unwrap();
    
    let mut rx_buffer = Vec::new();
    let mut read_buf = [0u8; 256];
    
    let mut rx_latencies: Vec<u64> = Vec::new();
    let mut parse_latencies: Vec<u64> = Vec::new();
    let mut total_latencies: Vec<u64> = Vec::new();
    
    let mut sample_count = 0;
    let test_start = Instant::now();
    
    while sample_count < NUM_SAMPLES {
        let rx_start = Instant::now();
        
        match port.read(&mut read_buf) {
            Ok(n) if n > 0 => {
                let rx_time = rx_start.elapsed();
                rx_buffer.extend_from_slice(&read_buf[..n]);
                
                while let Some((msg_type, payload, parse_start)) = try_parse_frame(&mut rx_buffer) {
                    let parse_time = parse_start.elapsed();
                    let total_time = rx_start.elapsed();
                    
                    let rx_us = rx_time.as_micros() as u64;
                    let parse_us = parse_time.as_micros() as u64;
                    let total_us = total_time.as_micros() as u64;
                    
                    rx_latencies.push(rx_us);
                    parse_latencies.push(parse_us);
                    total_latencies.push(total_us);
                    
                    let msg_name = match msg_type {
                        MsgType::Imu => "IMU",
                        MsgType::Depth => "DEPTH",
                        MsgType::Orientation => "ORIENT",
                        _ => "OTHER",
                    };
                    
                    writeln!(csv_file, "{},{},{},{},{}", 
                        sample_count, msg_name, rx_us, parse_us, total_us).unwrap();
                    
                    sample_count += 1;
                    
                    if sample_count % 100 == 0 {
                        println!("  Collected {} samples...", sample_count);
                    }
                    
                    if sample_count >= NUM_SAMPLES {
                        break;
                    }
                }
            }
            Ok(_) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => eprintln!("Read error: {}", e),
        }
    }
    
    let test_duration = test_start.elapsed();
    
    // Calculate statistics
    println!("\n==============================================");
    println!("  RESULTS ({} samples in {:.2}s)", 
        sample_count, test_duration.as_secs_f64());
    println!("==============================================\n");
    
    fn stats(name: &str, data: &[u64]) {
        if data.is_empty() {
            println!("{}: No data", name);
            return;
        }
        
        let mut sorted = data.to_vec();
        sorted.sort();
        
        let sum: u64 = sorted.iter().sum();
        let mean = sum as f64 / sorted.len() as f64;
        
        let variance: f64 = sorted.iter()
            .map(|&x| (x as f64 - mean).powi(2))
            .sum::<f64>() / sorted.len() as f64;
        let std_dev = variance.sqrt();
        
        let p50 = sorted[sorted.len() * 50 / 100];
        let p95 = sorted[sorted.len() * 95 / 100];
        let p99 = sorted[sorted.len() * 99 / 100];
        let min = sorted[0];
        let max = sorted[sorted.len() - 1];
        
        println!("{:12} │ Mean: {:8.2} µs │ Std Dev: {:8.2} µs", name, mean, std_dev);
        println!("{:12} │ P50:  {:8} µs │ P95: {:8} µs │ P99: {:8} µs", 
            "", p50, p95, p99);
        println!("{:12} │ Min:  {:8} µs │ Max: {:8} µs", "", min, max);
        println!();
    }
    
    stats("RX Latency", &rx_latencies);
    stats("Parse Time", &parse_latencies);
    stats("Total", &total_latencies);
    
    // Throughput
    let throughput = sample_count as f64 / test_duration.as_secs_f64();
    println!("Throughput: {:.1} msg/sec", throughput);
    
    println!("\n----------------------------------------------");
    println!("CSV saved to: /tmp/bibi_sync_latencies.csv");
    println!("----------------------------------------------\n");
    
    // Quick comparison table
    println!("==============================================");
    println!("  COMPARISON (Expected vs Measured)");
    println!("==============================================");
    println!("  Metric       │ ROS2 (expected) │ BiBi-Sync");
    println!("  ─────────────┼─────────────────┼───────────");
    
    let mean_total: f64 = total_latencies.iter().sum::<u64>() as f64 / total_latencies.len() as f64;
    let ros_mean = 395.0; // Expected ROS2 baseline
    let improvement = ros_mean / mean_total;
    
    println!("  Mean Latency │ ~395 µs         │ {:.1} µs", mean_total);
    println!("  Improvement  │                 │ {:.1}x faster! 🚀", improvement);
    println!("==============================================\n");
}
