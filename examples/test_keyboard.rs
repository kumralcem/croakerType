use evdev::Device;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let device_path = Path::new("/dev/input/event0");
    println!("Opening device: {:?}", device_path);
    
    let mut device = Device::open(device_path)?;
    println!("✅ Device opened successfully!");
    println!("Device name: {:?}", device.name());
    
    if let Some(keys) = device.supported_keys() {
        println!("Device supports {} keys", keys.iter().count());
        if keys.contains(evdev::Key::KEY_RIGHTALT) {
            println!("✅ Device supports KEY_RIGHTALT");
        }
    }
    
    println!("\n📝 Monitoring for 10 seconds - press ANY key (including RightAlt)...");
    println!("   You should see KEY events below:\n");
    
    let start = std::time::Instant::now();
    let mut event_count = 0;
    
    while start.elapsed().as_secs() < 10 {
        match device.fetch_events() {
            Ok(events) => {
                for event in events {
                    event_count += 1;
                    if event.event_type() == evdev::EventType::KEY {
                        println!("🔑 KEY EVENT: code={}, value={} (1=press, 0=release, 2=repeat)", 
                                event.code(), event.value());
                    } else {
                        println!("📦 Other event: type={:?}, code={}, value={}", 
                                event.event_type(), event.code(), event.value());
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No events - this is normal
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(e) => {
                eprintln!("❌ Error reading events: {}", e);
                break;
            }
        }
    }
    
    println!("\n📊 Total events received: {}", event_count);
    if event_count == 0 {
        println!("⚠️  WARNING: No events received! This means:");
        println!("   1. Another process might be consuming events");
        println!("   2. The device might need different permissions");
        println!("   3. The device might not be the right one");
    }
    
    Ok(())
}

