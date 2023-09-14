use lazy_static::lazy_static;
use pyo3::prelude::*;
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

use std::fmt::Debug;

use pyo3::types::{IntoPyDict, PyDict, PyList, PyString, PyTuple};
use pyo3::{Python, ToPyObject};

use rayon::prelude::*;
use std::process::Command;

use crate::CLIENT_PYTHON_PROCESS_QUEUE;

/// Attempts to lock the provided Python queue and returns it.
///
/// This macro will repeatedly try to lock the provided queue until it succeeds.
/// It will sleep for a random duration between attempts.
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

/// Executes the provided code block with the provided Python queue.
///
/// This macro will try to lock the queue and execute the code block with it.
/// It will return the result of the code block execution.
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

/// Represents various errors that can occur while processing Python tasks.
///
/// This enum encapsulates the different types of errors that might be encountered
/// when interfacing with Python through the `pyo3` crate. It provides a structured way
/// to handle these errors in Rust.
#[derive(Debug)]
pub enum PythonTaskError {
    /// Represents a generic Python error with a given message.
    PythonError(String),
    /// Indicates that an unsupported number type was encountered.
    UnsupportedNumberType,
    /// Indicates that an unsupported value type was encountered.
    UnsupportedValueType,
    /// Represents any other error with a given message.
    OtherError(String),
    // Add other error variants as needed
}

/// Represents the possible results returned by a Python task.
///
/// This enum models various data types and structures that
/// can be returned from Python to Rust. It's a counterpart to `PythonTaskContext`,
/// providing a way to easily convert between native Rust types and their Python results.
#[derive(Clone, Debug)]
pub enum PythonTaskResult {
    /// A dictionary-like structure mapping string keys to other PythonTaskResult values.
    Map(HashMap<String, PythonTaskResult>),
    /// A list-like structure containing other PythonTaskResult values.
    List(Vec<PythonTaskResult>),
    /// A string value.
    Str(String),
    /// An integer value.
    Int(i32),
    /// A floating-point value.
    Float(f64),
    /// A boolean value.
    Bool(bool),
    /// Represents the absence of a value.
    None,
    /// Represents an error with a given message.
    Error(String),
}

impl fmt::Display for PythonTaskResult {
    /// Provides a way to format the `PythonTaskResult` for display purposes.
    ///
    /// This implementation is particularly useful for debugging and logging,
    /// as it gives a human-readable representation of the result.
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

/// Represents the possible contexts to send to a function that will run with Python.
///
/// This enum is designed to model various data types and structures that
/// can be passed between Rust and Python, providing a way to easily convert
/// between native Rust types and their Python counterparts.
#[derive(Clone, Debug)]
pub enum PythonTaskContext {
    /// A dictionary-like structure mapping string keys to other PythonTaskContext values.
    Map(HashMap<String, PythonTaskContext>),
    /// A list-like structure containing other PythonTaskContext values.
    List(Vec<PythonTaskContext>),
    /// A string value
    Str(String),
    /// An integer value.
    Int(i32),
    /// A floating-point value.
    Float(f64),
    /// A boolean value.
    Bool(bool),
    /// Represents the absence of a value.
    None,
    /// Represents an error with a given message.
    Error(String),
}

impl ToPyObject for PythonTaskContext {
    /// Convert the `PythonTaskContext` into a corresponding PyObject.
    ///
    /// This method facilitates the conversion of the enum variants to their respective
    /// Python objects, making it easier to pass Rust-native data to Python functions.
    fn to_object(&self, py: Python) -> PyObject {
        match self {
            PythonTaskContext::Map(map) => {
                let dict = PyDict::new(py);
                for (key, value) in map {
                    dict.set_item(key, value.to_object(py)).unwrap();
                }
                dict.to_object(py)
            }
            PythonTaskContext::List(lst) => {
                let py_list = PyList::empty(py);
                for item in lst {
                    py_list.append(item.to_object(py)).unwrap();
                }
                py_list.to_object(py)
            }
            PythonTaskContext::Str(s) => PyString::new(py, s).to_object(py),
            PythonTaskContext::Int(i) => i.to_object(py),
            PythonTaskContext::Float(f) => f.to_object(py),
            PythonTaskContext::Bool(b) => b.to_object(py),
            PythonTaskContext::None => py.None(),
            PythonTaskContext::Error(err) => PyString::new(py, err).to_object(py),
        }
    }
}

impl fmt::Display for PythonTaskContext {
    /// Provides a way to format the `PythonTaskContext` for display purposes.
    ///
    /// This is particularly useful for debugging and logging, as it gives a human-readable
    /// representation of the context.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PythonTaskContext::None => write!(f, "None"),
            PythonTaskContext::Str(s) => write!(f, "\"{}\"", s),
            PythonTaskContext::Int(i) => write!(f, "{}", i),
            PythonTaskContext::Float(fl) => write!(f, "{}", fl),
            PythonTaskContext::Bool(b) => write!(f, "{}", b),
            PythonTaskContext::List(list) => {
                write!(f, "[")?;
                for (index, item) in list.iter().enumerate() {
                    if index != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            PythonTaskContext::Map(map) => {
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
            PythonTaskContext::Error(err) => write!(f, "Error: {}", err),
        }
    }
}

/// Alias for a Result type used for Python tasks.
pub type MyResult<T> = Result<T, PythonTaskError>;

/// Trait representing a task queue.
pub trait TaskQueue {
    // Define the methods and behaviors here
}

/// A trait representing tasks that can be executed in a Python context.
pub trait PythonTask: Debug {
    fn execute(
        &self,
        py: Python,
        tx: Sender<MyResult<PythonTaskResult>>,
    ) -> MyResult<PythonTaskResult>;

    fn serialize(&self) -> String {
        format!("{:?}", self) // This is a placeholder, adjust as needed
    }
}

// Implementation for the TaskQueue trait for PythonTaskQueue.
// In RustPyNet
impl TaskQueue for PythonTaskQueue {
    // Implement the methods here
}

/// Represents a queue of Python tasks that are to be executed.
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
    /// Creates a new empty PythonTaskQueue.
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Adds a task to the queue and returns a Receiver to get the result.
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

    /// Waits for and retrieves the result of a Python task execution.
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

/// Starts processing Python tasks from the global task queue.
///
/// This function will continuously check the global task queue for tasks,
/// execute them in a Python context, and send back the results.

pub fn start_processing_host_python_tasks() {
    println!("Start processing python calls!");

    loop {
        // Acquire the Python task queue.
        let tasks_clone = with_python_queue!(
            CLIENT_PYTHON_PROCESS_QUEUE,
            |python_queue: &mut PythonTaskQueue| { python_queue.tasks.clone() }
        );

        // Check the number of tasks in the queue.
        let mut tasks = tasks_clone.lock().unwrap();
        let num_tasks = tasks.len();
        if num_tasks > 0 {
            println!("Number of tasks in queue: {}", num_tasks);

            // Use rayon to parallelize task execution across processes
            let mut tasks_vec: Vec<_> = tasks.drain(..).collect();
            tasks_vec.par_iter_mut().for_each(|(task, tx)| {
                let output = Command::new("python")
                    .arg("your_python_script.py")
                    .arg(task.serialize()) // assuming you can serialize the task
                    .output()
                    .expect("Failed to execute process");

                if output.status.success() {
                    println!("Task successfully executed.");
                } else {
                    eprintln!(
                        "Error executing task: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
            });

            // Clear the tasks
            tasks.clear();
        } else {
            // If no tasks, sleep for a short duration before checking again.
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}
