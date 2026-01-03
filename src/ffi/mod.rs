use std::ffi::{c_char, CStr};
use std::sync::Arc;
use std::ptr;
use crate::pubsub::{TopicRegistry, ByteTopic};

pub struct BibiRegistry{
    inner: TopicRegistry,
}

pub struct BibiByteTopic{
    inner: Arc<ByteTopic>,
}

#[no_mangle]
pub extern "C" fn bibi_registry_new() -> *mut BibiRegistry{
    let registry = Box::new(BibiRegistry{
        inner: TopicRegistry::new(),
    });
    Box::into_raw(registry)
}

#[no_mangle]
pub unsafe extern "C" fn bibi_registry_free(registry: *mut BibiRegistry){
    if !registry.is_null(){
        unsafe{ drop(Box::from_raw(registry)); }
    }
}

#[no_mangle]
pub unsafe extern "C" fn bibi_registry_get_byte_topic(
    registry: *mut BibiRegistry,
    name: *const c_char,
    capacity: usize,
) -> *mut BibiByteTopic{
    if registry.is_null() || name.is_null(){
        return ptr::null_mut();
    }

    unsafe{
        let reg = &mut *registry;
        let name_str = match CStr::from_ptr(name).to_str(){
            Ok(s) => s,
            Err(_) => return ptr::null_mut(),
        };

        let topic = reg.inner.get_or_create_byte(name_str, capacity);
        let handle = Box::new(BibiByteTopic{ inner: topic });
        Box::into_raw(handle)
    }
}

#[no_mangle]
pub unsafe extern "C" fn bibi_byte_topic_free(topic: *mut BibiByteTopic){
    if !topic.is_null(){
        unsafe{ drop(Box::from_raw(topic)); }
    }
}

