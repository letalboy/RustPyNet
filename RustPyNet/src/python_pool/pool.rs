use pyo3::prelude::*;
use pyo3::types::IntoPyDict;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Condvar;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use lazy_static::lazy_static;

use rand::distributions::Alphanumeric;
use rand::Rng;

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
                Err(_) => {
                    let sleep_duration =
                        std::time::Duration::from_millis(rand::random::<u64>() % 1000);
                    println!("Not being able to lock on Pool!");
                    std::thread::sleep(sleep_duration);
                }
            }
        }
        result.expect("Failed to acquire python queue and execute code")
    }};
}

// In rustpynet_traits
pub trait TaskQueue {
    // Define the methods and behaviors here
}

// In RustPyNet
impl TaskQueue for PythonTaskQueue {
    // Implement the methods here
}

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

type MyResult<T> = Result<T, PythonTaskError>;

#[derive(Debug)]
pub enum PythonTaskResult {
    Int(i32),
    Str(String),
    Float(f64),
    // ... add other variants as needed
}

type Task = (
    Box<dyn for<'p> FnOnce(Python<'p>, std::sync::mpsc::Sender<MyResult<PythonTaskResult>>) + Send>,
    std::sync::mpsc::Sender<MyResult<PythonTaskResult>>,
);

pub struct PythonTaskQueue {
    tasks: Arc<Mutex<VecDeque<Task>>>,
}

impl PythonTaskQueue {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn enqueue(
        &self,
        task_fn: Box<
            dyn for<'p> FnOnce(Python<'p>, std::sync::mpsc::Sender<MyResult<PythonTaskResult>>)
                + Send,
        >,
    ) -> std::sync::mpsc::Receiver<MyResult<PythonTaskResult>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let tx_clone = tx.clone();

        let task = (task_fn, tx_clone);

        let mut tasks = self.tasks.lock().unwrap();
        tasks.push_back(task);
        println!("Task enqueued. Total tasks in queue: {}", tasks.len());

        rx // Return the receiver
    }

    pub fn wait_for_result(
        rx: std::sync::mpsc::Receiver<MyResult<PythonTaskResult>>,
    ) -> MyResult<PythonTaskResult> {
        let result = rx.recv().unwrap();
        result
    }
}

pub fn start_processing_host_python_tasks() {
    println!("Start processing python calls!");

    // Acquire the queue inside the function

    loop {
        let tasks_clone = with_python_queue!(
            CLIENT_PYTHON_PROCESS_QUEUE,
            |python_queue: &mut PythonTaskQueue| { python_queue.tasks.clone() }
        );

        println!("Hello");

        // Check the number of tasks in the queue
        let num_tasks = tasks_clone.lock().unwrap().len();
        if num_tasks > 0 {
            println!("Number of tasks in queue: {}", num_tasks);

            {
                println!("Gill pool acquired!");
                let py = unsafe { Python::assume_gil_acquired() };
                println!("Python acquired!");

                while let Some((task_fn, tx)) = tasks_clone.lock().unwrap().pop_front() {
                    println!("Executing a task from the queue...");
                    task_fn(py, tx); // Just call the task_fn without expecting a return value
                    println!("Task executed.");
                }
            }
        } else {
            // If no tasks, sleep for a short duration before checking again
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}
