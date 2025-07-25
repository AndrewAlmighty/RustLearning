mod fcfs;
mod scheduler_with_priority;
mod rr;

use crate::process_scheduler_simulator::process::{Process};
use crate::process_scheduler_simulator::WorkDoneSender;
use crate::process_scheduler_simulator::types::Seconds;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};

pub(super) struct SchedulerBase {
    pub(super) executed_processes_counter: Arc<AtomicUsize>,
    pub(super) process_generation_finished: AtomicBool
}

impl SchedulerBase {
    pub(super) fn create() -> Self {
        SchedulerBase {
            executed_processes_counter: Arc::new(AtomicUsize::new(0)),
            process_generation_finished: AtomicBool::new(false)
        }
    }
}

pub(super) trait BaseProvider {
    fn base(&self) -> &SchedulerBase;
}

#[async_trait::async_trait]
pub trait Scheduler : Sync + Send + BaseProvider {
    fn notify_no_more_processes(&self) {
        self.base().process_generation_finished.store(true, Ordering::Relaxed);
    }

    fn get_total_executed_processes_count(&self) -> usize {
        self.base().executed_processes_counter.load(Ordering::Relaxed)
    }

    fn start_work(self: Arc<Self>, work_done_sender: WorkDoneSender);
    async fn accept_process(&self, process: Arc<Process>);
}

pub fn create(algorithm: String, threads_count: usize, queue_capacity: usize, rr_interval: Seconds) -> Result<Arc<dyn Scheduler>, String> {
    assert!(threads_count > 0);

    match algorithm.to_lowercase().as_str() {
        "fcfs" => {
            println!("Chosen algorithm: First come, first serve");
            Ok(fcfs::FCFSScheduler::create(queue_capacity, threads_count))
        }
        "srtf" => {
            println!("Chosen algorithm: Shortest Remaining Time First");
            Ok(scheduler_with_priority::SRTFScheduler::create(queue_capacity, threads_count))
        }
        "priority" => {
            println!("Chosen algorithm: highest priority first");
            Ok(scheduler_with_priority::PriorityScheduler::create(queue_capacity, threads_count))
        }
        "rr" => {
            println!("Chosen algorithm: round robin");
            Ok(rr::RoundRobinScheduler::create(queue_capacity, threads_count, rr_interval))
        }
        a => Err(format!("Not valid algorithm: {}", a))
    }
}


