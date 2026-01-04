/**
 * AUV Controller
 * 
 * Main controller that:
 * 1. Connects to STM32 via UART
 * 2. Receives sensor data (IMU, depth, orientation)
 * 3. Accepts thrust commands from Python/threads
 * 4. Sends PWM commands to STM32
 */

use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::Duration;
use std::io::{Read, Write};

use crate::pubsub::TopicRegistry;
use crate::{MsgType, ThrusterPwmCmd, ImuMsg, OrientationMsg, DepthMsg};
use super::thrust_mixer::{ThrustMixer, ThrustCommand};

const SYNC_BYTE: u8 = 0xAA;
const MAX_MSG_SIZE: usize = 244;
const DEFAULT_BAUD: u32 = 9600;

/// Latest sensor readings from STM32
#[derive(Debug, Clone, Default)]
pub struct SensorData {
    pub imu: Option<ImuMsg>,
    pub orientation: Option<OrientationMsg>,
    pub depth: Option<DepthMsg>,
}

/// AUV Controller - unified control system
pub struct AuvController {
    registry: Arc<TopicRegistry>,
    mixer: ThrustMixer,
    running: Arc<AtomicBool>,
    port_name: String,
    baud_rate: u32,
    
    // Latest sensor data (thread-safe)
    sensors: Arc<std::sync::RwLock<SensorData>>,
    
    // Current thrust command
    thrust_cmd: Arc<std::sync::RwLock<ThrustCommand>>,
}

impl AuvController {
    pub fn new(port_name: &str) -> Self {
        Self {
            registry: Arc::new(TopicRegistry::new()),
            mixer: ThrustMixer::default(),
            running: Arc::new(AtomicBool::new(false)),
            port_name: port_name.to_string(),
            baud_rate: DEFAULT_BAUD,
            sensors: Arc::new(std::sync::RwLock::new(SensorData::default())),
            thrust_cmd: Arc::new(std::sync::RwLock::new(ThrustCommand::default())),
        }
    }
    
    pub fn with_baud(mut self, baud: u32) -> Self {
        self.baud_rate = baud;
        self
    }
    
    /// Set thrust command (called from Python or other threads)
    pub fn set_thrust(&self, cmd: ThrustCommand) {
        *self.thrust_cmd.write().unwrap() = cmd;
    }
    
    /// Set individual DoF thrust
    pub fn set_surge(&self, value: f32) {
        self.thrust_cmd.write().unwrap().surge = value;
    }
    
    pub fn set_sway(&self, value: f32) {
        self.thrust_cmd.write().unwrap().sway = value;
    }
    
    pub fn set_heave(&self, value: f32) {
        self.thrust_cmd.write().unwrap().heave = value;
    }
    
    pub fn set_roll(&self, value: f32) {
        self.thrust_cmd.write().unwrap().roll = value;
    }
    
    pub fn set_pitch(&self, value: f32) {
        self.thrust_cmd.write().unwrap().pitch = value;
    }
    
    pub fn set_yaw(&self, value: f32) {
        self.thrust_cmd.write().unwrap().yaw = value;
    }
    
    /// Get latest sensor data
    pub fn get_sensors(&self) -> SensorData {
        self.sensors.read().unwrap().clone()
    }
    
    /// Get current orientation (roll, pitch, yaw in degrees)
    pub fn get_orientation(&self) -> Option<(f32, f32, f32)> {
        self.sensors.read().unwrap().orientation.as_ref()
            .map(|o| (o.roll, o.pitch, o.yaw))
    }
    
    /// Get current depth in meters
    pub fn get_depth(&self) -> Option<f32> {
        self.sensors.read().unwrap().depth.as_ref().map(|d| d.depth)
    }
    
    /// Stop all thrusters
    pub fn stop(&self) {
        self.set_thrust(ThrustCommand::default());
    }
    
    /// Start the controller (blocking)
    pub fn run(&self) {
        self.running.store(true, Ordering::SeqCst);
        
        println!("[AUV] Opening port {} at {} baud...", self.port_name, self.baud_rate);
        
        let mut port = serialport::new(&self.port_name, self.baud_rate)
            .timeout(Duration::from_millis(100))
            .open()
            .expect(&format!("Failed to open port {}", self.port_name));
        
        println!("[AUV] Connected to STM32!");
        
        let mut rx_buffer = Vec::new();
        let mut read_buf = [0u8; 256];
        let mut last_tx = std::time::Instant::now();
        
        while self.running.load(Ordering::SeqCst) {
            // Read incoming sensor data
            match port.read(&mut read_buf) {
                Ok(n) if n > 0 => {
                    rx_buffer.extend_from_slice(&read_buf[..n]);
                    self.process_rx(&mut rx_buffer);
                }
                Ok(_) => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => eprintln!("[AUV] Read error: {}", e),
            }
            
            // Send thrust commands at 50Hz
            if last_tx.elapsed() >= Duration::from_millis(20) {
                last_tx = std::time::Instant::now();
                
                let cmd = self.thrust_cmd.read().unwrap().clone();
                let thrusts = self.mixer.mix(&cmd);
                let pwm = ThrustMixer::to_pwm(&thrusts);
                
                let pwm_cmd = ThrusterPwmCmd::new(pwm);
                self.send_frame(&mut port, MsgType::Thruster, &pwm_cmd.to_bytes());
            }
        }
        
        // Stop thrusters on exit
        println!("[AUV] Stopping thrusters...");
        let pwm_cmd = ThrusterPwmCmd::new([1500; 6]);
        self.send_frame(&mut port, MsgType::Thruster, &pwm_cmd.to_bytes());
        
        println!("[AUV] Shutdown complete");
    }
    
    /// Start in background thread
    pub fn start_background(self: Arc<Self>) -> thread::JoinHandle<()> {
        let controller = self.clone();
        thread::spawn(move || {
            controller.run();
        })
    }
    
    /// Signal shutdown
    pub fn shutdown(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
    
    fn send_frame(&self, port: &mut Box<dyn serialport::SerialPort>, msg_type: MsgType, payload: &[u8]) {
        let mut frame = Vec::with_capacity(4 + payload.len());
        frame.push(SYNC_BYTE);
        frame.push(msg_type as u8);
        frame.push(payload.len() as u8);
        frame.extend_from_slice(payload);
        
        let checksum = Self::calculate_checksum(&frame[1..]);
        frame.push(checksum);
        
        let _ = port.write_all(&frame);
        let _ = port.flush();
    }
    
    fn calculate_checksum(data: &[u8]) -> u8 {
        data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b))
    }
    
    fn process_rx(&self, buffer: &mut Vec<u8>) {
        while let Some((msg_type, payload)) = Self::try_parse_frame(buffer) {
            match msg_type {
                MsgType::Imu => {
                    if let Some(imu) = ImuMsg::from_bytes(&payload) {
                        self.sensors.write().unwrap().imu = Some(imu);
                    }
                }
                MsgType::Orientation => {
                    if let Some(orient) = OrientationMsg::from_bytes(&payload) {
                        self.sensors.write().unwrap().orientation = Some(orient);
                    }
                }
                MsgType::Depth => {
                    if let Some(depth) = DepthMsg::from_bytes(&payload) {
                        self.sensors.write().unwrap().depth = Some(depth);
                    }
                }
                _ => {}
            }
        }
    }
    
    fn try_parse_frame(buffer: &mut Vec<u8>) -> Option<(MsgType, Vec<u8>)> {
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
        let calculated = Self::calculate_checksum(&buffer[1..3 + len]);
        
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
        
        Some((msg_type, payload))
    }
}
