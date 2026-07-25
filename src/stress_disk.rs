use std::sync::{Arc, atomic::{AtomicBool, Ordering, AtomicU32}};
use std::thread;
use std::time::Instant;
use std::fs::OpenOptions;
use std::io::{Write, Read, Seek, SeekFrom};
use std::path::PathBuf;

pub struct DiskStress {
    is_running: Arc<AtomicBool>,
    throughput_mb_s: Arc<AtomicU32>,
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl DiskStress {
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

        self.is_running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.is_running);
        let throughput = Arc::clone(&self.throughput_mb_s);

        self.thread_handle = Some(thread::spawn(move || {
            // T11: Detect if /tmp is tmpfs — if so, we'd be measuring RAM, not the disk.
            // Fall back to $HOME in that case.
            let file_path = {
                let is_tmp_tmpfs = std::fs::read_to_string("/proc/mounts")
                    .map(|mounts| mounts.lines().any(|line| {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        parts.len() >= 3 && parts[1] == "/tmp" && parts[2] == "tmpfs"
                    }))
                    .unwrap_or(false);

                if is_tmp_tmpfs {
                    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                    PathBuf::from(home).join(".nemdiag_stress_test.tmp")
                } else {
                    PathBuf::from("/tmp/nemdiag_stress_test.tmp")
                }
            };
            
            let mut options = OpenOptions::new();
            options.read(true).write(true).create(true).truncate(true);

            #[cfg(target_os = "linux")]
            {
                use std::os::unix::fs::OpenOptionsExt;
                // O_DIRECT (040000) prevents using the OS page cache.
                // O_SYNC (010000) ensures it writes directly to disk.
                options.custom_flags(0o40000 | 0o10000);
            }

            let file_result = options.open(&file_path);
            if file_result.is_err() {
                // Fallback without O_DIRECT if not supported (e.g., tmpfs or Windows)
                let mut fallback_options = OpenOptions::new();
                fallback_options.read(true).write(true).create(true).truncate(true);
                let _ = fallback_options.open(&file_path);
            }
            
            let mut file = match OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&file_path) {
                Ok(f) => f,
                Err(e) => {
                    println!("Failed to open disk stress file: {}", e);
                    running.store(false, Ordering::SeqCst);
                    return;
                }
            };

            // O_DIRECT requires block-aligned memory (usually 512 or 4096 bytes). 
            // We use a simple 4MB buffer, which is a multiple of 4096.
            // Note: aligned memory allocation in stable Rust is tricky without crates, 
            // so we rely on standard Vec which is usually 16-byte aligned. If O_DIRECT fails,
            // we will just use normal I/O.
            let chunk_size = 1024 * 1024 * 4; // 4MB chunks
            let write_buffer = vec![0xAAu8; chunk_size];
            let mut read_buffer = vec![0u8; chunk_size];

            let mut bytes_processed = 0;
            let mut last_update = Instant::now();

            while running.load(Ordering::Relaxed) {
                // Write pass
                let _ = file.seek(SeekFrom::Start(0));
                for _ in 0..100 { // 400MB write
                    if !running.load(Ordering::Relaxed) { break; }
                    if file.write_all(&write_buffer).is_ok() {
                        bytes_processed += chunk_size;
                    }
                }

                // Read pass
                let _ = file.seek(SeekFrom::Start(0));
                for _ in 0..100 { // 400MB read
                    if !running.load(Ordering::Relaxed) { break; }
                    if file.read_exact(&mut read_buffer).is_ok() {
                        bytes_processed += chunk_size;
                    }
                }

                let elapsed = last_update.elapsed().as_secs_f64();
                if elapsed >= 1.0 {
                    let mb_s = (bytes_processed as f64 / 1024.0 / 1024.0 / elapsed) as u32;
                    throughput.store(mb_s, Ordering::Relaxed);
                    bytes_processed = 0;
                    last_update = Instant::now();
                }
            }

            let _ = std::fs::remove_file(file_path);
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

impl Drop for DiskStress {
    fn drop(&mut self) {
        self.stop();
    }
}
