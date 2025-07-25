use crate::process_scheduler_simulator::types::{Milliseconds, Seconds};

use std::cmp::Ordering;
use std::sync::Arc;

use chrono::{DateTime, Local, Utc};

use rand::{rng, Rng};
use rand::prelude::ThreadRng;

use tokio::time::Duration;

pub(super) type ID = usize;
pub(super) type InterruptReceiver = tokio::sync::broadcast::Receiver<()>;
pub(super) type InterruptSender = tokio::sync::broadcast::Sender<()>;
pub(super) type EventSender = tokio::sync::mpsc::Sender<Event>;
pub(super) type EventReceiver = tokio::sync::mpsc::Receiver<Event>;

type MemoryOrder = std::sync::atomic::Ordering;
type AtomicMilliseconds = std::sync::atomic::AtomicU32;
type AtomicEpoch = std::sync::atomic::AtomicU64;
type AtomicState = std::sync::atomic::AtomicU8;

#[derive(Copy, Clone, PartialEq, Debug)]
#[repr(u8)]
pub(super) enum State {
    Idle,
    Running,
    Finished,
}

impl From<State> for u8 {
    fn from(state: State) -> u8 {
        match state {
            State::Idle => 0,
            State::Running => 1,
            State::Finished => 2
        }
    }

}

