use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;
use std::sync::Arc;
use crate::pubsub::{TopicRegistry, ByteTopic};

#[pyclass]
pub struct PyBibiRegistry{
    inner: Arc<TopicRegistry>,
}

#[pymethods]
impl PyBibiRegistry{
    #[new]
    fn new() -> Self{
        PyBibiRegistry{
            inner: Arc::new(TopicRegistry::new()),
        }
    }

    fn get_byte_topic(&self, name: &str, capacity: usize) -> PyBibiByteTopic{
        let topic = self.inner.get_or_create_byte(name, capacity);
        PyBibiByteTopic{ inner: topic }
    }

    fn topic_count(&self) -> usize{
        self.inner.topic_count()
    }
}

#[pyclass]
pub struct PyBibiByteTopic{
    inner: Arc<ByteTopic>,
}

#[pymethods]
impl PyBibiByteTopic{
    fn name(&self) -> String{
        self.inner.name().to_string()
    }

    fn publish(&self, data: &[u8]) -> PyResult<u64>{
        match self.inner.publish(data){
            Some(epoch) => Ok(epoch),
            None => Err(PyValueError::new_err("Data too large for slot")),
        }
    }

    fn try_receive(&self) -> Option<(Vec<u8>, u64)>{
        self.inner.try_receive()
    }

    fn peek_latest(&self) -> Option<(Vec<u8>, u64)>{
        self.inner.peek_latest()
    }

    fn len(&self) -> usize{
        self.inner.len()
    }

    fn is_empty(&self) -> bool{
        self.inner.is_empty()
    }

    fn latest_epoch(&self) -> u64{
        self.inner.latest_epoch()
    }

    fn capacity(&self) -> usize{
        self.inner.capacity()
    }
}

#[pyclass]
pub struct PyBibiTypedTopic{
    inner: Arc<ByteTopic>,
    msg_size: usize,
}

#[pymethods]
impl PyBibiTypedTopic{
    fn name(&self) -> String{
        self.inner.name().to_string()
    }

    fn publish(&self, data: &[u8]) -> PyResult<u64>{
        if data.len() != self.msg_size{
            return Err(PyValueError::new_err(
                format!("Expected {} bytes, got {}", self.msg_size, data.len())
            ));
        }
        match self.inner.publish(data){
            Some(epoch) => Ok(epoch),
            None => Err(PyValueError::new_err("Data too large for slot")),
        }
    }

    fn try_receive(&self) -> PyResult<Option<(Vec<u8>, u64)>>{
        match self.inner.try_receive(){
            Some((data, epoch)) =>{
                if data.len() != self.msg_size{
                    return Err(PyValueError::new_err("Size mismatch"));
                }
                Ok(Some((data, epoch)))
            }
            None => Ok(None),
        }
    }

    fn peek_latest(&self) -> Option<(Vec<u8>, u64)>{
        self.inner.peek_latest()
    }

    fn len(&self) -> usize{
        self.inner.len()
    }

    fn is_empty(&self) -> bool{
        self.inner.is_empty()
    }
}

#[pymodule]
fn bibi_sync(_py: Python, m: &PyModule) -> PyResult<()>{
    m.add_class::<PyBibiRegistry>()?;
    m.add_class::<PyBibiByteTopic>()?;
    m.add_class::<PyBibiTypedTopic>()?;
    Ok(())
}

#[cfg(test)]
mod tests{
    use super::*;

    #[test]
    fn test_py_registry(){
        let registry = PyBibiRegistry::new();
        let topic = registry.get_byte_topic("/test", 8);
        assert_eq!(topic.name(), "/test");
    }

    #[test]
    fn test_py_publish_receive(){
        let registry = PyBibiRegistry::new();
        let topic = registry.get_byte_topic("/test", 8);
        
        let epoch = topic.publish(&[1, 2, 3]).unwrap();
        assert_eq!(epoch, 1);

        let (data, _) = topic.try_receive().unwrap();
        assert_eq!(data, vec![1, 2, 3]);
    }

    #[test]
    fn test_py_shared_topic(){
        let registry = PyBibiRegistry::new();
        let topic1 = registry.get_byte_topic("/shared", 8);
        let topic2 = registry.get_byte_topic("/shared", 8);

        topic1.publish(&[0xAB, 0xCD]).unwrap();
        
        let (data, _) = topic2.try_receive().unwrap();
        assert_eq!(data, vec![0xAB, 0xCD]);
    }
}