#[no_mangle]
pub unsafe extern "C" fn bibi_byte_topic_publish(
    topic: *mut BibiByteTopic,
    data: *const u8,
    len: usize,
) -> u64{
    if topic.is_null() || data.is_null(){
        return 0;
    }

    unsafe{
        let t = &*topic;
        let slice = std::slice::from_raw_parts(data, len);
        
        match t.inner.publish(slice){
            Some(epoch) => epoch,
            None => 0,
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn bibi_byte_topic_try_receive(
    topic: *mut BibiByteTopic,
    out_data: *mut u8,
    out_len: *mut usize,
    max_len: usize,
) -> i32{
    if topic.is_null() || out_data.is_null() || out_len.is_null(){
        return -1;
    }

    unsafe{
        let t = &*topic;
        
        match t.inner.try_receive(){
            Some((data, _epoch)) =>{
                if data.len() > max_len{
                    return -2;
                }
                ptr::copy_nonoverlapping(data.as_ptr(), out_data, data.len());
                *out_len = data.len();
                1
            }
            None => 0,
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn bibi_byte_topic_peek_latest(
    topic: *mut BibiByteTopic,
    out_data: *mut u8,
    out_len: *mut usize,
    out_epoch: *mut u64,
    max_len: usize,
) -> i32{
    if topic.is_null() || out_data.is_null() || out_len.is_null(){
        return -1;
    }

    unsafe{
        let t = &*topic;
        
        match t.inner.peek_latest(){
            Some((data, epoch)) =>{
                if data.len() > max_len{
                    return -2;
                }
                ptr::copy_nonoverlapping(data.as_ptr(), out_data, data.len());
                *out_len = data.len();
                if !out_epoch.is_null(){
                    *out_epoch = epoch;
                }
                1
            }
            None => 0,
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn bibi_byte_topic_len(topic: *mut BibiByteTopic) -> usize{
    if topic.is_null(){
        return 0;
    }
    unsafe{
        let t = &*topic;
        t.inner.len()
    }
}

#[no_mangle]
pub unsafe extern "C" fn bibi_byte_topic_is_empty(topic: *mut BibiByteTopic) -> bool{
    if topic.is_null(){
        return true;
    }
    unsafe{
        let t = &*topic;
        t.inner.is_empty()
    }
}

#[no_mangle]
pub unsafe extern "C" fn bibi_byte_topic_latest_epoch(topic: *mut BibiByteTopic) -> u64{
    if topic.is_null(){
        return 0;
    }
    unsafe{
        let t = &*topic;
        t.inner.latest_epoch()
    }
}

pub struct BibiTypedTopic{
    inner: Arc<ByteTopic>,
    msg_size: usize,
}

#[no_mangle]
pub unsafe extern "C" fn bibi_registry_get_typed_topic(
    registry: *mut BibiRegistry,
    name: *const c_char,
    capacity: usize,
    msg_size: usize,
) -> *mut BibiTypedTopic{
    if registry.is_null() || name.is_null(){
        return ptr::null_mut();
    }

    unsafe{
        let reg = &mut *registry;
        let name_str = match CStr::from_ptr(name).to_str(){
            Ok(s) => s,
            Err(_) => return ptr::null_mut(),
        };

        let topic = reg.inner.get_or_create_byte(name_str, capacity);
        let handle = Box::new(BibiTypedTopic{ inner: topic, msg_size });
        Box::into_raw(handle)
    }
}

#[no_mangle]
pub unsafe extern "C" fn bibi_typed_topic_free(topic: *mut BibiTypedTopic){
    if !topic.is_null(){
        unsafe{ drop(Box::from_raw(topic)); }
    }
}

#[no_mangle]
pub unsafe extern "C" fn bibi_typed_topic_publish(
    topic: *mut BibiTypedTopic,
    data: *const u8,
) -> u64{
    if topic.is_null() || data.is_null(){
        return 0;
    }

    unsafe{
        let t = &*topic;
        let slice = std::slice::from_raw_parts(data, t.msg_size);
        
        match t.inner.publish(slice){
            Some(epoch) => epoch,
            None => 0,
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn bibi_typed_topic_try_receive(
    topic: *mut BibiTypedTopic,
    out_data: *mut u8,
) -> i32{
    if topic.is_null() || out_data.is_null(){
        return -1;
    }

    unsafe{
        let t = &*topic;
        
        match t.inner.try_receive(){
            Some((data, _epoch)) =>{
                if data.len() != t.msg_size{
                    return -2;
                }
                ptr::copy_nonoverlapping(data.as_ptr(), out_data, t.msg_size);
                1
            }
            None => 0,
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn bibi_typed_topic_peek_latest(
    topic: *mut BibiTypedTopic,
    out_data: *mut u8,
    out_epoch: *mut u64,
) -> i32{
    if topic.is_null() || out_data.is_null(){
        return -1;
    }

    unsafe{
        let t = &*topic;
        
        match t.inner.peek_latest(){
            Some((data, epoch)) =>{
                if data.len() != t.msg_size{
                    return -2;
                }
                ptr::copy_nonoverlapping(data.as_ptr(), out_data, t.msg_size);
                if !out_epoch.is_null(){
                    *out_epoch = epoch;
                }
                1
            }
            None => 0,
        }
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_ffi_registry_create_free(){
        let registry = bibi_registry_new();
        assert!(!registry.is_null());
        unsafe{ bibi_registry_free(registry); }
    }

    #[test]
    fn test_ffi_byte_topic_publish_receive(){
        let registry = bibi_registry_new();
        let name = CString::new("/test/ffi").unwrap();
        
        unsafe{
            let topic = bibi_registry_get_byte_topic(registry, name.as_ptr(), 8);
            assert!(!topic.is_null());

            let data: [u8; 3] = [1, 2, 3];
            let epoch = bibi_byte_topic_publish(topic, data.as_ptr(), 3);
            assert_eq!(epoch, 1);

            let mut out_data: [u8; 256] = [0; 256];
            let mut out_len: usize = 0;
            let result = bibi_byte_topic_try_receive(
                topic,
                out_data.as_mut_ptr(),
                &mut out_len,
                256,
            );
            
            assert_eq!(result, 1);
            assert_eq!(out_len, 3);
            assert_eq!(&out_data[..3], &[1, 2, 3]);

            bibi_byte_topic_free(topic);
            bibi_registry_free(registry);
        }
    }

    #[test]
    fn test_ffi_typed_topic(){
        #[repr(C)]
        struct ImuMsg{
            accel_x: f32,
            accel_y: f32,
            accel_z: f32,
        }

        let registry = bibi_registry_new();
        let name = CString::new("/imu").unwrap();
        
        unsafe{
            let topic = bibi_registry_get_typed_topic(
                registry,
                name.as_ptr(),
                8,
                std::mem::size_of::<ImuMsg>(),
            );

            let msg = ImuMsg{ accel_x: 1.0, accel_y: 2.0, accel_z: 9.8 };
            let msg_ptr = &msg as *const ImuMsg as *const u8;
            let epoch = bibi_typed_topic_publish(topic, msg_ptr);
            assert_eq!(epoch, 1);

            let mut out_msg = ImuMsg{ accel_x: 0.0, accel_y: 0.0, accel_z: 0.0 };
            let out_ptr = &mut out_msg as *mut ImuMsg as *mut u8;
            let result = bibi_typed_topic_try_receive(topic, out_ptr);
            
            assert_eq!(result, 1);
            assert_eq!(out_msg.accel_x, 1.0);
            assert_eq!(out_msg.accel_y, 2.0);
            assert_eq!(out_msg.accel_z, 9.8);

            bibi_typed_topic_free(topic);
            bibi_registry_free(registry);
        }
    }

    #[test]
    fn test_ffi_shared_topic(){
        let registry = bibi_registry_new();
        let name = CString::new("/shared").unwrap();
        
        unsafe{
            let topic1 = bibi_registry_get_byte_topic(registry, name.as_ptr(), 8);
            let topic2 = bibi_registry_get_byte_topic(registry, name.as_ptr(), 8);

            let data: [u8; 2] = [0xAB, 0xCD];
            bibi_byte_topic_publish(topic1, data.as_ptr(), 2);

            let mut out_data: [u8; 256] = [0; 256];
            let mut out_len: usize = 0;
            let result = bibi_byte_topic_try_receive(topic2, out_data.as_mut_ptr(), &mut out_len, 256);
            
            assert_eq!(result, 1);
            assert_eq!(&out_data[..2], &[0xAB, 0xCD]);

            bibi_byte_topic_free(topic1);
            bibi_byte_topic_free(topic2);
            bibi_registry_free(registry);
        }
    }
}