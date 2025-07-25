pub mod types;

mod scheduler;
mod process;

use crate::Config;

use process::ProcessFactory;
use scheduler::Scheduler;
use types::Seconds;

use chrono::Local;
use rand::{rng, Rng};
use tokio::time::Duration;

use std::sync::Arc;

pub type WorkDoneSender = tokio::sync::watch::Sender<bool>;

pub struct Simulator {
    scheduler: Arc<dyn Scheduler>,
    factory: ProcessFactory,
    processes_count: usize,
    process_creation_interval: (Seconds, Seconds)
}

impl Simulator {
    pub fn create(cfg: Config) -> Result<Self, String> {
        let scheduler = scheduler::create(cfg.algorithm, cfg.threads_count.into(), cfg.processes_count, cfg.rr_interval)?;
        let factory = ProcessFactory::create(cfg.burst_min, cfg.burst_max);
        assert!(cfg.min_interval <= cfg.max_interval);
        Ok(Simulator { scheduler: scheduler, factory: factory, processes_count: cfg.processes_count, process_creation_interval:(cfg.min_interval, cfg.max_interval) })
    }

    pub async fn run(&mut self) {
        println!("{} processes will be created", self.processes_count);
        let (work_done_sender, work_done_receiver) = tokio::sync::watch::channel::<bool>(false);
        let mut no_more_processes_to_create = false;
        let mut created_process_counter = 0usize;
        let begin = Local::now();
        let mut random_generator = if self.process_creation_interval.0 == 0 || self.process_creation_interval.1 == 0 { Some(rng()) } else { None };
        println!("\n{} - Simulation begins!", begin.to_rfc3339());

        Scheduler::start_work(self.scheduler.clone(), work_done_sender);

        while *work_done_receiver.borrow() == false {
            if created_process_counter < self.processes_count {
                let process = self.factory.create_process();
                self.scheduler.accept_process(process).await;
                created_process_counter += 1;
                if let Some(ref mut generator) = random_generator {
                    let interval = generator.random_range(self.process_creation_interval.0..=self.process_creation_interval.1);
                    tokio::time::sleep(Duration::from_secs(interval.into())).await;
                }
            }
            else {
                if !no_more_processes_to_create {                       
                    println!("{} - No more processes will be created!", Local::now().to_rfc3339());
                    self.scheduler.notify_no_more_processes();
                    no_more_processes_to_create = true;
                }
                else {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        }

        let end = Local::now();
        println!("{} - Simulation ends! Total execution time: {} second(s), total processes executed: {}", end.to_rfc3339(), (end-begin).num_seconds(), self.scheduler.get_total_executed_processes_count());
    }
}