impl From<u8> for State {
    fn from(state: u8) -> State {
        match state {
            0 => State::Idle,
            1 => State::Running,
            2 => State::Finished,
            n => { panic!("There should be no such state value: {}", n); }
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
#[repr(u8)]
pub(super) enum Event {
    Finished(ID),
    Interrupted(ID)
}

pub(super) struct Process {
    creation_time: DateTime<Local>,
    id: ID,
    remaining_time: AtomicMilliseconds,
    execution_begin_time: AtomicEpoch,
    state: AtomicState,
    priority: u8
}

pub(super) struct ProcessFactory {
    processes_counter : usize,
    burst_range: (Seconds, Seconds),
    rng_th: ThreadRng
}

impl Process{
    pub(super) fn get_id(&self) -> ID {
        self.id
    }

    #[cfg(test)]
    pub(super) fn get_state(&self) -> State {
        State::from(self.state.load(MemoryOrder::Relaxed))
    }

    fn calculate_remaining_time(&self) -> Milliseconds {
        let remaining = self.remaining_time.load(MemoryOrder::Relaxed);
        let elapsed_time = (Utc::now().timestamp_millis() - self.execution_begin_time.load(MemoryOrder::Relaxed) as i64) as Milliseconds;
        if remaining < elapsed_time {
            0
        }
        else {
            remaining - elapsed_time
        }
    }

    pub(super) fn get_priority(&self) -> u8 {
        self.priority
    }

    pub(super) fn get_remaining_time(&self) -> Milliseconds {
        if self.execution_begin_time.load(MemoryOrder::Relaxed) != 0 {
           self.calculate_remaining_time()
        }
        else {
            self.remaining_time.load(MemoryOrder::Relaxed)
        }
    }

    async fn handle_execution_outcome(&self, sender: &Option<EventSender>) {
        let remaining_time = self.calculate_remaining_time();
        self.execution_begin_time.store(0, MemoryOrder::Relaxed);
        if remaining_time == 0 {
            self.remaining_time.store(0, MemoryOrder::Relaxed);
            self.state.store(u8::from(State::Finished), MemoryOrder::Relaxed);
            let now = Local::now();
            println!("{} - Process with ID: {} finished. Total execution time: {} second(s)", now.to_rfc3339(), self.id, (now - self.creation_time).num_seconds());
            if let Some(sender) = sender {
                let _ = sender.send(Event::Finished(self.id)).await;
            }
        }
        else {
            self.remaining_time.store(remaining_time, MemoryOrder::Relaxed);
            println!("{} - Execution of process with ID: {} is stopped! Remaining time: {} ms", Local::now().to_rfc3339(), self.id, remaining_time);
            self.state.store(u8::from(State::Idle), MemoryOrder::Relaxed);
            if let Some(sender) = sender {
                let _ = sender.send(Event::Interrupted(self.id)).await;
            }
        }
    }

    pub async fn run(&self, interrupt: Option<InterruptReceiver>, sender: Option<EventSender>, time_chunk: Option<Milliseconds>) {
        let remaining_time = self.remaining_time.load(MemoryOrder::Relaxed);
        if remaining_time == 0 && State::from(self.state.load(MemoryOrder::Relaxed)) != State::Finished {
            self.state.store(u8::from(State::Finished), MemoryOrder::Relaxed);
            let now = Local::now();
            println!("{} - Process with ID: {} was already finished when picked to execute. Total execution time: {} second(s)", now.to_rfc3339(), self.id, (now - self.creation_time).num_seconds());
            if let Some(sender) = sender {
                let _ = sender.send(Event::Finished(self.id)).await;
            }
        }
        else {
            println!("{} - Process with ID: {} is running! Priority: {}, Remainig time: {} ms", Local::now().to_rfc3339(), self.id, self.priority, remaining_time);
            let max_exec_time = time_chunk.unwrap_or(remaining_time);
            self.execution_begin_time.store(Local::now().timestamp_millis().try_into().unwrap(), MemoryOrder::Relaxed);
            self.state.store(u8::from(State::Running), MemoryOrder::Relaxed);

            if let Some(mut interrupt) = interrupt {
                tokio::select!(
                    _ = tokio::time::sleep(Duration::from_millis(max_exec_time.into())) => {
                        self.handle_execution_outcome(&sender).await;
                    }
                    recv_result = interrupt.recv() => {
                        match recv_result {
                            Ok(_) => {
                                println!("{} - Execution of process with ID: {} is interrupted!", Local::now().to_rfc3339(), self.id);
                                self.handle_execution_outcome(&sender).await;
                            }
                            _ => {}
                        }
                    }
                )
            }
            else {
                tokio::time::sleep(Duration::from_millis(max_exec_time.into())).await;
                self.handle_execution_outcome(&sender).await;
            }
        }
    }
}

impl ProcessFactory {
    pub(super) fn create(burst_min: Seconds, burst_max: Seconds) -> Self {
        assert!(burst_min != 0);
        assert!(burst_min <= burst_max);

        ProcessFactory {
            processes_counter: 0,
            burst_range: (burst_min, burst_max),
            rng_th: rng()
        }
    }

    pub(super) fn create_process(&mut self) -> Arc<Process> {
        self.processes_counter += 1;
        let duration: Seconds = self.rng_th.random_range(self.burst_range.0 ..= self.burst_range.1);
        let now = Local::now();
        let priority = self.rng_th.random_range(1..=10);
        println!("{} - Creating process with ID: {}, priority: {}, burst_time: {} seconds", now.to_rfc3339(), self.processes_counter, priority, duration);

        Arc::new(Process { 
            id: self.processes_counter,
            creation_time: now,
            remaining_time: AtomicMilliseconds::new(duration as Milliseconds * 1000),
            execution_begin_time: AtomicEpoch::new(0),
            priority: priority,
            state: AtomicState::new(u8::from(State::Idle))
        })
    }
}

pub(super) struct ByRemainingTime(pub(super) Arc<Process>);
pub(super) struct ByPriority(pub(super) Arc<Process>);

pub(super) trait Prioritable: Send + Sync {
    fn create_from_process(process: Arc<Process>) -> Self;
    fn compare_priority_values(first_priority_from_queue: Milliseconds, last_priority_from_running_processes: Milliseconds) -> bool;
    fn priority_value(&self) -> Milliseconds;
    fn get_process(&self) -> &Arc<Process>;
}

impl Prioritable for ByRemainingTime {
    fn priority_value(&self) -> Milliseconds {
        self.0.get_remaining_time()
    }

    fn compare_priority_values(first_priority_from_queue: Milliseconds, last_priority_from_running_processes: Milliseconds) -> bool {
       first_priority_from_queue < last_priority_from_running_processes
    }

    fn create_from_process(process: Arc<Process>) -> Self {
        ByRemainingTime(process)
    }

    fn get_process(&self) -> &Arc<Process> { &self.0 }
}

impl Prioritable for ByPriority {
    fn priority_value(&self) -> Milliseconds {
        self.0.get_priority().into()
    }

    fn compare_priority_values(first_priority_from_queue: Milliseconds, last_priority_from_running_processes: Milliseconds) -> bool {
       first_priority_from_queue > last_priority_from_running_processes
    }

    fn create_from_process(process: Arc<Process>) -> Self {
        ByPriority(process)
    }

    fn get_process(&self) -> &Arc<Process> { &self.0 }
}

impl PartialEq for ByRemainingTime {
    fn eq(&self, other: &Self) -> bool {
        other.0.remaining_time.load(MemoryOrder::Relaxed).eq(&self.0.remaining_time.load(MemoryOrder::Relaxed))
    }
    fn ne(&self, other: &Self) -> bool {
        other.0.remaining_time.load(MemoryOrder::Relaxed).ne(&self.0.remaining_time.load(MemoryOrder::Relaxed))
    }
}

impl PartialEq for ByPriority {
    fn eq(&self, other: &Self) -> bool {
        other.0.priority.eq(&self.0.priority)
    }
    fn ne(&self, other: &Self) -> bool {
        other.0.priority.ne(&self.0.priority)
    }
}

impl Eq for ByRemainingTime {}
impl Eq for ByPriority {}

impl PartialOrd for ByRemainingTime {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.0.remaining_time.load(MemoryOrder::Relaxed).partial_cmp(&self.0.remaining_time.load(MemoryOrder::Relaxed))
        
    }
}

impl PartialOrd for ByPriority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let order = other.0.priority.partial_cmp(&self.0.priority);
        match order {
            Some(o) => Some(o.reverse()),
            None => None
        }
    }
}

