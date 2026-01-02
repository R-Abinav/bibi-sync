use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};

//fixed slot size (must accm. largest message + header)
pub const SLOT_SIZE: usize = 256; //256 bytes per slot (adjust as needed)
pub const HEADER_SIZE: usize = 12; //4(len) + 8(epoch)
pub const MAX_PAYLOAD_SIZE: usize = SLOT_SIZE - HEADER_SIZE; 

//a slot with inline length header
#[repr(C)]
pub struct ByteSlot{
    len: u32,  //payload len
    epoch: u64,
    data: [u8; MAX_PAYLOAD_SIZE],
}

impl Default for ByteSlot{
    fn default() -> Self{
        ByteSlot{
            len: 0,
            epoch: 0,
            data: [0u8; MAX_PAYLOAD_SIZE],
        }
    }
}

impl Clone for ByteSlot{
    fn clone(&self) -> Self{
        let mut data = [0u8; MAX_PAYLOAD_SIZE];
        data.copy_from_slice(&self.data);
        ByteSlot{
            len: self.len,
            epoch: self.epoch,
            data,
        }
    }
}

//spsc lock free ring buffer for variable len byte payloads (cam loads)
pub struct ByteRingBuffer{
    buffer: Vec<ByteSlot>,
    head: AtomicUsize,
    tail: AtomicUsize,
    write_epoch: AtomicU64,
    read_epoch: AtomicU64,
    capacity: usize,
}

impl ByteRingBuffer{
    //create new byte-ring-buffer with given slot count
    pub fn new(capacity: usize) -> Self{
        assert!(capacity > 0, "Capacity shud be greater than 0 bro!");

        let mut buffer = Vec::with_capacity(capacity);
        for _ in 0..capacity{
            buffer.push(ByteSlot::default());
        }

        return ByteRingBuffer{
            buffer,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            write_epoch: AtomicU64::new(0),
            read_epoch: AtomicU64::new(0),
            capacity,
        };
    }

    fn is_empty_internal(&self, head: usize, tail: usize) -> bool{
        if head != tail{
            return false;
        }

        return self.buffer[head].epoch <= self.read_epoch.load(Ordering::Acquire);
    }

    fn is_full_internal(&self, head: usize, tail: usize) -> bool{
        if head != tail{
            return false;
        }

        return self.buffer[head].epoch > self.read_epoch.load(Ordering::Acquire);
    }

    //push var len bytes. return epoch, none if too large
    pub fn push(&mut self, data: &[u8]) -> Option<u64>{
        if data.len() > MAX_PAYLOAD_SIZE{
            return None;
        }

        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);

        //overflow cond-> discard oldest
        if self.is_full_internal(head, tail){
            let slot_epoch = self.buffer[tail].epoch;
            self.read_epoch.store(slot_epoch, Ordering::Release);
            let new_tail = (tail + 1) % self.capacity;
            self.tail.store(new_tail, Ordering::Release);
        }

        //inc epoch
        let new_epoch = self.write_epoch.load(Ordering::Relaxed) + 1;
        self.write_epoch.store(new_epoch, Ordering::Relaxed);

        //write to slot (inline, zero-copy on write side)
        let slot = &mut self.buffer[head];
        slot.len = data.len() as u32;
        slot.data[..data.len()].copy_from_slice(data);
        slot.epoch = new_epoch;

        //advance head
        let new_head = (head + 1) % self.capacity;
        self.head.store(new_head, Ordering::Release);

        return Some(new_epoch);
    }
    
    //pop oldest msg, returns (data_slice, epoch)
    pub fn pop(&mut self) -> Option<(Vec<u8>, u64)>{
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);

        if self.is_empty_internal(head, tail){
            return None;
        }

        let slot = &self.buffer[tail];
        let len = slot.len as usize;

        let epoch = slot.epoch;
        let data = slot.data[..len].to_vec();

        self.read_epoch.store(epoch, Ordering::Release);
        let new_tail = (tail + 1) % self.capacity;
        self.tail.store(new_tail, Ordering::Release);

        return Some((data, epoch));
    } 

    //this function peeks at the latest msg, but returns owned Vec (clones data)
    pub fn peek_latest(&self) -> Option<(Vec<u8>, u64)>{
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        if self.is_empty_internal(head, tail){
            return None;
        }

        let latest_idx = if head == 0{
            self.capacity - 1
        }else{
            head - 1
        };

        let slot = &self.buffer[latest_idx]; //borrow, we not moving
        let len = slot.len as usize;

        return Some((slot.data[..len].to_vec(), slot.epoch));

    }

    //peek latest msg without owning (zero copy read via slice)
    pub fn peek_latest_ref(&self) -> Option<(&[u8], u64)>{
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        if self.is_empty_internal(head, tail){
            return None;
        }

        let latest_idx = if head == 0{
            self.capacity - 1
        }else{
            head - 1
        };

        let slot = &self.buffer[latest_idx];
        let len = slot.len as usize;

        return Some((&slot.data[..len], slot.epoch));
    }

    //peek oldest msg without owning (zero copy read via slice)
    pub fn peek_oldest_ref(&self) -> Option<(&[u8], u64)>{
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        if self.is_empty_internal(head, tail){
            return None;
        }

        let slot = &self.buffer[tail];
        let len = slot.len as usize;

        return Some((&slot.data[..len], slot.epoch));   
    }

    //latest epoch
    pub fn latest_epoch(&self) -> u64{
        return self.write_epoch.load(Ordering::Acquire);
    }

    //len
    pub fn len(&self) -> usize{
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        if head == tail{
            if self.is_empty_internal(head, tail){
                return 0;
            }else{
                return self.capacity;
            }
        }else if head > tail{
            return head - tail;
        }else{
            return self.capacity - tail + head;
        }
    }

    //check emptyness
    pub fn is_empty(&self) -> bool{
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        return self.is_empty_internal(head, tail);
    }

    //check fullness
    pub fn is_full(&self) -> bool{
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        return self.is_full_internal(head, tail);
    }

    //capacity
    pub fn capacity(&self) -> usize{
        return self.capacity;
    }
}

//Tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_length_push_pop() {
        let mut rb = ByteRingBuffer::new(4);

        //push different sized messages
        rb.push(&[1, 2, 3]);              //3 bytes
        rb.push(&[10, 20, 30, 40, 50]);   //5 bytes
        rb.push(&[100]);                  //1 byte

        let (data, _) = rb.pop().unwrap();
        assert_eq!(data, vec![1, 2, 3]);

        let (data, _) = rb.pop().unwrap();
        assert_eq!(data, vec![10, 20, 30, 40, 50]);

        let (data, _) = rb.pop().unwrap();
        assert_eq!(data, vec![100]);
    }

    #[test]
    fn test_imu_sized_message() {
        let mut rb = ByteRingBuffer::new(8);

        //56-byte Sensor message from my sensor
        let imu_data: Vec<u8> = (0..56).collect();
        rb.push(&imu_data);

        let (data, epoch) = rb.pop().unwrap();
        assert_eq!(data.len(), 56);
        assert_eq!(epoch, 1);
    }

    #[test]
    fn test_max_payload() {
        let mut rb = ByteRingBuffer::new(4);

        //push max size
        let max_data = vec![0xAB; MAX_PAYLOAD_SIZE];
        assert!(rb.push(&max_data).is_some());

        //push too large -> returns None
        let too_large = vec![0xCD; MAX_PAYLOAD_SIZE + 1];
        assert!(rb.push(&too_large).is_none());
    }

    #[test]
    fn test_zero_copy_peek() {
        let mut rb = ByteRingBuffer::new(4);

        rb.push(&[1, 2, 3, 4, 5]);
        rb.push(&[10, 20, 30]);

        //peek returns slice, not Vec (zero-copy!)
        let (slice, epoch) = rb.peek_latest_ref().unwrap();
        assert_eq!(slice, &[10, 20, 30]);
        assert_eq!(epoch, 2);

        //buffer still has 2 items
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn test_overflow_variable_length() {
        let mut rb = ByteRingBuffer::new(3);

        rb.push(&[1, 1, 1]);
        rb.push(&[2, 2]);
        rb.push(&[3, 3, 3, 3]);
        
        //full, now overflow
        rb.push(&[4]);  //discards [1,1,1]

        let (data, _) = rb.pop().unwrap();
        assert_eq!(data, vec![2, 2]);  //first was discarded
    }

    #[test]
    fn test_peek_latest_owned() {
        let mut rb = ByteRingBuffer::new(4);

        rb.push(&[1, 2, 3]);
        rb.push(&[10, 20, 30, 40]);

        //owned peek (clones data)
        let (data, epoch) = rb.peek_latest().unwrap();
        assert_eq!(data, vec![10, 20, 30, 40]);
        assert_eq!(epoch, 2);

        //buffer still has 2 items
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn test_peek_oldest_ref() {
        let mut rb = ByteRingBuffer::new(4);

        rb.push(&[1, 2, 3]);
        rb.push(&[10, 20]);
        rb.push(&[100]);

        //oldest is first pushed
        let (slice, epoch) = rb.peek_oldest_ref().unwrap();
        assert_eq!(slice, &[1, 2, 3]);
        assert_eq!(epoch, 1);

        //still 3 items
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn test_peek_methods_empty_buffer() {
        let rb = ByteRingBuffer::new(4);

        assert!(rb.peek_latest().is_none());
        assert!(rb.peek_latest_ref().is_none());
        assert!(rb.peek_oldest_ref().is_none());
    }
}