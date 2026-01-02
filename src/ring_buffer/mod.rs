use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};

//a slot in the ring buffer containing data and its epoch
pub struct Slot<T>{
    data: T,
    epoch: AtomicU64,  //epoch when this slot was last written
}

impl<T: Default> Slot<T>{
    fn default() -> Self{
        Slot{
            data: T::default(),
            epoch: AtomicU64::new(0),
        }
    }
}

//SPSC lock free ring buffer with per-slot epochs
pub struct RingBuffer<T>{
    buffer: Vec<Slot<T>>,
    head:AtomicUsize,
    tail: AtomicUsize,
    write_epoch: AtomicU64, //writer's current epoch (inc on push)
    read_epoch: AtomicU64, //reader's last seen epoch
    capacity: usize,
}

impl<T: Clone + Default> RingBuffer<T>{
    //creating a new ring buffer with given capacity
    pub fn new(capacity: usize) -> Self{
        assert!(capacity > 0, "Capacity must be greater than 0 bro!");

        //init all slots with epoch 0
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

    //check if buffer is empty usign epoch comparision
    fn is_empty_internal(&self, head: usize, tail: usize) -> bool{
        if head != tail{
            return false;
        }
        //head equals tail
        //check epochs to confirm if empty 
        let slot_epoch = self.buffer[head].epoch.load(Ordering::Acquire);
        let read_epoch = self.read_epoch.load(Ordering::Acquire);

        //empty -> reader has seen the current slot (slot_e <= read_e)
        return slot_epoch <= read_epoch;
    }

    fn is_full_internal(&self, head: usize, tail: usize) -> bool{
        if head != tail{
            return false;
        }

        //head == tail
        //check epochs to confirm if fukk
        let slot_epoch = self.buffer[head].epoch.load(Ordering::Acquire);
        let read_epoch = self.read_epoch.load(Ordering::Acquire);

        //full -> writer is ahead
        return slot_epoch > read_epoch;
    }


    //push item to buffer
    //return the epoch num. of the push
    pub fn push(&mut self, item: T) -> u64{
        let head = self.head.load(Ordering::Relaxed);
         let tail = self.tail.load(Ordering::Acquire);

        //check if full (freshness bias, discard old)
        if self.is_full_internal(head, tail){
            //buffer is full, advance to discard oldest
            let slot_epoch = self.buffer[tail].epoch.load(Ordering::Acquire);
            self.read_epoch.store(slot_epoch, Ordering::Release);
            let new_tail = (tail + 1) % self.capacity;
            self.tail.store(new_tail, Ordering::Release);
        }

        //increment write epoch first
        let new_epoch = self.write_epoch.load(Ordering::Relaxed) + 1;
        self.write_epoch.store(new_epoch, Ordering::Relaxed);

        //write data to slot
        self.buffer[head].data = item;

        //publish epoch after data (mem ord.)
        self.buffer[head].epoch.store(new_epoch, Ordering::Release);

        //increment head
        let next_head = (head + 1) % self.capacity;
        self.head.store(next_head, Ordering::Release);

        return new_epoch;
    }

    //pop fn
    //pop the oldest item from buffer
    pub fn pop(&mut self) -> Option<T>{
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);

        if self.is_empty_internal(tail, head){
            return None; //empty
        }

        //read data from slot
        let item = self.buffer[tail].data.clone();
        let slot_epoch = self.buffer[tail].epoch.load(Ordering::Acquire);

        //update reader's epoch to mark this slot as 'seen'
        self.read_epoch.store(slot_epoch, Ordering::Release);

        //advance tail
        let new_tail = (tail + 1) % self.capacity;
        self.tail.store(new_tail, Ordering::Release);

        return Some(item);
    }

    //peek at latest item without removing (for subscribers!!)
    pub fn peek_latest(&self) -> Option<(T, u64)>{
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        if self.is_empty_internal(head, tail){
            return None;
        }

        //latest item is at (head - 1 + capacity) % capacity
        let latest_idx = if head == 0{
            self.capacity - 1
        }else{
            head - 1
        };

        let epoch = self.buffer[latest_idx].epoch.load(Ordering::Acquire);
        let data = self.buffer[latest_idx].data.clone();

        return Some((data, epoch));
    }

