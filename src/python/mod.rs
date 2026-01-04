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

// AUV Controller Python bindings
use crate::auv::{AuvController, thrust_mixer::ThrustCommand};

#[pyclass]
pub struct PyAuvController {
    inner: Arc<AuvController>,
    _handle: Option<std::thread::JoinHandle<()>>,
}

#[pymethods]
impl PyAuvController {
    #[new]
    #[pyo3(signature = (port = "/dev/ttyACM0", baud = 9600))]
    fn new(port: &str, baud: u32) -> Self {
        let controller = Arc::new(AuvController::new(port).with_baud(baud));
        let ctrl = controller.clone();
        let handle = ctrl.start_background();
        
        // Give it time to connect
        std::thread::sleep(std::time::Duration::from_millis(500));
        
        PyAuvController {
            inner: controller,
            _handle: Some(handle),
        }
    }
    
    fn set_surge(&self, value: f32) {
        self.inner.set_surge(value);
    }
    
    fn set_sway(&self, value: f32) {
        self.inner.set_sway(value);
    }
    
    fn set_heave(&self, value: f32) {
        self.inner.set_heave(value);
    }
    
    fn set_roll(&self, value: f32) {
        self.inner.set_roll(value);
    }
    
    fn set_pitch(&self, value: f32) {
        self.inner.set_pitch(value);
    }
    
    fn set_yaw(&self, value: f32) {
        self.inner.set_yaw(value);
    }
    
    fn set_thrust(&self, surge: f32, sway: f32, heave: f32, roll: f32, pitch: f32, yaw: f32) {
        self.inner.set_thrust(ThrustCommand {
            surge, sway, heave, roll, pitch, yaw
        });
    }
    
    fn stop(&self) {
        self.inner.stop();
    }
    
    fn get_orientation(&self) -> Option<(f32, f32, f32)> {
        self.inner.get_orientation()
    }
    
    fn get_depth(&self) -> Option<f32> {
        self.inner.get_depth()
    }
    
    fn shutdown(&self) {
        self.inner.stop();
        self.inner.shutdown();
    }
}

impl Drop for PyAuvController {
    fn drop(&mut self) {
        self.inner.stop();
        self.inner.shutdown();
    }
}

#[pymodule]
fn bibi_sync(_py: Python, m: &PyModule) -> PyResult<()>{
    m.add_class::<PyBibiRegistry>()?;
    m.add_class::<PyBibiByteTopic>()?;
    m.add_class::<PyBibiTypedTopic>()?;
    m.add_class::<PyAuvController>()?;
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