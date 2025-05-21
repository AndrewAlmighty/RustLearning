use crate::tasks_handler::types::Task;

use std::sync::{Mutex, Arc};
use std::sync::mpsc::Sender;
use std::sync::atomic::{AtomicBool, Ordering};

use std::thread;

use std::time::Duration;

pub(super) struct Worker {
    id: u8,
    sender: Sender<u8>,
    task: Arc<Mutex<Option<Box<Task>>>>,
    stop_worker: Arc<AtomicBool>
}

impl Worker {
    pub(super) fn create(id: u8, tx: Sender<u8>) -> Self {
        Worker { id: id, sender: tx, task: Arc::new(Mutex::new(None)), stop_worker: Arc::new(AtomicBool::new(false)) }
    }

    pub(super) fn run(&mut self) -> thread::JoinHandle<()> {

        let id = self.id;
        let stop_worker = self.stop_worker.clone();
        let arc_task = self.task.clone();
        let sender = self.sender.clone();

        thread::spawn(move || {
            println!("Worker: {} starting", id);
            let mut tasks_done = 0usize;
            let _ = sender.send(id);
            while !stop_worker.load(Ordering::Relaxed) {
                {
                    if let Some(mut task_opt) = arc_task.lock().ok() {
                        if let Some(task) = task_opt.take() {
                            task();
                            tasks_done += 1;
                            let _ = sender.send(id);
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            println!("Worker: {} finished work Tasks done: {}", id, tasks_done);
        })
    }

    pub(super) fn push_task(&mut self, task: Box<Task>) {
        loop {
            {
                if let Some(mut task_opt) = self.task.lock().ok() {
                    if task_opt.is_none() {
                        *task_opt = Some(task);
                        break;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    pub(super) fn stop_worker(&mut self) {
        self.stop_worker.store(true, Ordering::Relaxed);
    }
}