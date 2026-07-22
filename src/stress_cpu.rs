use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::{Duration, Instant};

pub struct CpuStress {
    is_running: Arc<AtomicBool>,
    threads: Vec<thread::JoinHandle<()>>,
}

impl CpuStress {
    pub fn new() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            threads: Vec::new(),
        }
    }

    pub fn start(&mut self) {
        if self.is_running.load(Ordering::SeqCst) {
            return;
        }

        self.is_running.store(true, Ordering::SeqCst);
        let available_cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);

        for _ in 0..available_cores {
            let running_flag = Arc::clone(&self.is_running);
            let handle = thread::spawn(move || {
                // Heavy computation loop to stress the CPU
                let mut x = 1.0f64;
                while running_flag.load(Ordering::Relaxed) {
                    for i in 1..1000 {
                        x = (x * 1.0000001).sin().cos().tan();
                        x += i as f64;
                    }
                }
            });
            self.threads.push(handle);
        }
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);
        while let Some(handle) = self.threads.pop() {
            let _ = handle.join();
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
}

impl Drop for CpuStress {
    fn drop(&mut self) {
        self.stop();
    }
}
