use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};

pub const SLOT_SIZE: usize = 256;
pub const HEADER_SIZE: usize = 12;
pub const MAX_PAYLOAD_SIZE: usize = SLOT_SIZE - HEADER_SIZE;

#[repr(C)]
struct ByteSlotInner{
    len: u32,
    epoch: AtomicU64,
    data: [u8; MAX_PAYLOAD_SIZE],
}

pub struct ByteSlot{
    inner: UnsafeCell<ByteSlotInner>,
}

impl ByteSlot{
    fn new() -> Self{
        ByteSlot{
            inner: UnsafeCell::new(ByteSlotInner{
                len: 0,
                epoch: AtomicU64::new(0),
                data: [0u8; MAX_PAYLOAD_SIZE],
            }),
        }
    }
}

pub struct ByteRingBuffer{
    buffer: Vec<ByteSlot>,
    head: AtomicUsize,
    tail: AtomicUsize,
    write_epoch: AtomicU64,
    read_epoch: AtomicU64,
    capacity: usize,
}

unsafe impl Send for ByteRingBuffer{}
unsafe impl Sync for ByteRingBuffer{}

impl ByteRingBuffer{
    pub fn new(capacity: usize) -> Self{
        assert!(capacity > 0, "Capacity must be greater than 0 bruddaa!!");

        let mut buffer = Vec::with_capacity(capacity);
        for _ in 0..capacity{
            buffer.push(ByteSlot::new());
        }

        ByteRingBuffer{
            buffer,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            write_epoch: AtomicU64::new(0),
            read_epoch: AtomicU64::new(0),
            capacity,
        }
    }

    #[inline]
    unsafe fn slot_inner(&self, index: usize) -> &mut ByteSlotInner{
        unsafe{ &mut *self.buffer[index].inner.get() }
    }

    #[inline]
    fn slot_epoch(&self, index: usize) -> u64{
        unsafe{ (*self.buffer[index].inner.get()).epoch.load(Ordering::SeqCst) }
    }

    pub fn push(&self, data: &[u8]) -> Option<u64>{
        if data.len() > MAX_PAYLOAD_SIZE{
            return None;
        }

        let head = self.head.load(Ordering::Relaxed);

        let new_epoch = self.write_epoch.load(Ordering::Relaxed) + 1;
        self.write_epoch.store(new_epoch, Ordering::Relaxed);

        unsafe{
            let slot = self.slot_inner(head);
            slot.len = data.len() as u32;
            slot.data[..data.len()].copy_from_slice(data);
            slot.epoch.store(new_epoch, Ordering::SeqCst);
        }

        let new_head = (head + 1) % self.capacity;
        self.head.store(new_head, Ordering::SeqCst);

        Some(new_epoch)
    }

    pub fn pop(&self) -> Option<(Vec<u8>, u64)>{
        loop{
            let tail = self.tail.load(Ordering::SeqCst);
            let head = self.head.load(Ordering::SeqCst);
            let read_epoch = self.read_epoch.load(Ordering::SeqCst);
            let write_epoch = self.write_epoch.load(Ordering::SeqCst);

            if write_epoch == 0{
                return None;
            }

            let slot_epoch = self.slot_epoch(tail);

            //already consumed this slot?
            if slot_epoch <= read_epoch{
                if tail == head{
                    return None;
                }
                let new_tail = (tail + 1) % self.capacity;
                self.tail.store(new_tail, Ordering::SeqCst);
                continue;
            }

            //check if slot was overwritten
            let min_valid_epoch = write_epoch.saturating_sub(self.capacity as u64 - 1);
            if slot_epoch < min_valid_epoch{
                self.read_epoch.store(slot_epoch, Ordering::SeqCst);
                let new_tail = (tail + 1) % self.capacity;
                self.tail.store(new_tail, Ordering::SeqCst);
                continue;
            }

            //valid slot - read data
            let (data, epoch) = unsafe{
                let slot = &*self.buffer[tail].inner.get();
                let len = slot.len as usize;
                (slot.data[..len].to_vec(), slot.epoch.load(Ordering::SeqCst))
            };

            self.read_epoch.store(epoch, Ordering::SeqCst);

            let new_tail = (tail + 1) % self.capacity;
            self.tail.store(new_tail, Ordering::SeqCst);

            return Some((data, epoch));
        }
    }

    pub fn peek_latest(&self) -> Option<(Vec<u8>, u64)>{
        let write_epoch = self.write_epoch.load(Ordering::SeqCst);
        if write_epoch == 0{
            return None;
        }

        let head = self.head.load(Ordering::SeqCst);
        let latest_idx = if head == 0{ self.capacity - 1 }else{ head - 1 };

        unsafe{
            let slot = &*self.buffer[latest_idx].inner.get();
            let len = slot.len as usize;
            let epoch = slot.epoch.load(Ordering::SeqCst);
            Some((slot.data[..len].to_vec(), epoch))
        }
    }

    pub fn peek_latest_ref(&self) -> Option<(&[u8], u64)>{
        let write_epoch = self.write_epoch.load(Ordering::SeqCst);
        if write_epoch == 0{
            return None;
        }

        let head = self.head.load(Ordering::SeqCst);
        let latest_idx = if head == 0{ self.capacity - 1 }else{ head - 1 };

        unsafe{
            let slot = &*self.buffer[latest_idx].inner.get();
            let len = slot.len as usize;
            let epoch = slot.epoch.load(Ordering::SeqCst);
            Some((&slot.data[..len], epoch))
        }
    }

