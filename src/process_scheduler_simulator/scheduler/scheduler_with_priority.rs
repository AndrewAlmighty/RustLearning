use crate::process_scheduler_simulator::process::{Process, Prioritable, ByRemainingTime, ByPriority, ID, InterruptSender, Event, EventSender, EventReceiver};
use crate::process_scheduler_simulator::scheduler::{BaseProvider, SchedulerBase, Scheduler};
use crate::process_scheduler_simulator::WorkDoneSender;

use std::collections::BinaryHeap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::{Mutex, broadcast, mpsc};
use tokio::time::Duration;

pub type PriorityScheduler = SchedulerWithPriority<ByPriority>;
pub type SRTFScheduler = SchedulerWithPriority<ByRemainingTime>;

#[derive(Clone)]
struct RunningProcess<T: Prioritable + Ord + 'static> {
    id: ID,
    process: T,
    interrupter: InterruptSender
}

// Priority can be remaining time or priority
pub(super) struct SchedulerWithPriority<T: Prioritable + Ord + 'static> {
    queued_processes: Arc<Mutex<BinaryHeap<T>>>,
    running_processes: Mutex<Vec<RunningProcess<T>>>,
    base: SchedulerBase,
    event_receiver: Mutex<EventReceiver>,
    event_sender: EventSender,
    new_process_arrived: AtomicBool
}

impl<T: Prioritable + Ord + 'static> SchedulerWithPriority<T> {
    pub(super) fn create(queue_processes_size: usize, threads_count: usize) -> Arc<Self> {
        let (sender, receiver) = mpsc::channel::<Event>(threads_count);
        Arc::new(SchedulerWithPriority {
            queued_processes: Arc::new(Mutex::new(BinaryHeap::with_capacity(queue_processes_size))),
            running_processes: Mutex::new(Vec::with_capacity(threads_count)),
            base: SchedulerBase::create(),
            event_sender: sender,
            event_receiver: Mutex::new(receiver),
            new_process_arrived: AtomicBool::new(false)
        })
    }
    
    async fn run(&self, work_done_sender: WorkDoneSender) {
        let mut running_processes = self.running_processes.lock().await;
        let mut something_happened = false;
        loop {
            {
                let mut event_receiver = self.event_receiver.lock().await;
                while let Ok(event) = event_receiver.try_recv() {
                    if !something_happened { something_happened = true; }
                    if let Event::Finished(id) = event {
                        if let Some(index) = running_processes.iter().position(|p| p.id == id) {
                            running_processes.remove(index);
                            self.base.executed_processes_counter.fetch_add(1, Ordering::Relaxed);
                        } 
                    }
                }
            }

            let mut queue = self.queued_processes.lock().await;

            if queue.is_empty() && running_processes.is_empty() && self.base.process_generation_finished.load(Ordering::Relaxed) {
                break;
            }
            
            let number_of_running_processes = running_processes.len();
            let mut available_slots = running_processes.capacity() - number_of_running_processes;

            if self.new_process_arrived.load(Ordering::Relaxed) {
                self.new_process_arrived.store(false, Ordering::Relaxed);
                if available_slots == 0 {
                    loop {
                        let first_in_queue_priority = queue.peek().unwrap().priority_value();
                        if <T as Prioritable>::compare_priority_values(first_in_queue_priority, running_processes.last().unwrap().process.priority_value()) {
                            if !something_happened { something_happened = true; }
                            let mut position:Option<usize> = None;
                            for i in 0..number_of_running_processes {
                                if position.is_none() && <T as Prioritable>::compare_priority_values(first_in_queue_priority, running_processes[i].process.priority_value()) {
                                    position = Some(i);
                                }

                                if position.is_some() {
                                    let _ = running_processes[i].interrupter.send(());
                                }
                            }

                            queue.push(running_processes.remove(number_of_running_processes - 1).process);
                            let place_to_push_process = position.unwrap();
                            
                            let process = queue.pop().unwrap();
                            let event_sender = self.event_sender.clone();
                            let (interrupt_sender, interrupt_receiver) = broadcast::channel(1);
                            let process_clone = Arc::clone(process.get_process());
                            tokio::spawn(async move {
                                process_clone.run(Some(interrupt_receiver), Some(event_sender), None).await;
                            });

                            running_processes.insert(place_to_push_process, RunningProcess{ id: process.get_process().get_id(), process: process, interrupter: interrupt_sender });
                            for i in place_to_push_process+1..number_of_running_processes {
                                let (interrupt_sender, interrupt_receiver) = broadcast::channel(1);
                                let event_sender = self.event_sender.clone();
                                running_processes[i].interrupter = interrupt_sender;
                                let process_clone = Arc::clone(running_processes[i].process.get_process());

                                tokio::spawn(async move {
                                    process_clone.run(Some(interrupt_receiver), Some(event_sender), None).await;
                                });
                            }
                        }
                        else { break; }
                    }
                }
            }

            while available_slots > 0 && !queue.is_empty() {
                if !something_happened { something_happened = true; }
                let process = queue.pop().unwrap();
                let (interrupt_sender, interrupt_receiver) = broadcast::channel(1);
                let event_sender = self.event_sender.clone();                    
                let process_clone = Arc::clone(process.get_process());

                tokio::spawn(async move {
                    process_clone.run(Some(interrupt_receiver), Some(event_sender), None).await;
                });

                let process_priority = process.priority_value();
                let mut position_to_insert: Option<usize> = None;
                for i in 0..number_of_running_processes {
                    if <T as Prioritable>::compare_priority_values(process_priority, running_processes[i].process.priority_value()) {
                        position_to_insert = Some(i);
                        break;
                    }
                }

                if let Some(position) = position_to_insert {
                    running_processes.insert(position, RunningProcess{ id: process.get_process().get_id(), process: process, interrupter: interrupt_sender });
                }
                else {
                    running_processes.push(RunningProcess{ id: process.get_process().get_id(), process: process, interrupter: interrupt_sender });
                }

                available_slots -= 1;
            }

            if !something_happened { tokio::time::sleep(Duration::from_millis(10)).await; }
            else { something_happened = false; }
        }
        let _ = work_done_sender.send(true);
    }
}

