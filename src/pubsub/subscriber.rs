use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use super::topic::{Topic, ByteTopic};
use super::message::Message;

pub struct Subscriber<T: Message>{
    topic: Arc<Topic<T>>,
    last_seen_epoch: AtomicU64,
}

impl<T: Message> Subscriber<T>{
    pub fn new(topic: Arc<Topic<T>>) -> Self{
        Subscriber{
            topic,
            last_seen_epoch: AtomicU64::new(0),
        }
    }

    pub fn try_recv(&self) -> Option<T>{
        self.topic.try_receive()
    }

    pub fn peek_latest(&self) -> Option<(T, u64)>{
        self.topic.peek_latest()
    }

    pub fn has_new(&self) -> bool{
        let current = self.topic.latest_epoch();
        let last = self.last_seen_epoch.load(Ordering::SeqCst);
        current > last
    }

    pub fn mark_seen(&self){
        let current = self.topic.latest_epoch();
        self.last_seen_epoch.store(current, Ordering::SeqCst);
    }

    pub fn topic_name(&self) -> &str{
        self.topic.name()
    }
}

pub struct ByteSubscriber{
    topic: Arc<ByteTopic>,
    last_seen_epoch: AtomicU64,
}

impl ByteSubscriber{
    pub fn new(topic: Arc<ByteTopic>) -> Self{
        ByteSubscriber{
            topic,
            last_seen_epoch: AtomicU64::new(0),
        }
    }

    pub fn try_recv(&self) -> Option<(Vec<u8>, u64)>{
        self.topic.try_receive()
    }

    pub fn peek_latest(&self) -> Option<(Vec<u8>, u64)>{
        self.topic.peek_latest()
    }

    pub fn peek_latest_ref(&self) -> Option<(&[u8], u64)>{
        self.topic.peek_latest_ref()
    }

    pub fn has_new(&self) -> bool{
        let current = self.topic.latest_epoch();
        let last = self.last_seen_epoch.load(Ordering::SeqCst);
        current > last
    }

    pub fn mark_seen(&self){
        let current = self.topic.latest_epoch();
        self.last_seen_epoch.store(current, Ordering::SeqCst);
    }

    pub fn topic_name(&self) -> &str{
        self.topic.name()
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    
    #[test]
    fn test_subscriber_try_recv(){
        let topic = Arc::new(Topic::<i32>::new("/test", 8));
        let subscriber = Subscriber::new(Arc::clone(&topic));

        topic.publish(10);
        topic.publish(20);

        assert_eq!(subscriber.try_recv(), Some(10));
        assert_eq!(subscriber.try_recv(), Some(20));
        assert_eq!(subscriber.try_recv(), None);
    }

    #[test]
    fn test_subscriber_has_new(){
        let topic = Arc::new(Topic::<i32>::new("/test", 8));
        let subscriber = Subscriber::new(Arc::clone(&topic));
        
        assert!(!subscriber.has_new());

        topic.publish(10);
        assert!(subscriber.has_new());

        subscriber.mark_seen();
        assert!(!subscriber.has_new());

        topic.publish(20);
        assert!(subscriber.has_new());
    }

    #[test]
    fn test_subscriber_peek_latest(){
        let topic = Arc::new(Topic::<i32>::new("/test", 8));
        let subscriber = Subscriber::new(Arc::clone(&topic));

        topic.publish(10);
        topic.publish(20);
        topic.publish(30);

        let (val, epoch) = subscriber.peek_latest().unwrap();
        assert_eq!(val, 30);
        assert_eq!(epoch, 3);

        //peek doesn't consume
        assert_eq!(topic.len(), 3);
    }
}