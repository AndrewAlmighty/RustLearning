use crate::process_scheduler_simulator::process::{Process, ID, EventSender, EventReceiver, Event, State};
use crate::process_scheduler_simulator::scheduler::{BaseProvider, SchedulerBase, Scheduler};
use crate::process_scheduler_simulator::WorkDoneSender;
use crate::process_scheduler_simulator::types::{Seconds, Milliseconds};

use std::collections::{VecDeque, HashMap};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use tokio::sync::{Mutex, mpsc};
use tokio::time::Duration;

pub(super) struct RoundRobinScheduler {
    queued_processes: Arc<Mutex<VecDeque<Arc<Process>>>>,
    running_processes: Mutex<HashMap<ID, Arc<Process>>>,
    event_receiver: Mutex<EventReceiver>,
    event_sender: EventSender,
    base: SchedulerBase,
    threads_count: usize,
    interval: Milliseconds
    

}

impl RoundRobinScheduler {
    pub(super) fn create(queue_processes_size: usize, threads_count: usize, interval: Seconds) -> Arc<Self> {
        let (sender, receiver) = mpsc::channel::<Event>(threads_count);
        Arc::new(RoundRobinScheduler {
            queued_processes: Arc::new(Mutex::new(VecDeque::with_capacity(queue_processes_size))),
            running_processes: Mutex::new(HashMap::with_capacity(threads_count)),
            event_sender: sender,
            event_receiver: Mutex::new(receiver),
            base: SchedulerBase::create(),
            interval: (interval as u32) * 1000,
            threads_count: threads_count
        })
    }

    async fn run(&self, work_done_sender: WorkDoneSender) {
        let mut running_processes = self.running_processes.lock().await;
        let mut something_happened = false;
        let interval = self.interval;

        loop {
            let mut queue = self.queued_processes.lock().await;
            {
                let mut event_receiver = self.event_receiver.lock().await;
                while let Ok(event) = event_receiver.try_recv() {
                    if !something_happened { something_happened = true; }
                    match event {
                        Event::Finished(id) => {
                            let _ = running_processes.remove(&id);
                        },
                        Event::Interrupted(id) => {
                            let process = running_processes.remove(&id).expect("Scenario when in hasmap there is no process which send us interrupt is not acceptable");
                            queue.push_back(process);
                        }
                    }
                }
            }

            if queue.is_empty() && running_processes.is_empty() && self.base.process_generation_finished.load(Ordering::Relaxed) {
                break;
            }

            let number_of_running_processes = running_processes.len();
            let mut available_slots = self.threads_count - number_of_running_processes;

            while available_slots > 0 && !queue.is_empty() {
                if !something_happened { something_happened = true; }

                let process = queue.pop_front().unwrap();
                let event_sender = self.event_sender.clone();           
                let process_clone = Arc::clone(&process);

                tokio::spawn(async move {
                    process_clone.run(None, Some(event_sender), Some(interval)).await;
                });

                assert!(running_processes.insert(process.get_id(), process).is_none());
                available_slots -= 1;
            }

            if !something_happened { tokio::time::sleep(Duration::from_millis(10)).await; }
            else { something_happened = false; }
        }

        let _ = work_done_sender.send(true);
    }
}

impl BaseProvider for RoundRobinScheduler {
    fn base(&self) -> &SchedulerBase {
        &self.base
    }
}

#[async_trait::async_trait]
impl Scheduler for RoundRobinScheduler {
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

#[tokio::test]
async fn test_rr_single_thread() {
    use crate::process_scheduler_simulator::process::ProcessFactory;
    let processes_count = 3;
    let time_chunk: Seconds = 1;
    let mut factory = ProcessFactory::create(3, 3);
    let scheduler = RoundRobinScheduler::create(processes_count, 1, time_chunk);
    let mut processes = Vec::with_capacity(processes_count);

    for _ in 0..processes_count {
        let process = factory.create_process();
        processes.push(Arc::clone(&process));
        scheduler.accept_process(process).await;
    }

    let (work_done_sender, _) = tokio::sync::watch::channel(false);
    Scheduler::start_work(scheduler.clone(), work_done_sender);
    tokio::time::sleep(Duration::from_millis(100)).await;
    scheduler.notify_no_more_processes();
    let mut idx_with_running_process = 0;
    let mut running_iteration = 1;
    while running_iteration <= processes_count {
        for j in 0..processes_count {
            if running_iteration < processes_count {
                if idx_with_running_process == j {
                    assert_eq!(processes[j].get_state(), State::Running);
                }
                else {
                    assert_eq!(processes[j].get_state(), State::Idle);
                }
            }
            else {
                if idx_with_running_process > j {
                    assert_eq!(processes[j].get_state(), State::Finished);
                }
                else if idx_with_running_process == j {
                    assert_eq!(processes[j].get_state(), State::Running);
                }
                else {
                    assert_eq!(processes[j].get_state(), State::Idle);
                }
            }
        }

        idx_with_running_process += 1;
        if idx_with_running_process >= processes_count {
            idx_with_running_process = 0;
            running_iteration += 1;
        }

        tokio::time::sleep(Duration::from_secs(time_chunk.into())).await;
    }
}