    pub fn peek_oldest_ref(&self) -> Option<(&[u8], u64)>{
        let write_epoch = self.write_epoch.load(Ordering::SeqCst);
        if write_epoch == 0{
            return None;
        }

        let tail = self.tail.load(Ordering::SeqCst);
        let read_epoch = self.read_epoch.load(Ordering::SeqCst);
        let slot_epoch = self.slot_epoch(tail);

        if slot_epoch <= read_epoch{
            return None;
        }

        unsafe{
            let slot = &*self.buffer[tail].inner.get();
            let len = slot.len as usize;
            let epoch = slot.epoch.load(Ordering::SeqCst);
            Some((&slot.data[..len], epoch))
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
    fn test_variable_length_push_pop(){
        let rb = ByteRingBuffer::new(4);
        rb.push(&[1, 2, 3]);
        rb.push(&[10, 20, 30, 40, 50]);
        rb.push(&[100]);

        let (data, _) = rb.pop().unwrap();
        assert_eq!(data, vec![1, 2, 3]);

        let (data, _) = rb.pop().unwrap();
        assert_eq!(data, vec![10, 20, 30, 40, 50]);

        let (data, _) = rb.pop().unwrap();
        assert_eq!(data, vec![100]);
    }

    #[test]
    fn test_imu_sized_message(){
        let rb = ByteRingBuffer::new(8);
        let imu_data: Vec<u8> = (0..56).collect();
        rb.push(&imu_data);

        let (data, epoch) = rb.pop().unwrap();
        assert_eq!(data.len(), 56);
        assert_eq!(epoch, 1);
    }

    #[test]
    fn test_max_payload(){
        let rb = ByteRingBuffer::new(4);
        let max_data = vec![0xAB; MAX_PAYLOAD_SIZE];
        assert!(rb.push(&max_data).is_some());

        let too_large = vec![0xCD; MAX_PAYLOAD_SIZE + 1];
        assert!(rb.push(&too_large).is_none());
    }

    #[test]
    fn test_zero_copy_peek(){
        let rb = ByteRingBuffer::new(4);
        rb.push(&[1, 2, 3, 4, 5]);
        rb.push(&[10, 20, 30]);

        let (slice, epoch) = rb.peek_latest_ref().unwrap();
        assert_eq!(slice, &[10, 20, 30]);
        assert_eq!(epoch, 2);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn test_overflow_variable_length(){
        let rb = ByteRingBuffer::new(3);
        rb.push(&[1, 1, 1]);
        rb.push(&[2, 2]);
        rb.push(&[3, 3, 3, 3]);
        rb.push(&[4]);
        rb.push(&[5]);

        let mut values = vec![];
        while let Some((data, _)) = rb.pop(){
            values.push(data);
        }
        assert_eq!(values, vec![vec![4], vec![5]]);
    }

    #[test]
    fn test_peek_latest_owned(){
        let rb = ByteRingBuffer::new(4);
        rb.push(&[1, 2, 3]);
        rb.push(&[10, 20, 30, 40]);

        let (data, epoch) = rb.peek_latest().unwrap();
        assert_eq!(data, vec![10, 20, 30, 40]);
        assert_eq!(epoch, 2);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn test_peek_oldest_ref(){
        let rb = ByteRingBuffer::new(4);
        rb.push(&[1, 2, 3]);
        rb.push(&[10, 20]);
        rb.push(&[100]);

        let (slice, epoch) = rb.peek_oldest_ref().unwrap();
        assert_eq!(slice, &[1, 2, 3]);
        assert_eq!(epoch, 1);
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn test_peek_methods_empty_buffer(){
        let rb = ByteRingBuffer::new(4);
        assert!(rb.peek_latest().is_none());
        assert!(rb.peek_latest_ref().is_none());
        assert!(rb.peek_oldest_ref().is_none());
    }

    #[test]
    fn test_spsc_threaded_var_len(){
        use std::sync::atomic::AtomicBool;

        let rb = Arc::new(ByteRingBuffer::new(2048));
        let done = Arc::new(AtomicBool::new(false));

        let rb_producer = Arc::clone(&rb);
        let done_flag = Arc::clone(&done);

        let rb_consumer = Arc::clone(&rb);
        let done_check = Arc::clone(&done);

        let num_items: u32 = 1000;

        let producer = thread::spawn(move ||{
            for i in 0..num_items{
                let bytes = i.to_le_bytes();
                rb_producer.push(&bytes);
            }
            done_flag.store(true, Ordering::SeqCst);
        });

        let consumer = thread::spawn(move ||{
            let mut received = Vec::new();
            loop{
                match rb_consumer.pop(){
                    Some((data, _)) =>{
                        let val = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                        received.push(val);
                    }
                    None =>{
                        if done_check.load(Ordering::SeqCst){
                            while let Some((data, _)) = rb_consumer.pop(){
                                let val = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
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
            assert_eq!(val, i as u32);
        }
    }
}