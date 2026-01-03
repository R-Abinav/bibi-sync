pub mod byte_buffer;

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};

struct SlotInner<T>{
    data: T,
    epoch: AtomicU64,
}

pub struct Slot<T>{
    inner: UnsafeCell<SlotInner<T>>,
}

impl<T: Default> Slot<T>{
    fn default() -> Self{
        Slot{
            inner: UnsafeCell::new(SlotInner{
                data: T::default(),
                epoch: AtomicU64::new(0),
            }),
        }
    }
}

pub struct RingBuffer<T>{
    buffer: Vec<Slot<T>>,
    head: AtomicUsize,
    tail: AtomicUsize,
    write_epoch: AtomicU64,
    read_epoch: AtomicU64,  //last epoch consumed by reader
    capacity: usize,
}

unsafe impl<T: Send> Send for RingBuffer<T>{}
unsafe impl<T: Send> Sync for RingBuffer<T>{}

impl<T: Clone + Default> RingBuffer<T>{
    pub fn new(capacity: usize) -> Self{
        assert!(capacity > 0, "Capacity must be greater than 0 bruddaa!!");

        let mut buffer = Vec::with_capacity(capacity);
        for _ in 0..capacity{
            buffer.push(Slot::default());
        }

        RingBuffer{
            buffer,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            write_epoch: AtomicU64::new(0),
            read_epoch: AtomicU64::new(0),
            capacity,
        }
    }

    #[inline]
    unsafe fn slot_inner(&self, index: usize) -> &mut SlotInner<T>{
        unsafe{ &mut *self.buffer[index].inner.get() }
    }

    #[inline]
    fn slot_epoch(&self, index: usize) -> u64{
        unsafe{ (*self.buffer[index].inner.get()).epoch.load(Ordering::SeqCst) }
    }

    pub fn push(&self, item: T) -> u64{
        let head = self.head.load(Ordering::Relaxed);

        let new_epoch = self.write_epoch.load(Ordering::Relaxed) + 1;
        self.write_epoch.store(new_epoch, Ordering::Relaxed);

        unsafe{
            let slot = self.slot_inner(head);
            slot.data = item;
            slot.epoch.store(new_epoch, Ordering::SeqCst);
        }

        let new_head = (head + 1) % self.capacity;
        self.head.store(new_head, Ordering::SeqCst);

        new_epoch
    }

    pub fn pop(&self) -> Option<T>{
        loop{
            let tail = self.tail.load(Ordering::SeqCst);
            let head = self.head.load(Ordering::SeqCst);
            let read_epoch = self.read_epoch.load(Ordering::SeqCst);
            let write_epoch = self.write_epoch.load(Ordering::SeqCst);

            //empty check: nothing written yet
            if write_epoch == 0{
                return None;
            }

            let slot_epoch = self.slot_epoch(tail);

            //already consumed this slot?
            if slot_epoch <= read_epoch{
                //check if there's newer data ahead
                if tail == head{
                    return None; //truly empty - caught up
                }
                //advance tail to find unread slot
                let new_tail = (tail + 1) % self.capacity;
                self.tail.store(new_tail, Ordering::SeqCst);
                continue;
            }

            //check if slot was overwritten (producer lapped us)
            let min_valid_epoch = write_epoch.saturating_sub(self.capacity as u64 - 1);
            if slot_epoch < min_valid_epoch{
                //slot overwritten, skip it
                self.read_epoch.store(slot_epoch, Ordering::SeqCst);
                let new_tail = (tail + 1) % self.capacity;
                self.tail.store(new_tail, Ordering::SeqCst);
                continue;
            }

            //valid slot - read data
            let item = unsafe{
                let slot = &*self.buffer[tail].inner.get();
                slot.data.clone()
            };

            //mark as consumed
            self.read_epoch.store(slot_epoch, Ordering::SeqCst);

            //advance tail
            let new_tail = (tail + 1) % self.capacity;
            self.tail.store(new_tail, Ordering::SeqCst);

            return Some(item);
        }
    }

    pub fn peek_latest(&self) -> Option<(T, u64)>{
        let write_epoch = self.write_epoch.load(Ordering::SeqCst);
        if write_epoch == 0{
            return None;
        }

        let head = self.head.load(Ordering::SeqCst);
        let latest_idx = if head == 0{ self.capacity - 1 }else{ head - 1 };

        unsafe{
            let slot = &*self.buffer[latest_idx].inner.get();
            let epoch = slot.epoch.load(Ordering::SeqCst);
            Some((slot.data.clone(), epoch))
        }
    }

    pub fn peek_latest_ref(&self) -> Option<(&T, u64)>{
        let write_epoch = self.write_epoch.load(Ordering::SeqCst);
        if write_epoch == 0{
            return None;
        }

        let head = self.head.load(Ordering::SeqCst);
        let latest_idx = if head == 0{ self.capacity - 1 }else{ head - 1 };

        unsafe{
            let slot = &*self.buffer[latest_idx].inner.get();
            let epoch = slot.epoch.load(Ordering::SeqCst);
            Some((&slot.data, epoch))
        }
    }

