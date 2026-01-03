pub mod protocol;
pub use protocol::*;

use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use serialport::SerialPort;
use crate::pubsub::{TopicRegistry, ByteTopic};

pub const SYNC_BYTE: u8 = 0xAA;
pub const MAX_MSG_SIZE: usize = 244;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MsgType{
    Imu = 0x01,
    Depth = 0x02,
    Thruster = 0x03,
    Heartbeat = 0x04,
    Command = 0x10,
    Ack = 0x11,
}

impl MsgType{
    fn from_u8(val: u8) -> Option<Self>{
        match val{
            0x01 => Some(MsgType::Imu),
            0x02 => Some(MsgType::Depth),
            0x03 => Some(MsgType::Thruster),
            0x04 => Some(MsgType::Heartbeat),
            0x10 => Some(MsgType::Command),
            0x11 => Some(MsgType::Ack),
            _ => None,
        }
    }

    fn to_topic_name(&self) -> &'static str{
        match self{
            MsgType::Imu => "/stm32/imu",
            MsgType::Depth => "/stm32/depth",
            MsgType::Thruster => "/stm32/thruster",
            MsgType::Heartbeat => "/stm32/heartbeat",
            MsgType::Command => "/stm32/command",
            MsgType::Ack => "/stm32/ack",
        }
    }
}

#[derive(Debug, Clone)]
pub struct UartFrame{
    pub msg_type: MsgType,
    pub payload: Vec<u8>,
}

pub struct UartBridge{
    port: Box<dyn SerialPort>,
    registry: Arc<TopicRegistry>,
    running: Arc<AtomicBool>,
    rx_buffer: Vec<u8>,
}

impl UartBridge{
    pub fn new(port_name: &str, baud_rate: u32, registry: Arc<TopicRegistry>) -> Result<Self, serialport::Error>{
        let port = serialport::new(port_name, baud_rate)
            .timeout(Duration::from_millis(10))
            .open()?;

        Ok(UartBridge{
            port,
            registry,
            running: Arc::new(AtomicBool::new(false)),
            rx_buffer: Vec::with_capacity(512),
        })
    }

    pub fn start(mut self) -> (JoinHandle<()>, Arc<AtomicBool>){
        let running = Arc::clone(&self.running);
        self.running.store(true, Ordering::SeqCst);

        let handle = thread::spawn(move ||{
            self.run_loop();
        });

        (handle, running)
    }

    fn run_loop(&mut self){
        let mut read_buf = [0u8; 256];

        while self.running.load(Ordering::SeqCst){
            match self.port.read(&mut read_buf){
                Ok(n) if n > 0 =>{
                    self.rx_buffer.extend_from_slice(&read_buf[..n]);
                    self.process_buffer();
                }
                Ok(_) => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) =>{
                    eprintln!("UART read error: {}", e);
                }
            }
        }
    }

    fn process_buffer(&mut self){
        loop{
            if let Some(frame) = self.try_parse_frame(){
                self.publish_frame(&frame);
            }else{
                break;
            }
        }
    }

    fn try_parse_frame(&mut self) -> Option<UartFrame>{
        //frame format: [SYNC][TYPE][LEN][PAYLOAD...][CHECKSUM]
        //              0xAA  1byte 1byte  LEN bytes   1byte

        if self.rx_buffer.len() < 4{
            return None;
        }

        //find sync byte
        let sync_pos = self.rx_buffer.iter().position(|&b| b == SYNC_BYTE)?;
        
        if sync_pos > 0{
            self.rx_buffer.drain(0..sync_pos);
        }

        if self.rx_buffer.len() < 4{
            return None;
        }

        let msg_type_byte = self.rx_buffer[1];
        let len = self.rx_buffer[2] as usize;

        if len > MAX_MSG_SIZE{
            self.rx_buffer.remove(0);
            return None;
        }

        let frame_len = 4 + len; //sync + type + len + payload + checksum

        if self.rx_buffer.len() < frame_len{
            return None;
        }

        //verify checksum
        let checksum = self.rx_buffer[3 + len];
        let calculated = self.calculate_checksum(&self.rx_buffer[1..3 + len]);

        if checksum != calculated{
            self.rx_buffer.remove(0);
            return None;
        }

        let msg_type = MsgType::from_u8(msg_type_byte)?;
        let payload = self.rx_buffer[3..3 + len].to_vec();

        self.rx_buffer.drain(0..frame_len);

        Some(UartFrame{ msg_type, payload })
    }

    fn calculate_checksum(&self, data: &[u8]) -> u8{
        data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b))
    }

    fn publish_frame(&self, frame: &UartFrame){
        let topic_name = frame.msg_type.to_topic_name();
        let topic = self.registry.get_or_create_byte(topic_name, 32);
        topic.publish(&frame.payload);
    }

    pub fn send_frame(&mut self, msg_type: MsgType, payload: &[u8]) -> std::io::Result<()>{
        if payload.len() > MAX_MSG_SIZE{
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Payload too large"
            ));
        }

        let mut frame = Vec::with_capacity(4 + payload.len());
        frame.push(SYNC_BYTE);
        frame.push(msg_type as u8);
        frame.push(payload.len() as u8);
        frame.extend_from_slice(payload);

        let checksum = self.calculate_checksum(&frame[1..]);
        frame.push(checksum);

        self.port.write_all(&frame)?;
        self.port.flush()?;

        Ok(())
    }
}

pub fn stop_bridge(running: &Arc<AtomicBool>){
    running.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests{
    use super::*;

    #[test]
    fn test_msg_type_conversion(){
        assert_eq!(MsgType::from_u8(0x01), Some(MsgType::Imu));
        assert_eq!(MsgType::from_u8(0x02), Some(MsgType::Depth));
        assert_eq!(MsgType::from_u8(0xFF), None);
    }

    #[test]
    fn test_topic_names(){
        assert_eq!(MsgType::Imu.to_topic_name(), "/stm32/imu");
        assert_eq!(MsgType::Depth.to_topic_name(), "/stm32/depth");
    }

    #[test]
    fn test_checksum(){
        let bridge = create_mock_bridge();
        let data = [0x01, 0x05, 0xAB, 0xCD];
        let checksum = bridge.calculate_checksum(&data);
        assert_eq!(checksum, 0x01u8.wrapping_add(0x05).wrapping_add(0xAB).wrapping_add(0xCD));
    }

    fn create_mock_bridge() -> MockBridge{
        MockBridge{ rx_buffer: Vec::new() }
    }

    struct MockBridge{
        rx_buffer: Vec<u8>,
    }

    impl MockBridge{
        fn calculate_checksum(&self, data: &[u8]) -> u8{
            data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b))
        }
    }
}