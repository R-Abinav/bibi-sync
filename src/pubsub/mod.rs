pub mod message;
pub mod topic;
pub mod publisher;
pub mod subscriber;
pub mod registry;

pub use message::Message;
pub use topic::{Topic, ByteTopic};
pub use publisher::{Publisher, BytePublisher};
pub use subscriber::{Subscriber, ByteSubscriber};
pub use registry::TopicRegistry;

#[cfg(test)]
mod tests{
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_pubsub_threaded(){
        let topic = Arc::new(Topic::<i32>::new("/threaded", 2048));

        let pub_topic = Arc::clone(&topic);
        let sub_topic = Arc::clone(&topic);

        let done = Arc::new(AtomicBool::new(false));
        let done_flag = Arc::clone(&done);
        let done_check = Arc::clone(&done);

        let num_items = 1000;

        let producer = thread::spawn(move ||{
            for i in 0..num_items{
                pub_topic.publish(i);
            }
            done_flag.store(true, Ordering::SeqCst);
        });

        let consumer = thread::spawn(move ||{
            let mut received = Vec::new();
            loop{
                match sub_topic.try_receive(){
                    Some(val) => received.push(val),
                    None =>{
                        if done_check.load(Ordering::SeqCst){
                            while let Some(val) = sub_topic.try_receive(){
                                received.push(val);
                            }
                            break;
                        }
                        std::hint::spin_loop();
                    }
                }
            }
            received
        });

        producer.join().unwrap();
        let received = consumer.join().unwrap();

        assert_eq!(received.len(), num_items as usize);
        for i in 1..received.len(){
            assert!(received[i] > received[i - 1]);
        }
    }
}