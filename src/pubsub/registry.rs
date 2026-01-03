use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use std::any::Any;
use super::topic::{Topic, ByteTopic};
use super::message::Message;

pub struct TopicRegistry{
    typed_topics: RwLock<HashMap<String, Arc<dyn Any + Send + Sync>>>,
    byte_topics: RwLock<HashMap<String, Arc<ByteTopic>>>,
}

impl TopicRegistry{
    pub fn new() -> Self{
        TopicRegistry{
            typed_topics: RwLock::new(HashMap::new()),
            byte_topics: RwLock::new(HashMap::new()),
        }
    }

    pub fn get_or_create<T: Message>(&self, name: &str, capacity: usize) -> Arc<Topic<T>>{
        let mut topics = self.typed_topics.write().unwrap();
        if let Some(existing) = topics.get(name){
            if let Some(topic) = existing.clone().downcast::<Topic<T>>().ok(){
                return topic;
            }
        }
        let topic = Arc::new(Topic::<T>::new(name, capacity));
        topics.insert(name.to_string(), topic.clone() as Arc<dyn Any + Send + Sync>);
        topic
    }

    pub fn get_or_create_byte(&self, name: &str, capacity: usize) -> Arc<ByteTopic>{
        let mut topics = self.byte_topics.write().unwrap();
        if let Some(existing) = topics.get(name){
            return Arc::clone(existing);
        }
        let topic = Arc::new(ByteTopic::new(name, capacity));
        topics.insert(name.to_string(), Arc::clone(&topic));
        topic
    }

    pub fn topic_count(&self) -> usize{
        let typed = self.typed_topics.read().unwrap().len();
        let bytes = self.byte_topics.read().unwrap().len();
        typed + bytes
    }
}

impl Default for TopicRegistry{
    fn default() -> Self{
        Self::new()
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    
    #[test]
    fn test_registry_get_or_create(){
        let registry = TopicRegistry::new();
        let topic1: Arc<Topic<i32>> = registry.get_or_create("/sensor/temp", 8);
        let topic2: Arc<Topic<f64>> = registry.get_or_create("/sensor/humidity", 16);
        assert_eq!(topic1.name(), "/sensor/temp");
        assert_eq!(topic2.name(), "/sensor/humidity");
        assert_eq!(registry.topic_count(), 2);
    }

    #[test]
    fn test_registry_same_topic_returns_same(){
        let registry = TopicRegistry::new();
        let topic1: Arc<Topic<i32>> = registry.get_or_create("/imu", 8);
        topic1.publish(42);
        let topic2: Arc<Topic<i32>> = registry.get_or_create("/imu", 8);
        let val = topic2.try_receive().unwrap();
        assert_eq!(val, 42);
        assert_eq!(registry.topic_count(), 1);
    }

    #[test]
    fn test_registry_byte_topics(){
        let registry = TopicRegistry::new();
        let topic1 = registry.get_or_create_byte("/camera/0", 32);
        topic1.publish(&[1, 2, 3]);
        let topic2 = registry.get_or_create_byte("/camera/0", 32);
        let (data, _) = topic2.try_receive().unwrap();
        assert_eq!(data, vec![1, 2, 3]);
    }
}
