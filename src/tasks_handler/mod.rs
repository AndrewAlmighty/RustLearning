mod types;
mod worker;

use types::Task;
use worker::Worker;

use std::sync::mpsc::{Receiver, channel};
use std::sync::atomic::{AtomicUsize, AtomicU8, Ordering};

use std::thread;

use std::time::Duration;

pub struct Threadpool {
    tasks: Vec<Task>,
    workers: Vec<(Worker, Option<thread::JoinHandle<()>>)>,
    receiver: Receiver<u8>
}

impl Threadpool {
    pub fn prepare(tasks_number: usize, threads_number: u8) -> Self {

        fn create_new_task() -> Task {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            static EXECUTION_TIME_IN_SECONDS: AtomicU8 = AtomicU8::new(1);
        
            let execution_time_is_seconds = EXECUTION_TIME_IN_SECONDS.fetch_add(3, Ordering::Relaxed);
            let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        
            if execution_time_is_seconds > 10 {
                EXECUTION_TIME_IN_SECONDS.store(execution_time_is_seconds - 10, Ordering::Relaxed);
            }
        
            Box::new(
                move || {
                    println!("Running job: {}. Should last for: {} seconds", id, execution_time_is_seconds);
                    std::thread::sleep(Duration::from_secs(execution_time_is_seconds.into()));
                    println!("Job: {} is done", id);
                }
            )
        }
        let (tx, rx) = channel();
        let mut tp = Threadpool { tasks: Vec::with_capacity(tasks_number), workers: Vec::with_capacity(threads_number.into()), receiver: rx };
        
        for _ in 0..tasks_number {
            tp.tasks.push(create_new_task());
        }

        for i in 0..threads_number {
            tp.workers.push( (Worker::create(i, tx.clone()), None) );
        }

        println!("Threadpool prepared!");
        tp
    }

    pub fn run(&mut self) {
        println!("Threadpool started! Tasks in queue: {}", self.tasks.len());
        for (worker, join_handler) in &mut self.workers {
            *join_handler = Some(worker.run());
        }
        self.work();
        println!("Threadpool finished work!")
    }

    fn work(&mut self) {
        let mut workers_working = self.workers.len();
        while !self.tasks.is_empty() {
            let id = self.receiver.recv().unwrap() as usize;
            match self.tasks.pop() {
                Some(task) => { self.workers[id].0.push_task(task.into()); }
                None => break,
            }   
        }

        while workers_working > 0 {
            let id = self.receiver.recv().unwrap() as usize;
            self.workers[id].0.stop_worker();
            workers_working -= 1;
        }

        for (_, join_handler) in &mut self.workers {
            let _ = join_handler.take().expect("There should be always something here").join();
        }
    }
}