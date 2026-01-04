/**
 * Thrust Mixer
 * 
 * Converts 6-DoF thrust commands (surge, sway, heave, roll, pitch, yaw)
 * into individual thruster values using the thruster configuration.
 */

/// Thrust command for 6 degrees of freedom
#[derive(Debug, Clone, Copy, Default)]
pub struct ThrustCommand {
    pub surge: f32,
    pub sway: f32,
    pub heave: f32,
    pub roll: f32,
    pub pitch: f32,
    pub yaw: f32,
}

/// Thrust mixer configuration (matches your thruster layout)
/// Default values for 6-thruster vectored configuration
#[derive(Debug, Clone)]
pub struct ThrustMixer {
    /// Contribution of each DoF to each thruster [6 thrusters x 6 DoFs]
    pub mix_matrix: [[f32; 6]; 6],
    /// Maximum thrust per thruster
    pub max_thrust: f32,
}

impl Default for ThrustMixer {
    fn default() -> Self {
        // Standard vectored 6-thruster configuration
        // Rows: thrusters, Columns: [surge, sway, heave, roll, pitch, yaw]
        Self {
            mix_matrix: [
                // Thruster 0 (front-left horizontal)
                [1.0, -1.0, 0.0, 0.0, 0.0, -1.0],
                // Thruster 1 (front-right horizontal)  
                [1.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                // Thruster 2 (rear-left horizontal)
                [-1.0, -1.0, 0.0, 0.0, 0.0, 1.0],
                // Thruster 3 (rear-right horizontal)
                [-1.0, 1.0, 0.0, 0.0, 0.0, -1.0],
                // Thruster 4 (left vertical)
                [0.0, 0.0, 1.0, -1.0, 1.0, 0.0],
                // Thruster 5 (right vertical)
                [0.0, 0.0, 1.0, 1.0, 1.0, 0.0],
            ],
            max_thrust: 100.0,
        }
    }
}

impl ThrustMixer {
    /// Mix 6-DoF command into individual thruster values
    pub fn mix(&self, cmd: &ThrustCommand) -> [f32; 6] {
        let dof = [cmd.surge, cmd.sway, cmd.heave, cmd.roll, cmd.pitch, cmd.yaw];
        let mut output = [0.0f32; 6];
        
        for (i, row) in self.mix_matrix.iter().enumerate() {
            let mut sum = 0.0;
            for (j, &coeff) in row.iter().enumerate() {
                sum += coeff * dof[j];
            }
            output[i] = sum.clamp(-self.max_thrust, self.max_thrust);
        }
        
        output
    }
    
    /// Convert thrust values (-100 to 100) to PWM (1100 to 1900)
    pub fn thrust_to_pwm(thrust: f32) -> i32 {
        // Linear mapping: -100 -> 1100, 0 -> 1500, 100 -> 1900
        (1500.0 + thrust * 4.0) as i32
    }
    
    /// Convert thrust array to PWM array
    pub fn to_pwm(thrusts: &[f32; 6]) -> [i32; 6] {
        [
            Self::thrust_to_pwm(thrusts[0]),
            Self::thrust_to_pwm(thrusts[1]),
            Self::thrust_to_pwm(thrusts[2]),
            Self::thrust_to_pwm(thrusts[3]),
            Self::thrust_to_pwm(thrusts[4]),
            Self::thrust_to_pwm(thrusts[5]),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_neutral_thrust() {
        let mixer = ThrustMixer::default();
        let cmd = ThrustCommand::default();
        let output = mixer.mix(&cmd);
        assert!(output.iter().all(|&x| x == 0.0));
    }
    
    #[test]
    fn test_surge() {
        let mixer = ThrustMixer::default();
        let cmd = ThrustCommand { surge: 50.0, ..Default::default() };
        let output = mixer.mix(&cmd);
        // Front thrusters should be positive, rear negative
        assert!(output[0] > 0.0);
        assert!(output[1] > 0.0);
        assert!(output[2] < 0.0);
        assert!(output[3] < 0.0);
    }
}