impl<T: Prioritable + Ord + 'static> BaseProvider for SchedulerWithPriority<T> {
    fn base(&self) -> &SchedulerBase {
        &self.base
    }
}

#[async_trait::async_trait]
impl<T: Prioritable + Ord + 'static> Scheduler for SchedulerWithPriority<T> {
    async fn accept_process(&self, process: Arc<Process>) {
        let mut queue = self.queued_processes.lock().await;
        queue.push(T::create_from_process(process));
        if !self.new_process_arrived.load(Ordering::Relaxed) {
            self.new_process_arrived.store(true, Ordering::Relaxed);
        }
    }

    fn start_work(self: Arc<Self>, work_done_sender: WorkDoneSender) {
        tokio::spawn(async move {
            self.run(work_done_sender).await;
        });
    }
}

// -----------------------------------------------
// TESTS
// -----------------------------------------------

#[tokio::test]
async fn test_priority_single_thread() {
    use crate::process_scheduler_simulator::process::ProcessFactory;
    use crate::process_scheduler_simulator::process::State;
    use std::collections::BTreeMap;

    let processes_number = 10;
    let mut factory = ProcessFactory::create(1, 3);
    let scheduler = PriorityScheduler::create(processes_number, 1);

    let mut processes_by_priority = BTreeMap::<u8, Vec<Arc<Process>>>::new();
    let (work_done_sender, work_done_receiver) = tokio::sync::watch::channel(false);
    Scheduler::start_work(scheduler.clone(), work_done_sender);

    for _ in 0..processes_number {
        let process = factory.create_process();
        let priority = process.get_priority();
        processes_by_priority.entry(priority).or_insert_with(Vec::new).push(Arc::clone(&process));
        scheduler.accept_process(process).await;
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    scheduler.notify_no_more_processes();

    while *work_done_receiver.borrow() == false {
        let mut previous_row_finished = true;
        for (_, processes) in processes_by_priority.iter().rev() {
            for p in processes {
                if !previous_row_finished {
                    assert_ne!(p.get_state(), State::Finished);
                }
                else if p.get_state() != State::Finished {
                    previous_row_finished = false;
                    break;
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}


#[tokio::test]
async fn test_priority_multiple_threads() {
    use crate::process_scheduler_simulator::process::ProcessFactory;
    use crate::process_scheduler_simulator::process::State;
    use std::collections::BTreeMap;

    let processes_number = 30;
    let mut factory = ProcessFactory::create(3, 6);
    let scheduler = PriorityScheduler::create(processes_number, 6);

    let mut processes_by_priority = BTreeMap::<u8, Vec<Arc<Process>>>::new();
    let (work_done_sender, work_done_receiver) = tokio::sync::watch::channel(false);

    Scheduler::start_work(scheduler.clone(), work_done_sender);

    for _ in 0..processes_number {
        let process = factory.create_process();
        let priority = process.get_priority();
        processes_by_priority.entry(priority).or_insert_with(Vec::new).push(Arc::clone(&process));
        scheduler.accept_process(process).await;
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    scheduler.notify_no_more_processes();
    tokio::time::sleep(Duration::from_millis(500)).await;
    while *work_done_receiver.borrow() == false {
        let first_priority_value_in_queue =
            if let Some(process) = scheduler.queued_processes.lock().await.peek() {
                Some(process.get_process().get_priority())
            }
            else { None };

        let mut previous_row_finished = true;
        for (_, processes) in processes_by_priority.iter().rev() {
            for p in processes {
                if !previous_row_finished && first_priority_value_in_queue.is_some() && first_priority_value_in_queue.unwrap() > p.get_priority()  {
                    assert_ne!(p.get_state(), State::Finished);
                }
                else if p.get_state() != State::Finished {
                    previous_row_finished = false;
                    break;
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}


#[tokio::test]
async fn test_srtf_single_thread() {
    use crate::process_scheduler_simulator::process::ProcessFactory;
    use crate::process_scheduler_simulator::process::State;
    use crate::process_scheduler_simulator::types::Milliseconds;
    use std::collections::BTreeMap;

    let processes_number = 10;
    let mut factory = ProcessFactory::create(1, 5);
    let scheduler = SRTFScheduler::create(processes_number, 1);

    let mut processes_by_remaining_time = BTreeMap::<Milliseconds, Vec<Arc<Process>>>::new();
    let (work_done_sender, work_done_receiver) = tokio::sync::watch::channel(false);
    Scheduler::start_work(scheduler.clone(), work_done_sender);

    for _ in 0..processes_number {
        let process = factory.create_process();
        let remaining_time = process.get_remaining_time();
        processes_by_remaining_time.entry(remaining_time).or_insert_with(Vec::new).push(Arc::clone(&process));
        scheduler.accept_process(process).await;
    }

    scheduler.notify_no_more_processes();

    while *work_done_receiver.borrow() == false {
        let mut previous_row_finished = true;
        for (_, processes) in &processes_by_remaining_time {
            for p in processes {
                if !previous_row_finished {
                    assert_ne!(p.get_state(), State::Finished);
                }
                else if p.get_state() != State::Finished {
                    previous_row_finished = false;
                    break;
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[tokio::test]
async fn test_srtf_multiple_threads() {
    use crate::process_scheduler_simulator::process::ProcessFactory;
    use crate::process_scheduler_simulator::process::State;
    use crate::process_scheduler_simulator::types::Milliseconds;
    use std::collections::BTreeMap;

    let processes_number = 30;
    let mut factory = ProcessFactory::create(1, 10);
    let scheduler = SRTFScheduler::create(processes_number, 6);

    let mut processes_by_remaining_time = BTreeMap::<Milliseconds, Vec<Arc<Process>>>::new();
    let (work_done_sender, work_done_receiver) = tokio::sync::watch::channel(false);
    Scheduler::start_work(scheduler.clone(), work_done_sender);

    for _ in 0..processes_number {
        let process = factory.create_process();
        let remaining_time = process.get_remaining_time();
        processes_by_remaining_time.entry(remaining_time).or_insert_with(Vec::new).push(Arc::clone(&process));
        scheduler.accept_process(process).await;
        tokio::time::sleep(Duration::from_millis(30)).await;
    }

    scheduler.notify_no_more_processes();

    while *work_done_receiver.borrow() == false {
        let mut previous_row_finished = true;
        for (_, processes) in &processes_by_remaining_time {
            for p in processes {
                if !previous_row_finished {
                    assert_ne!(p.get_state(), State::Finished);
                }
                else if p.get_state() != State::Finished {
                    previous_row_finished = false;
                    break;
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}