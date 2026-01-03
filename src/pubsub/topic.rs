use std::sync::Arc;
use crate::ring_buffer::RingBuffer;
use crate::ring_buffer::byte_buffer::ByteRingBuffer;
use super::message::Message;

pub struct Topic<T: Message>{
    name: String,
    buffer: Arc<RingBuffer<T>>
}

impl<T: Message> Topic<T>{
    pub fn new(name: &str, capacity: usize) -> Self{
        Topic{
            name: name.to_string(),
            buffer: Arc::new(RingBuffer::new(capacity)),
        }
    }

    pub fn name(&self) -> &str{
        &self.name
    }

    pub fn publish(&self, msg: T) -> u64{
        self.buffer.push(msg)
    }

    pub fn try_receive(&self) -> Option<T>{
        self.buffer.pop()
    }

    pub fn peek_latest(&self) -> Option<(T, u64)>{
        self.buffer.peek_latest()
    }
    
    pub fn peek_latest_ref(&self) -> Option<(&T, u64)>{
        self.buffer.peek_latest_ref()
    }

    pub fn latest_epoch(&self) -> u64{
        self.buffer.latest_epoch()
    }

    pub fn len(&self) -> usize{
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool{
        self.buffer.is_empty()
    }

    pub fn capacity(&self) -> usize{
        self.buffer.capacity()
    }

    pub fn buffer(&self) -> Arc<RingBuffer<T>>{
        Arc::clone(&self.buffer)
    }
}

impl<T: Message> Clone for Topic<T>{
    fn clone(&self) -> Self{
        Topic{
            name: self.name.clone(),
            buffer: Arc::clone(&self.buffer),
        }
    }
}

pub struct ByteTopic{
    name: String,
    buffer: Arc<ByteRingBuffer>,
}

impl ByteTopic{
    pub fn new(name: &str, capacity: usize) -> Self{
        ByteTopic{
            name: name.to_string(),
            buffer: Arc::new(ByteRingBuffer::new(capacity)),
        }
    }

    pub fn name(&self) -> &str{
        &self.name
    }

    pub fn publish(&self, data: &[u8]) -> Option<u64>{
        self.buffer.push(data)
    }

    pub fn try_receive(&self) -> Option<(Vec<u8>, u64)>{
        self.buffer.pop()
    }
    
    pub fn peek_latest(&self) -> Option<(Vec<u8>, u64)>{
        self.buffer.peek_latest()
    }
    
    pub fn peek_latest_ref(&self) -> Option<(&[u8], u64)>{
        self.buffer.peek_latest_ref()
    }
    
    pub fn latest_epoch(&self) -> u64{
        self.buffer.latest_epoch()
    }
    
    pub fn len(&self) -> usize{
        self.buffer.len()
    }
    
    pub fn is_empty(&self) -> bool{
        self.buffer.is_empty()
    }
    
    pub fn capacity(&self) -> usize{
        self.buffer.capacity()
    }
    
    pub fn buffer(&self) -> Arc<ByteRingBuffer>{
        Arc::clone(&self.buffer)
    }
}
impl Clone for ByteTopic{
    fn clone(&self) -> Self{
        ByteTopic{
            name: self.name.clone(),
            buffer: Arc::clone(&self.buffer),
        }
    }
}

#[cfg(test)]
mod tests{
    use super::*;

    #[derive(Clone, Default, Debug, PartialEq)]

    struct ImuData{
        accel_x: f32,
        accel_y: f32,
        accel_z: f32,
    }

    #[test]
    fn test_typed_topic_publish_subscribe(){
        let topic: Topic<ImuData> = Topic::new("/imu/data", 8);
        let msg1 = ImuData{ accel_x: 1.0, accel_y: 2.0, accel_z: 9.8 };
        let msg2 = ImuData{ accel_x: 1.1, accel_y: 2.1, accel_z: 9.9 };
        let e1 = topic.publish(msg1.clone());
        let e2 = topic.publish(msg2.clone());
        assert_eq!(e1, 1);
        assert_eq!(e2, 2);
        assert_eq!(topic.len(), 2);
        assert_eq!(topic.name(), "/imu/data");
        let received1 = topic.try_receive().unwrap();
        assert_eq!(received1, msg1);
        let received2 = topic.try_receive().unwrap();
        assert_eq!(received2, msg2);
        assert!(topic.try_receive().is_none());
    }

    #[test]
    fn test_typed_topic_peek_latest(){
        let topic: Topic<i32> = Topic::new("/test/int", 8);
        topic.publish(10);
        topic.publish(20);
        topic.publish(30);
        let (val, epoch) = topic.peek_latest().unwrap();
        assert_eq!(val, 30);
        assert_eq!(epoch, 3);
        assert_eq!(topic.len(), 3);
    }

    #[test]
    fn test_byte_topic_publish_subscribe(){
        let topic = ByteTopic::new("/camera/raw", 8);
        let frame1 = vec![0xAA, 0xBB, 0xCC];
        let frame2 = vec![0x11, 0x22, 0x33, 0x44];
        let e1 = topic.publish(&frame1).unwrap();
        let e2 = topic.publish(&frame2).unwrap();
        assert_eq!(e1, 1);
        assert_eq!(e2, 2);
        assert_eq!(topic.name(), "/camera/raw");
        let (data1, _) = topic.try_receive().unwrap();
        assert_eq!(data1, frame1);
        let (data2, _) = topic.try_receive().unwrap();
        assert_eq!(data2, frame2);
    }
    
    #[test]
    fn test_topic_clone_shares_buffer(){
        let topic1: Topic<i32> = Topic::new("/shared", 8);
        let topic2 = topic1.clone();
        topic1.publish(100);
        
        let val = topic2.try_receive().unwrap();
        assert_eq!(val, 100);
        assert!(topic1.try_receive().is_none());
    }
}