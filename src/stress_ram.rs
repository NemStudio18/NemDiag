use std::sync::{Arc, atomic::{AtomicBool, Ordering, AtomicU32}};
use std::thread;
use std::time::Instant;
use sysinfo::System;

pub struct RamStress {
    is_running: Arc<AtomicBool>,
    throughput_mb_s: Arc<AtomicU32>,
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl RamStress {
    pub fn new() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            throughput_mb_s: Arc::new(AtomicU32::new(0)),
            thread_handle: None,
        }
    }

    pub fn start(&mut self) {
        if self.is_running.load(Ordering::SeqCst) {
            return;
        }

        let mut sys = System::new_all();
        sys.refresh_memory();
        let available_memory = sys.available_memory(); // in bytes

        // We will try to allocate 75% of available memory to not crash the OS
        let target_bytes = (available_memory as f64 * 0.75) as usize;
        let num_u64s = target_bytes / 8;

        self.is_running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.is_running);
        let throughput = Arc::clone(&self.throughput_mb_s);

        self.thread_handle = Some(thread::spawn(move || {
            // Allocate a massive buffer
            let mut buffer = vec![0u64; num_u64s];
            
            let patterns = [0x5555555555555555u64, 0xAAAAAAAAAAAAAAAAu64, 0xFFFFFFFFFFFFFFFFu64, 0x0000000000000000u64];
            let mut pattern_index = 0;

            let mut bytes_processed = 0;
            let mut last_update = Instant::now();

            while running.load(Ordering::Relaxed) {
                let current_pattern = patterns[pattern_index];
                
                // Write pattern
                for val in buffer.iter_mut() {
                    *val = current_pattern;
                    
                    // Periodically check if we should stop to avoid blocking for too long
                    bytes_processed += 8;
                    if bytes_processed % (1024 * 1024 * 64) == 0 {
                        if !running.load(Ordering::Relaxed) {
                            break;
                        }
                    }
                }

                // Verify pattern
                for val in buffer.iter() {
                    if *val != current_pattern {
                        // In a real memtest, we would log this bitflip error
                        println!("RAM Error detected! Expected {}, got {}", current_pattern, *val);
                    }
                    bytes_processed += 8;
                    if bytes_processed % (1024 * 1024 * 64) == 0 {
                        if !running.load(Ordering::Relaxed) {
                            break;
                        }
                    }
                }

                let elapsed = last_update.elapsed().as_secs_f64();
                if elapsed >= 1.0 {
                    let mb_s = (bytes_processed as f64 / 1024.0 / 1024.0 / elapsed) as u32;
                    throughput.store(mb_s, Ordering::Relaxed);
                    bytes_processed = 0;
                    last_update = Instant::now();
                }

                pattern_index = (pattern_index + 1) % patterns.len();
            }

            // T14: Final throughput update — ensures score is non-zero if 1s timer never fired
            let elapsed = last_update.elapsed().as_secs_f64();
            if elapsed > 0.05 && bytes_processed > 0 {
                let mb_s = (bytes_processed as f64 / 1024.0 / 1024.0 / elapsed) as u32;
                if mb_s > 0 {
                    throughput.store(mb_s, Ordering::Relaxed);
                }
            }
        }));
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn get_throughput(&self) -> u32 {
        self.throughput_mb_s.load(Ordering::Relaxed)
    }
}

impl Drop for RamStress {
    fn drop(&mut self) {
        self.stop();
    }
}
