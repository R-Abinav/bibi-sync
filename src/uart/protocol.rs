#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ImuMsg{
    pub accel_x: f32,     //m/s² (already multiplied by G on STM32)
    pub accel_y: f32,
    pub accel_z: f32,
    pub gyro_x: f32,      //rad/s
    pub gyro_y: f32,
    pub gyro_z: f32,
    pub mag_x: f32,       //µT
    pub mag_y: f32,
    pub mag_z: f32,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct OrientationMsg{
    pub roll: f32,        //degrees
    pub pitch: f32,
    pub yaw: f32,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DepthMsg{
    pub depth: f32,       //meters
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ThrusterPwmCmd{
    pub pwm: [i32; 6],    //PWM values for all 6 thrusters (1000-2000 µs)
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LedCmd{
    pub indicator: i16,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CalibrationCmd{
    pub enable: bool,
}

//message sizes
pub const IMU_MSG_SIZE: usize = 36;        //9 * f32
pub const ORIENTATION_MSG_SIZE: usize = 12; //3 * f32
pub const DEPTH_MSG_SIZE: usize = 4;        //1 * f32
pub const THRUSTER_PWM_SIZE: usize = 24;    //6 * i32
pub const LED_CMD_SIZE: usize = 2;          //1 * i16
pub const CALIBRATION_CMD_SIZE: usize = 1;  //1 * bool

impl ThrusterPwmCmd{
    pub fn new(pwm_values: [i32; 6]) -> Self{
        ThrusterPwmCmd{ pwm: pwm_values }
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self>{
        if data.len() < THRUSTER_PWM_SIZE{
            return None;
        }
        unsafe{
            Some(std::ptr::read_unaligned(data.as_ptr() as *const Self))
        }
    }

    pub fn to_bytes(&self) -> Vec<u8>{
        let mut bytes = vec![0u8; THRUSTER_PWM_SIZE];
        unsafe{
            std::ptr::copy_nonoverlapping(
                self as *const Self as *const u8,
                bytes.as_mut_ptr(),
                THRUSTER_PWM_SIZE
            );
        }
        bytes
    }
}

impl ImuMsg{
    pub fn from_bytes(data: &[u8]) -> Option<Self>{
        if data.len() < IMU_MSG_SIZE{
            return None;
        }
        unsafe{
            Some(std::ptr::read_unaligned(data.as_ptr() as *const Self))
        }
    }
}

impl OrientationMsg{
    pub fn from_bytes(data: &[u8]) -> Option<Self>{
        if data.len() < ORIENTATION_MSG_SIZE{
            return None;
        }
        unsafe{
            Some(std::ptr::read_unaligned(data.as_ptr() as *const Self))
        }
    }
}

impl DepthMsg{
    pub fn from_bytes(data: &[u8]) -> Option<Self>{
        if data.len() < DEPTH_MSG_SIZE{
            return None;
        }
        unsafe{
            Some(std::ptr::read_unaligned(data.as_ptr() as *const Self))
        }
    }
}

#[cfg(test)]
mod tests{
    use super::*;

    #[test]
    fn test_thruster_pwm_cmd(){
        let cmd = ThrusterPwmCmd::new([1500, 1600, 1400, 1550, 1450, 1500]);
        let bytes = cmd.to_bytes();
        assert_eq!(bytes.len(), 24);

        let decoded = ThrusterPwmCmd::from_bytes(&bytes).unwrap();
        let pwm = decoded.pwm; //copy to avoid unaligned access
        assert_eq!(pwm[0], 1500);
        assert_eq!(pwm[2], 1400);
        assert_eq!(pwm[3], 1550);
        assert_eq!(pwm[1], 1600);
        assert_eq!(pwm[5], 1500);
    }

    #[test]
    fn test_imu_msg_size(){
        assert_eq!(std::mem::size_of::<ImuMsg>(), IMU_MSG_SIZE);
    }
}