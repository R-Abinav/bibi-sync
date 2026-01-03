use std::sync::Arc;
use super::topic::{Topic, ByteTopic};
use super::message::Message;

pub struct Publisher<T: Message>{
    topic: Arc<Topic<T>>,
}

impl<T: Message> Publisher<T>{
    pub fn new(topic: Arc<Topic<T>>) -> Self{
        Publisher{ topic }
    }

    pub fn publish(&self, msg: T) -> u64{
        self.topic.publish(msg)
    }

    pub fn topic_name(&self) -> &str{
        self.topic.name()
    }
}

impl<T: Message> Clone for Publisher<T>{
    fn clone(&self) -> Self{
        Publisher{ topic: Arc::clone(&self.topic) }
    }
}

pub struct BytePublisher{
    topic: Arc<ByteTopic>,
}

impl BytePublisher{
    pub fn new(topic: Arc<ByteTopic>) -> Self{
        BytePublisher{ topic }
    }

    pub fn publish(&self, data: &[u8]) -> Option<u64>{
        self.topic.publish(data)
    }

    pub fn topic_name(&self) -> &str{
        self.topic.name()
    }
}

impl Clone for BytePublisher{
    fn clone(&self) -> Self{
        BytePublisher{ topic: Arc::clone(&self.topic) }
    }
}

#[cfg(test)]
mod tests{
    use super::*;

    #[test]
    fn test_publisher_publish(){
        let topic = Arc::new(Topic::<i32>::new("/test", 8));
        let publisher = Publisher::new(Arc::clone(&topic));
        let e1 = publisher.publish(10);
        let e2 = publisher.publish(20);
        assert_eq!(e1, 1);
        assert_eq!(e2, 2);
        assert_eq!(publisher.topic_name(), "/test");
        assert_eq!(topic.len(), 2);
    }
    
    #[test]
    fn test_byte_publisher(){
        let topic = Arc::new(ByteTopic::new("/bytes", 8));
        let publisher = BytePublisher::new(Arc::clone(&topic));
        let e1 = publisher.publish(&[1, 2, 3]).unwrap();
        assert_eq!(e1, 1);
        assert_eq!(topic.len(), 1);
    }
}