    pub fn peek_oldest_ref(&self) -> Option<(&T, u64)>{
        let write_epoch = self.write_epoch.load(Ordering::SeqCst);
        if write_epoch == 0{
            return None;
        }

        let tail = self.tail.load(Ordering::SeqCst);
        let read_epoch = self.read_epoch.load(Ordering::SeqCst);
        let slot_epoch = self.slot_epoch(tail);

        if slot_epoch <= read_epoch{
            return None; //already consumed
        }

        unsafe{
            let slot = &*self.buffer[tail].inner.get();
            let epoch = slot.epoch.load(Ordering::SeqCst);
            Some((&slot.data, epoch))
        }
    }

    pub fn latest_epoch(&self) -> u64{
        self.write_epoch.load(Ordering::SeqCst)
    }

    pub fn len(&self) -> usize{
        let write_epoch = self.write_epoch.load(Ordering::SeqCst);
        let read_epoch = self.read_epoch.load(Ordering::SeqCst);

        if write_epoch == 0{
            return 0;
        }

        //number of unread items = write_epoch - read_epoch, capped at capacity
        let unread = write_epoch.saturating_sub(read_epoch) as usize;
        std::cmp::min(unread, self.capacity)
    }

    pub fn is_empty(&self) -> bool{
        self.len() == 0
    }

    pub fn is_full(&self) -> bool{
        self.len() == self.capacity
    }

    pub fn capacity(&self) -> usize{
        self.capacity
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_push_pop_fifo(){
        let rb: RingBuffer<i32> = RingBuffer::new(5);
        rb.push(10);
        rb.push(20);
        rb.push(30);
        assert_eq!(rb.pop(), Some(10));
        assert_eq!(rb.pop(), Some(20));
        assert_eq!(rb.pop(), Some(30));
        assert_eq!(rb.pop(), None);
    }

    #[test]
    fn test_wraparound(){
        let rb: RingBuffer<i32> = RingBuffer::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        assert_eq!(rb.pop(), Some(1));
        rb.push(4);
        assert_eq!(rb.pop(), Some(2));
        assert_eq!(rb.pop(), Some(3));
        assert_eq!(rb.pop(), Some(4));
        assert_eq!(rb.pop(), None);
    }

    #[test]
    fn test_epoch_increment(){
        let rb: RingBuffer<i32> = RingBuffer::new(5);
        let e1 = rb.push(10);
        let e2 = rb.push(20);
        assert_eq!(e1, 1);
        assert_eq!(e2, 2);
        assert_eq!(rb.latest_epoch(), 2);
    }

    #[test]
    fn test_overflow_skips_old(){
        let rb: RingBuffer<i32> = RingBuffer::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        let mut values = vec![];
        while let Some(v) = rb.pop(){
            values.push(v);
        }
        assert_eq!(values, vec![4, 5]); //when head wraps to tail, that slot becomes inaccessible
    }

    #[test]
    fn test_full_capacity_usable(){
        let rb: RingBuffer<i32> = RingBuffer::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        assert_eq!(rb.len(), 3);
        assert!(rb.is_full());
    }

    #[test]
    fn test_peek_latest(){
        let rb: RingBuffer<i32> = RingBuffer::new(5);
        rb.push(10);
        rb.push(20);
        rb.push(30);
        let (val, epoch) = rb.peek_latest().unwrap();
        assert_eq!(val, 30);
        assert_eq!(epoch, 3);
    }

    #[test]
    fn test_zero_copy_peek_ref(){
        let rb: RingBuffer<i32> = RingBuffer::new(5);
        rb.push(10);
        rb.push(20);
        rb.push(30);
        let (val_ref, _) = rb.peek_latest_ref().unwrap();
        assert_eq!(*val_ref, 30);
    }

    #[test]
    fn test_spsc_threaded(){
        use std::sync::atomic::AtomicBool;

        let rb = Arc::new(RingBuffer::<i32>::new(2048));
        let done = Arc::new(AtomicBool::new(false));

        let rb_producer = Arc::clone(&rb);
        let done_flag = Arc::clone(&done);

        let rb_consumer = Arc::clone(&rb);
        let done_check = Arc::clone(&done);

        let num_items: i32 = 1000;

        let producer = thread::spawn(move ||{
            for i in 0..num_items{
                rb_producer.push(i);
            }
            done_flag.store(true, Ordering::SeqCst);
        });

        let consumer = thread::spawn(move ||{
            let mut received = Vec::new();
            loop{
                match rb_consumer.pop(){
                    Some(val) => received.push(val),
                    None =>{
                        if done_check.load(Ordering::SeqCst){
                            while let Some(val) = rb_consumer.pop(){
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

        for (i, &val) in received.iter().enumerate(){
            assert_eq!(val, i as i32);
        }
    }
}