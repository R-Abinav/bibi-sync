/**
 * AUV Controller Module
 * 
 * Unified controller that combines:
 * - UART bridge to STM32
 * - Motion control (thrust mixing)
 * - Python API for task scripts
 * 
 * All in one process with shared BiBi-Sync ring buffers.
 */

pub mod controller;
pub mod thrust_mixer;

pub use controller::AuvController;
pub use thrust_mixer::ThrustMixer;
