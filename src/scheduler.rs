use std::{collections::{BinaryHeap, HashMap}, sync::mpsc::SyncSender, thread::JoinHandle, time::Duration};

use chrono::{DateTime, Local, Timelike};

use crate::{Event, event_queue::EventSender, log};



const START_TIME: u32 = 9;
const END_TIME: u32 = 23;



pub enum SchedulerCommand {
    AddTask(SchedulerTask),
    DeleteTask(String),
    Next,
    Stop
}

unsafe impl Sync for SchedulerCommand {}
unsafe impl Send for SchedulerCommand {}



#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct SchedulerTask {
    pub name: String,
    period: Duration
}

impl SchedulerTask {
    pub fn new<S: ToString>(name: S, period: u64) -> SchedulerTask {
        Self {
            name: name.to_string(),
            period: Duration::from_secs(period)
        }
    }
}



#[derive(PartialEq, Eq)]
pub struct SchedulerEntity {
    trigger_time: DateTime<Local>,
    task: SchedulerTask
}


impl Ord for SchedulerEntity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.trigger_time.cmp(&self.trigger_time)
    }
}

impl PartialOrd for SchedulerEntity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}


pub struct SchedulerCore {
    tasks: HashMap<String, SchedulerTask>,
    entities: BinaryHeap<SchedulerEntity>
}


impl SchedulerCore {
    pub fn new() -> SchedulerCore {
        Self {
            tasks: HashMap::new(),
            entities: BinaryHeap::new()
        }
    }

    pub fn add_task(&mut self, task: SchedulerTask) {
        let now = Local::now();
        let task_name = task.name.clone();
        let mut trigger_time = now + task.period;

        if trigger_time.hour() < START_TIME {
            trigger_time = trigger_time
                .date_naive()
                .and_hms_opt(START_TIME, 0, 0)
                .unwrap()
                .and_local_timezone(Local)
                .unwrap();
        }
        if trigger_time.hour() >= END_TIME {
            trigger_time = (trigger_time + chrono::Duration::days(1))
                .date_naive()
                .and_hms_opt(START_TIME, 0, 0)
                .unwrap()
                .and_local_timezone(Local)
                .unwrap();
        }

        let entity = SchedulerEntity {
            trigger_time,
            task: task.clone()
        };

        self.tasks.insert(task_name, task);
        self.entities.push(entity);
    }

    pub fn remove_task(&mut self, name: &str) {
        self.tasks.remove(name);
    }

    pub fn next(&mut self) -> Option<String> {
        if let Some(entity) = self.entities.peek() {
            let now = Local::now();

            if entity.trigger_time <= now {
                let entity = self.entities.pop().unwrap();
                let task_name = entity.task.name.clone();

                if let Some(task) = self.tasks.get(&task_name) {
                    self.add_task(task.clone());
                }

                return Some(task_name)
            }
        }

        None
    }
}



pub struct SchedulerCommander {
    sender: SyncSender<SchedulerCommand>
}

impl SchedulerCommander {
    pub fn add_task(&self, task: SchedulerTask) {
        let _ = self.sender.send(SchedulerCommand::AddTask(task));
    }

    pub fn delete_task(&self, task: String) {
        let _ = self.sender.send(SchedulerCommand::DeleteTask(task));
    }

    pub fn next(&self) {
        let _ = self.sender.send(SchedulerCommand::Next);
    }

    pub fn stop(&self) {
        let _ = self.sender.send(SchedulerCommand::Stop);
    }
}



pub struct Scheduler {
    thread: Option<JoinHandle<()>>,
    sender: SyncSender<SchedulerCommand>
}

impl Scheduler {
    pub fn new(tasks: Vec<SchedulerTask>, event_queue: EventSender) -> Scheduler {
        let mut scheduler = SchedulerCore::new();

        for task in tasks {
            scheduler.add_task(task)
        }

        let (sender, receiver) = std::sync::mpsc::sync_channel::<SchedulerCommand>(64);

        let thread = std::thread::Builder::new().name("SchedulerProcess".to_string()).spawn(move || {
            log!("The scheduler process started");

            while let Ok(command) = receiver.recv() {
                match command {
                    SchedulerCommand::AddTask(task) => scheduler.add_task(task),

                    SchedulerCommand::DeleteTask(task) => scheduler.remove_task(&task),

                    SchedulerCommand::Next =>
                        if let Some(task) = scheduler.next() {
                            let _ = event_queue.send(Event::PlayTrack(task));
                        }
                        else {
                            let _ = event_queue.send(Event::None);
                        }

                    SchedulerCommand::Stop => break
                }
            }

            log!("The scheduler process stopped")
        }).unwrap();


        Self {
            thread: Some(thread),
            sender
        }
    }


    pub fn commander(&self) -> SchedulerCommander {
        SchedulerCommander {
            sender: self.sender.clone()
        }
    }
}


impl Drop for Scheduler {
    fn drop(&mut self) {
        self.commander().stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}