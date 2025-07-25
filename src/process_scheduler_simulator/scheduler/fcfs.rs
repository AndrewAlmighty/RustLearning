use crate::process_scheduler_simulator::process::Process;
use crate::process_scheduler_simulator::scheduler::{BaseProvider, SchedulerBase, Scheduler};
use crate::process_scheduler_simulator::WorkDoneSender;

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::sync::{Mutex, Semaphore};

// First come, First serve
pub(super) struct FCFSScheduler {
    queued_processes: Arc<Mutex<VecDeque<Arc<Process>>>>,
    base: SchedulerBase,
    concurrent_threads_permissions: Option<Arc<Semaphore>>,
    max_concurrent_tasks: usize,
}

impl FCFSScheduler {
    pub(super) fn create(queue_processes_size: usize, threads_count: usize) -> Arc<Self> {
        Arc::new(FCFSScheduler {
            queued_processes: Arc::new(Mutex::new(VecDeque::with_capacity(queue_processes_size))),
            base: SchedulerBase::create(),
            concurrent_threads_permissions: if threads_count > 1 { Some(Arc::new(Semaphore::new(threads_count.into()))) } else { None },
            max_concurrent_tasks: threads_count,
        })
    }

    async fn run(&self, work_done_sender: WorkDoneSender) {
        loop {
            let maybe_process = {
                let mut queue = self.queued_processes.lock().await;
                queue.pop_front()
            };

            match maybe_process {
                Some(mut p) => {
                    if let Some(permissions) = &self.concurrent_threads_permissions {
                        let mut process = Arc::clone(&p);
                        let permission = permissions.clone().acquire_owned().await.unwrap();
                        let executed_processes_counter = Arc::clone(&self.base.executed_processes_counter);
                        tokio::spawn(async move {
                            let mutable_process = Arc::get_mut(&mut process).unwrap();
                            mutable_process.run(None, None, None).await;
                            drop(permission);
                            executed_processes_counter.fetch_add(1, Ordering::Relaxed);
                        });
                    }
                    else {
                        let process = Arc::get_mut(&mut p).unwrap();
                        process.run(None, None, None).await;
                        self.base.executed_processes_counter.fetch_add(1, Ordering::Relaxed);
                    }
                }
                None => {
                    if self.base.process_generation_finished.load(Ordering::Relaxed) &&
                        self.concurrent_threads_permissions.as_ref().map(|sem| sem.available_permits() == self.max_concurrent_tasks).unwrap_or(true) {
                        break;
                    }
                    else {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }
            }
        }

        let _ = work_done_sender.send(true);
    }
}

impl BaseProvider for FCFSScheduler {
    fn base(&self) -> &SchedulerBase {
        &self.base
    }
}

#[async_trait::async_trait]
impl Scheduler for FCFSScheduler {
    async fn accept_process(&self, process: Arc<Process>) {
        let mut queue = self.queued_processes.lock().await;
        queue.push_back(process);
    }

    fn start_work(self: Arc<Self>,  work_done_sender: WorkDoneSender) {
        tokio::spawn(async move {
            self.run(work_done_sender).await;
        });
    }
}

// -----------------------------------------------
// TESTS
// -----------------------------------------------

#[tokio::test]
async fn test_fcfs_scheduler_single_thread() {
    use crate::process_scheduler_simulator::process::ProcessFactory;
    let mut factory = ProcessFactory::create(1, 1);
    let scheduler = FCFSScheduler::create(5, 1);
    
    let (w_tx, mut w_rx) = tokio::sync::watch::channel::<bool>(false);
    let _ = Scheduler::start_work(Arc::clone(&scheduler), w_tx.clone());

    assert!(*w_rx.borrow() == false);
    scheduler.accept_process(factory.create_process()).await;
    assert!(*w_rx.borrow() == false);
    assert!(scheduler.get_total_executed_processes_count() == 0);
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(*w_rx.borrow() == false);
    assert!(scheduler.get_total_executed_processes_count() == 1);
    scheduler.accept_process(factory.create_process()).await;
    scheduler.accept_process(factory.create_process()).await;
    scheduler.notify_no_more_processes();
    assert!(scheduler.get_total_executed_processes_count() == 1);
    assert!(w_rx.wait_for(|val| *val == true).await.is_ok());
    assert!(scheduler.get_total_executed_processes_count() == 3);
}

#[tokio::test]
async fn test_fcfs_scheduler_multiple_thread_1() {
    use crate::process_scheduler_simulator::process::ProcessFactory;
    let mut factory = ProcessFactory::create(2, 2);
    let scheduler = FCFSScheduler::create(4, 4);
    
    let (w_tx, w_rx) = tokio::sync::watch::channel::<bool>(false);
    let _ = Scheduler::start_work(Arc::clone(&scheduler), w_tx.clone());

    assert!(*w_rx.borrow() == false);
    scheduler.accept_process(factory.create_process()).await;
    scheduler.accept_process(factory.create_process()).await;
    scheduler.accept_process(factory.create_process()).await;
    scheduler.accept_process(factory.create_process()).await;
    scheduler.notify_no_more_processes();
    assert!(scheduler.get_total_executed_processes_count() == 0);
    assert!(*w_rx.borrow() == false);
    tokio::time::sleep(Duration::from_millis(2050)).await;
    assert!(*w_rx.borrow() == true);
    assert!(scheduler.get_total_executed_processes_count() == 4);
}

#[tokio::test]
async fn test_fcfs_scheduler_multiple_thread_2() {
    use crate::process_scheduler_simulator::process::ProcessFactory;
    let mut factory = ProcessFactory::create(2, 2);
    let scheduler = FCFSScheduler::create(7, 4);
    
    let (w_tx, w_rx) = tokio::sync::watch::channel::<bool>(false);
    let _ = Scheduler::start_work(Arc::clone(&scheduler), w_tx.clone());

    assert!(*w_rx.borrow() == false);
    scheduler.accept_process(factory.create_process()).await;
    scheduler.accept_process(factory.create_process()).await;
    scheduler.accept_process(factory.create_process()).await;
    scheduler.accept_process(factory.create_process()).await;
    scheduler.accept_process(factory.create_process()).await;
    scheduler.accept_process(factory.create_process()).await;
    scheduler.accept_process(factory.create_process()).await;
    scheduler.notify_no_more_processes();
    tokio::time::sleep(Duration::from_millis(2010)).await;
    assert!(*w_rx.borrow() == false);
    assert!(scheduler.get_total_executed_processes_count() == 4);
    tokio::time::sleep(Duration::from_millis(2010)).await;
    assert!(*w_rx.borrow() == true);
    assert!(scheduler.get_total_executed_processes_count() == 7);
}