use std::sync::{Arc, atomic::{AtomicBool, Ordering, AtomicU32}};
use std::thread;
use std::time::Instant;

pub static RAM_ERRORS: AtomicU32 = AtomicU32::new(0);

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
        
        RAM_ERRORS.store(0, Ordering::Relaxed);

        // Buffer de 256 Mo — assez grand pour mesurer, assez petit pour que
        // chaque chunk de 32 Mo soit traité en bien moins d'une seconde.
        let num_u64s: usize = (256 * 1024 * 1024) / 8;
        // Chunk de 32 Mo = ~4 millions de u64
        let chunk_u64s: usize = (32 * 1024 * 1024) / 8;

        self.is_running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.is_running);
        let throughput = Arc::clone(&self.throughput_mb_s);

        self.thread_handle = Some(thread::spawn(move || {
            let mut buffer = vec![0u64; num_u64s];
            
            let patterns = [0x5555555555555555u64, 0xAAAAAAAAAAAAAAAAu64, 0xFFFFFFFFFFFFFFFFu64, 0x0000000000000000u64];
            let mut pattern_index = 0;

            let mut bytes_processed: u64 = 0;
            let mut last_update = Instant::now();

            'outer: while running.load(Ordering::Relaxed) {
                let current_pattern = patterns[pattern_index];
                
                // Écriture par chunks de 32 Mo avec mise à jour du débit à chaque chunk
                for chunk in buffer.chunks_mut(chunk_u64s) {
                    if !running.load(Ordering::Relaxed) { break 'outer; }
                    for val in chunk.iter_mut() {
                        *val = current_pattern;
                    }
                    bytes_processed += (chunk.len() * 8) as u64;

                    let elapsed = last_update.elapsed().as_secs_f64();
                    if elapsed >= 1.0 {
                        let mb_s = (bytes_processed as f64 / 1_048_576.0 / elapsed) as u32;
                        throughput.store(mb_s, Ordering::Relaxed);
                        bytes_processed = 0;
                        last_update = std::time::Instant::now();
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1)); // PREVENT OS FREEZE
                }

                // Vérification par chunks de 32 Mo
                for chunk in buffer.chunks(chunk_u64s) {
                    if !running.load(Ordering::Relaxed) { break 'outer; }
                    for val in chunk.iter() {
                        if *val != current_pattern {
                            println!("RAM Error! Expected {:#x}, got {:#x}", current_pattern, *val);
                            RAM_ERRORS.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    bytes_processed += (chunk.len() * 8) as u64;

                    let elapsed = last_update.elapsed().as_secs_f64();
                    if elapsed >= 1.0 {
                        let mb_s = (bytes_processed as f64 / 1_048_576.0 / elapsed) as u32;
                        throughput.store(mb_s, Ordering::Relaxed);
                        bytes_processed = 0;
                        last_update = std::time::Instant::now();
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1)); // PREVENT OS FREEZE
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

    /// Signale l'arrêt sans bloquer (pour contextes async).
    /// Retourne le JoinHandle pour que l'appelant puisse faire join en spawn_blocking.
    pub fn stop_signal(&mut self) -> Option<thread::JoinHandle<()>> {
        self.is_running.store(false, Ordering::SeqCst);
        self.thread_handle.take()
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
