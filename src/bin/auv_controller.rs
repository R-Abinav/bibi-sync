/**
 * AUV Controller Binary
 * 
 * Runs the unified AUV controller that:
 * 1. Connects to STM32 via UART
 * 2. Receives sensor data
 * 3. Accepts thrust commands
 * 4. Sends PWM to thrusters
 * 
 * Usage: auv_controller [port] [baud]
 * Default: /dev/ttyACM0, 9600
 */

use bibi_sync::auv::AuvController;
use std::sync::Arc;
use std::io::{self, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    let port = args.get(1).map(|s| s.as_str()).unwrap_or("/dev/ttyACM0");
    let baud: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(9600);
    
    println!("==============================================");
    println!("  BiBi-Sync AUV Controller");
    println!("==============================================");
    println!("  Port: {}", port);
    println!("  Baud: {}", baud);
    println!("==============================================\n");
    
    // Create controller
    let controller = Arc::new(AuvController::new(port).with_baud(baud));
    
    // Start controller in background
    let ctrl = controller.clone();
    let handle = ctrl.start_background();
    
    // Wait for connection
    std::thread::sleep(std::time::Duration::from_secs(1));
    
    println!("\n[Commands]");
    println!("  w/s - surge forward/backward");
    println!("  a/d - yaw left/right");
    println!("  q/e - heave up/down");
    println!("  space - stop all");
    println!("  x - exit\n");
    
    // Simple keyboard control loop
    println!("Enter commands (or 'x' to exit):");
    
    loop {
        print!("> ");
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            break;
        }
        
        let cmd = input.trim();
        
        match cmd {
            "w" => {
                controller.set_surge(30.0);
                println!("[SURGE +30]");
            }
            "s" => {
                controller.set_surge(-30.0);
                println!("[SURGE -30]");
            }
            "a" => {
                controller.set_yaw(-30.0);
                println!("[YAW -30]");
            }
            "d" => {
                controller.set_yaw(30.0);
                println!("[YAW +30]");
            }
            "q" => {
                controller.set_heave(30.0);
                println!("[HEAVE +30]");
            }
            "e" => {
                controller.set_heave(-30.0);
                println!("[HEAVE -30]");
            }
            " " | "stop" => {
                controller.stop();
                println!("[STOP]");
            }
            "sensors" | "r" => {
                let sensors = controller.get_sensors();
                if let Some((r, p, y)) = controller.get_orientation() {
                    println!("[ORIENT] roll={:.1}° pitch={:.1}° yaw={:.1}°", r, p, y);
                }
                if let Some(d) = controller.get_depth() {
                    println!("[DEPTH] {:.3} m", d);
                }
            }
            "x" | "exit" | "quit" => {
                println!("[SHUTDOWN]");
                controller.stop();
                controller.shutdown();
                break;
            }
            "" => {}
            _ => println!("Unknown command: {}", cmd),
        }
    }
    
    let _ = handle.join();
    println!("Goodbye!");
}
