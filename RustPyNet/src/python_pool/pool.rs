use lazy_static::lazy_static;
use pyo3::prelude::*;
use pyo3::types::IntoPyDict;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::mpsc::Sender;
use std::sync::Condvar;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use rand::distributions::Alphanumeric;
use rand::Rng;

use std::fmt::{self, format};

use crate::CLIENT_PYTHON_PROCESS_QUEUE;

macro_rules! acquire_python_queue {
    ($queue:expr) => {{
        let mut acquired = false;
        let mut q = None;
        while !acquired {
            match $queue.try_lock() {
                Ok(guard) => {
                    acquired = true;
                    q = Some(guard);
                }
                Err(_) => {
                    let sleep_duration =
                        std::time::Duration::from_millis(rand::random::<u64>() % 1000);
                    println!("Not being able to lock on Pool!");
                    std::thread::sleep(sleep_duration);
                }
            }
        }

        println!("Locked Python Queue in python pool");

        q.unwrap()
    }};
}

macro_rules! with_python_queue {
    ($queue:expr, $code:expr) => {{
        let mut acquired = false;
        let mut result: Option<_> = None; // Initialize result as None
        while !acquired {
            match $queue.try_lock() {
                Ok(mut guard) => {
                    acquired = true;
                    result = Some($code(&mut *guard));
                }
                Err(e) => {
                    let sleep_duration =
                        std::time::Duration::from_millis(rand::random::<u64>() % 100);
                    println!("Not being able to lock on Pool! Err {}", e);
                    std::thread::sleep(sleep_duration);
                }
            }
        }
        result.expect("Failed to acquire python queue and execute code")
    }};
}

// In rustpynet_traits

// lazy_static! {
//     pub static ref PYTHON_TASK_QUEUE: PythonTaskQueue = PythonTaskQueue::new();
// }

#[derive(Debug)]
pub enum PythonTaskError {
    PythonError(String),
    UnsupportedNumberType,
    UnsupportedValueType,
    OtherError(String),
    // Add other error variants as needed
}

#[derive(Clone, Debug)]
pub enum PythonTaskResult {
    Map(HashMap<String, PythonTaskResult>),
    List(Vec<PythonTaskResult>),
    Str(String),
    Int(i32),
    Float(f64),
    Bool(bool),
    None,
    Error(String),
}

impl fmt::Display for PythonTaskResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PythonTaskResult::None => write!(f, "None"),
            PythonTaskResult::Str(s) => write!(f, "\"{}\"", s),
            PythonTaskResult::Int(i) => write!(f, "{}", i),
            PythonTaskResult::Float(fl) => write!(f, "{}", fl),
            PythonTaskResult::Bool(b) => write!(f, "{}", b),
            PythonTaskResult::List(list) => {
                write!(f, "[")?;
                for (index, item) in list.iter().enumerate() {
                    if index != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            PythonTaskResult::Map(map) => {
                write!(f, "{{")?;
                let mut first = true;
                for (key, value) in map {
                    if !first {
                        write!(f, ", ")?;
                    }
                    write!(f, "\"{}\": {}", key, value)?;
                    first = false;
                }
                write!(f, "}}")
            }
            PythonTaskResult::Error(err) => write!(f, "Error: {}", err),
        }
    }
}

pub type MyResult<T> = Result<T, PythonTaskError>;

pub trait TaskQueue {
    // Define the methods and behaviors here
}

pub trait PythonTask {
    fn execute(
        &self,
        py: Python,
        tx: Sender<MyResult<PythonTaskResult>>,
    ) -> MyResult<PythonTaskResult>;
}

// In RustPyNet
impl TaskQueue for PythonTaskQueue {
    // Implement the methods here
}

pub struct PythonTaskQueue {
    tasks: Arc<
        Mutex<
            VecDeque<(
                Box<dyn PythonTask + Send>,
                std::sync::mpsc::Sender<MyResult<PythonTaskResult>>,
            )>,
        >,
    >,
}

impl PythonTaskQueue {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn enqueue(
        &self,
        task: Box<dyn PythonTask + Send>,
    ) -> std::sync::mpsc::Receiver<MyResult<PythonTaskResult>> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.tasks.lock().unwrap().push_back((task, tx));
        println!(
            "Task enqueued. Total tasks in queue: {}",
            self.tasks.lock().unwrap().len()
        );
        rx // Return the receiver
    }

    pub fn wait_for_result(
        rx: std::sync::mpsc::Receiver<MyResult<PythonTaskResult>>,
    ) -> MyResult<PythonTaskResult> {
        match rx.recv() {
            Ok(result) => match result {
                r => r,
            },
            Err(recv_error) => Err(PythonTaskError::OtherError(format!(
                "Failed to receive result from worker thread due to: {}.",
                recv_error
            ))),
        }
    }
}

pub fn start_processing_host_python_tasks() {
    println!("Start processing python calls!");

    loop {
        // Acquire the Python task queue.
        let tasks_clone = with_python_queue!(
            CLIENT_PYTHON_PROCESS_QUEUE,
            |python_queue: &mut PythonTaskQueue| { python_queue.tasks.clone() }
        );

        // Check the number of tasks in the queue.
        let num_tasks = tasks_clone.lock().unwrap().len();
        if num_tasks > 0 {
            println!("Number of tasks in queue: {}", num_tasks);

            // Acquire the GIL and execute the Python tasks.
            let gil_guard = Python::acquire_gil();
            let py = gil_guard.python();

            while let Some((task, tx)) = tasks_clone.lock().unwrap().pop_front() {
                println!("Executing a task from the queue...");
                match task.execute(py, tx) {
                    Ok(_) => println!("Task successfully executed."),
                    Err(e) => println!("Error executing task: {:?}", e),
                }

                println!("Task executed.");
            }
        } else {
            // If no tasks, sleep for a short duration before checking again.
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}
