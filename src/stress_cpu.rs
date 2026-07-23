use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::{Duration, Instant};

pub struct CpuStress {
    is_running: Arc<AtomicBool>,
    iterations: Arc<std::sync::atomic::AtomicU64>,
    threads: Vec<thread::JoinHandle<()>>,
}

impl CpuStress {
    pub fn new() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            iterations: Arc::new(std::sync::atomic::AtomicU64::new(0)),
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
            let iter_flag = Arc::clone(&self.iterations);
            let handle = thread::spawn(move || {
                // Heavy computation loop to stress the CPU
                let mut x = 1.0f64;
                while running_flag.load(Ordering::Relaxed) {
                    for i in 1..1000 {
                        x = (x * 1.0000001).sin().cos().tan();
                        x += i as f64;
                    }
                    iter_flag.fetch_add(1000, Ordering::Relaxed);
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

    pub fn get_score(&self) -> u64 {
        // Convert raw iterations to a more readable score (e.g., iterations / 10000)
        self.iterations.load(Ordering::Relaxed) / 10000
    }
}

impl Drop for CpuStress {
    fn drop(&mut self) {
        self.stop();
    }
}
