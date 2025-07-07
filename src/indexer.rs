use std::time::Duration;
use std::fs::Metadata;
use std::path::{PathBuf, Path};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;

use crossbeam::channel::{Sender, Receiver, unbounded};
use dashmap::DashMap;

pub struct DirectoryScanner {
    dir_path: PathBuf,
    dir_scan_finished: Arc<AtomicBool>,
    sender: Sender<PathBuf>,
    workers: Vec<thread::JoinHandle<()>>,
    metadata: Arc<DashMap<PathBuf, Metadata>>
}

impl DirectoryScanner {
    pub fn create(dir_path: PathBuf, workers_count: u8) -> Self {
        let dir_scan_finished = Arc::new(AtomicBool::new(false));
        let (s, r) = unbounded();
        let mut workers = Vec::with_capacity(workers_count as usize);
        let metadata_map = Arc::new(DashMap::<PathBuf, Metadata>::new());

        for i in 0..workers_count {
            let scan_finished = Arc::clone(&dir_scan_finished);
            let receiver: Receiver<PathBuf> = r.clone();
            let metadata = Arc::clone(&metadata_map);
            let thread_handler = thread::spawn(move || {
                loop {
                    //in order to not execute everything too fast
                    std::thread::sleep(Duration::from_millis(50));

                    if let Ok(path) = receiver.try_recv() {
                        println!("Thread: {} reads metadata about: {}", i, path.display());
                        let m = path.metadata().expect("Reading metadata failed");
                        metadata.insert(path, m);
                    }
                    else if scan_finished.load(Ordering::Relaxed) {
                        break;
                    }
                }
            });

            workers.push(thread_handler);
        }


        DirectoryScanner { dir_path: dir_path, dir_scan_finished: Arc::clone(&dir_scan_finished), sender: s.clone(), workers: workers, metadata: Arc::clone(&metadata_map) }
    }

    pub fn run(&mut self) {
        println!("Scanning directory: {}", self.dir_path.display());
        self.gather_metadata_about_files_in_directory(self.dir_path.as_path());
        self.dir_scan_finished.store(true, Ordering::Relaxed);
        for t in self.workers.drain(..) {
            match t.join() {
                Ok(()) => {}
                Err(_) => { println!("Error during joining threads"); }
            }
        }

        println!("Work done. Results:\n");

        for it in self.metadata.iter() {
            println!("{}: {:?}", it.key().display(), it.value());
        }
    }

    fn gather_metadata_about_files_in_directory(&self, path: &Path) {
        for entry in path.read_dir().expect("read_dir call failed") {
            if let Ok(entry) = entry {
                let p = entry.path();
                if p.is_dir() {
                    self.gather_metadata_about_files_in_directory(p.as_path());
                }
                else {
                    assert_eq!(self.sender.send(p), Ok(()));
                }
            }
        }
    }
}