    //get the latest epoch (for freshness detection yo)
    pub fn latest_epoch(&self) -> u64{
        return self.write_epoch.load(Ordering::Acquire);
    }

    //get the current occupancy
    pub fn len(&self) -> usize{
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        if head == tail{
            if self.is_empty_internal(head, tail){
                return 0;
            }else{
                return self.capacity;
            }
        }

        if head > tail{
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

    //return capacity
    pub fn capacity(&self) -> usize{
        return self.capacity;
    }
}

//Tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_pop_fifo() {
        let mut rb: RingBuffer<i32> = RingBuffer::new(5);
        
        rb.push(10);
        rb.push(20);
        rb.push(30);

        assert_eq!(rb.pop(), Some(10));
        assert_eq!(rb.pop(), Some(20));
        assert_eq!(rb.pop(), Some(30));
        assert_eq!(rb.pop(), None);
    }

    #[test]
    fn test_wraparound() {
        let mut rb: RingBuffer<i32> = RingBuffer::new(3);
        
        rb.push(1);
        rb.push(2);
        rb.push(3);  //buffer full: [1, 2, 3], head=0, tail=0, slot epochs > read_epoch
        
        assert_eq!(rb.pop(), Some(1));  //tail -> 1
        
        rb.push(4);  //writes at head=0, head -> 1
        
        assert_eq!(rb.pop(), Some(2));  //tail -> 2
        assert_eq!(rb.pop(), Some(3));  //tail ->0
        assert_eq!(rb.pop(), Some(4));  //tail -> 1
        assert_eq!(rb.pop(), None);
    }

    #[test]
    fn test_epoch_increment() {
        let mut rb: RingBuffer<i32> = RingBuffer::new(5);
        
        let e1 = rb.push(10);
        let e2 = rb.push(20);
        
        assert_eq!(e1, 1);
        assert_eq!(e2, 2);
        assert_eq!(rb.latest_epoch(), 2);
    }

    #[test]
    fn test_overflow_discards_old() {
        let mut rb: RingBuffer<i32> = RingBuffer::new(3);
        
        rb.push(1);
        rb.push(2);
        rb.push(3);  //buffer full: [1, 2, 3]
        
        assert!(rb.is_full());
        assert_eq!(rb.len(), 3);
        
        rb.push(4);  //overflow -> Discard 1, write 4: [4, 2, 3]
        
        assert_eq!(rb.pop(), Some(2));  //1 was discarded
        assert_eq!(rb.pop(), Some(3));
        assert_eq!(rb.pop(), Some(4));
        assert_eq!(rb.pop(), None);
    }

    #[test]
    fn test_full_capacity_usable() {
        let mut rb: RingBuffer<i32> = RingBuffer::new(3);
        
        rb.push(1);
        rb.push(2);
        rb.push(3);
        
        //all 3 slots used!
        assert_eq!(rb.len(), 3);
        assert!(rb.is_full());
        assert!(!rb.is_empty());
    }

    #[test]
    fn test_peek_latest() {
        let mut rb: RingBuffer<i32> = RingBuffer::new(5);
        
        rb.push(10);
        rb.push(20);
        rb.push(30);
        
        //peek doesn't consume
        let (val, epoch) = rb.peek_latest().unwrap();
        assert_eq!(val, 30);
        assert_eq!(epoch, 3);
        
        //still 3 items
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn test_slot_epoch_freshness() {
        let mut rb: RingBuffer<i32> = RingBuffer::new(3);
        
        let e1 = rb.push(100);
        let e2 = rb.push(200);
        
        //each push gets incremented epoch
        assert_eq!(e1, 1);
        assert_eq!(e2, 2);
        
        //pop and check freshness
        rb.pop();  //reads epoch 1
        
        let e3 = rb.push(300);
        assert_eq!(e3, 3);
    }
}