impl Ord for ByRemainingTime {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.remaining_time.load(MemoryOrder::Relaxed).cmp(&self.0.remaining_time.load(MemoryOrder::Relaxed))
    }
}

impl Ord for ByPriority {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.priority.cmp(&self.0.priority).reverse()
    }
}

// -----------------------------------------------
// TESTS
// -----------------------------------------------

#[tokio::test]
async fn test_process_module() {
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let mut factory = ProcessFactory::create(5, 5);
    let process_1 = Arc::new(Mutex::new(factory.create_process()));
    {
        let process_2 = factory.create_process();
        assert!(process_2.id == 2);
    }

    let (e_tx, mut e_rx) = tokio::sync::mpsc::channel::<Event>(10);
    {
        let mut process = process_1.lock().await;
        assert!(process.id == 1);
        assert!(process.remaining_time.load(MemoryOrder::Relaxed) == 5000);
        assert!(State::from(process.state.load(MemoryOrder::Relaxed)) == State::Idle);
        
        let mutable_process = Arc::get_mut(&mut process).unwrap();
        mutable_process.run(None, Some(e_tx.clone()), Some(1000)).await;
        assert_eq!(State::from(process.state.load(MemoryOrder::Relaxed)) , State::Idle);
        assert!((3990..=4000).contains(&process.remaining_time.load(MemoryOrder::Relaxed) ));
    }
    {
        let (b_tx, b_rx) = tokio::sync::broadcast::channel(1);
        let process_clone = process_1.clone();
        let e_tx_clone = e_tx.clone();

        tokio::spawn(async move {
            let mut proc = process_clone.lock().await;
            let mut_proc = Arc::get_mut(&mut proc).unwrap();
            mut_proc.run(Some(b_rx), Some(e_tx_clone), Some(3000)).await;
        });

        tokio::time::sleep(Duration::from_secs(1)).await;
        let _ = b_tx.send(());
        let process = process_1.lock().await;

        assert!(State::from(process.state.load(MemoryOrder::Relaxed))  == State::Idle);
        assert!((2990..=3000).contains(&process.remaining_time.load(MemoryOrder::Relaxed) ));
    }
    {
        let (_b_tx, b_rx) = tokio::sync::broadcast::channel(1);
        let mut process = process_1.lock().await;
        let mutable_process = Arc::get_mut(&mut process).unwrap();
        mutable_process.run(Some(b_rx), Some(e_tx.clone()), None).await;
        assert!(State::from(process.state.load(MemoryOrder::Relaxed)) == State::Finished);
        assert!(process.remaining_time.load(MemoryOrder::Relaxed)  == 0);
    }
    assert!(e_rx.recv().await == Some(Event::Interrupted(1)));
    assert!(e_rx.recv().await == Some(Event::Interrupted(1)));
    assert!(e_rx.recv().await == Some(Event::Finished(1)));
}

#[test]
#[should_panic]
fn test_panic_burst_min_is_zero() {
    let _ = ProcessFactory::create(0, 2);
}

#[test]
#[should_panic]
fn test_panic_burst_max_is_smaller_than_min() {
    let _ = ProcessFactory::create(4, 2);
}

#[test]
fn test_process_ordering() {
    use std::collections::BinaryHeap;
    let mut factory = ProcessFactory::create(1, 50);
    {
        let mut heap = BinaryHeap::<ByRemainingTime>::with_capacity(500);
        for _ in 0..500 {
            heap.push(ByRemainingTime(factory.create_process()));
        }
        let mut last_remaining_time = 0 as Milliseconds;
        for _ in 0..500 {
            let process = heap.pop().unwrap();
            let next_remaining_time = process.0.remaining_time.load(MemoryOrder::Relaxed);
            assert!(last_remaining_time <= next_remaining_time);
            last_remaining_time = next_remaining_time;
        }
    }
    {
        let mut heap = BinaryHeap::<ByPriority>::new();
        for _ in 0..500 {
            heap.push(ByPriority(factory.create_process()));
        }
        let mut last_priority = 255u8;
        for _ in 0..500 {
            let process = heap.pop().unwrap();
            assert!(last_priority >= process.0.priority);
            last_priority = process.0.priority;
        }
    }
}