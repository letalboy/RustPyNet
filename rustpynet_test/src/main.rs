use RustPyNet::python_pool::pool::{start_processing_host_python_tasks, PythonTaskResult};
use RustPyNet::run_with_py;

use chrono::Duration;
use pyo3::types::PyAny;
use pyo3::{PyResult, Python};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{self, format};
use RustPyNet::python_pool::pool::PythonTaskError;
use RustPyNet::python_pool::pool::PythonTaskQueue;
use RustPyNet::python_pool::pool::TaskQueue;

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
                    println!("Not being able to lock on main");
                    std::thread::sleep(sleep_duration);
                }
            }
        }

        println!("Locked Python Queue in main");

        q.unwrap()
    }};
}

// fn compute_sum(dict: &HashMap<String, PyAny>) -> Result<PythonTaskResult, PythonTaskError> {
//     let task = Box::new(move |py: Python, tx| {
//         let result: PyResult<PythonTaskResult> = (|| {
//             let sum: i32 = py.eval("1 + 2", None, None)?.extract()?;
//             Ok(PythonTaskResult::Int(sum))
//         })();
//         match result {
//             Ok(val) => tx.send(Ok(val)).unwrap(),
//             Err(err) => tx
//                 .send(Err(PythonTaskError::PythonError({
//                     let res = format!("{0:?}", err);
//                     res
//                 })))
//                 .unwrap(),
//         }
//     });
//     match RustPyNet::CLIENT_PYTHON_PROCESS_QUEUE.try_lock() {
//         Ok(mut python_queue) => {
//             {
//                 println!("Locked on Pool macros!\n");
//             };
//             let result = python_queue.enqueue(task);
//             result
//         }
//         Err(_) => {
//             {
//                 println!("Not being able to lock on Pool macros!\n");
//             };
//             Err(PythonTaskError::OtherError(
//                 "Not being able to lock on Pool macros!".to_string(),
//             ))
//         }
//     }
// }

#[run_with_py]
fn compute_sum(
    py: Python,
    dict: &HashMap<String, String>,
) -> Result<PythonTaskResult, PythonTaskError> {
    // Here, dict contains Rust strings. If you need to use them in Python,
    // you'd convert them back to Python strings.

    // Sample Python code: compute the sum of 1 + 2
    let sum: i32 = py.eval("1 + 2", None, None)?.extract()?;
    println!("Sum is: {}", sum);
    Ok(PythonTaskResult::Int(sum))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn test_parallel_compute_sum() {
        // Number of parallel tests to run
        const NUM_TESTS: usize = 10;

        // Initialize the Python interpreter
        pyo3::prepare_freethreaded_python();

        // Start processing tasks in a separate thread
        std::thread::spawn(move || {
            start_processing_host_python_tasks();
        });

        std::thread::sleep(std::time::Duration::from_secs(2)); // Whait pool initialize!

        // Channel to collect results
        let (tx, rx) = mpsc::channel();

        // Spawn multiple threads
        let handles: Vec<_> = (0..NUM_TESTS)
            .map(|_| {
                let rust_string = "Hello!".to_string();
                let mut sample_dict: HashMap<String, String> = HashMap::new();
                sample_dict.insert("key".to_string(), rust_string);

                let tx = tx.clone();

                std::thread::spawn(move || {
                    let result = compute_sum(&sample_dict).unwrap();
                    tx.send(result).unwrap();
                })
            })
            .collect();

        // Wait for threads to complete and collect results
        let mut results = Vec::new();
        for _ in 0..NUM_TESTS {
            results.push(rx.recv().unwrap());
        }

        // Check results for correctness
        for result in results {
            match result {
                PythonTaskResult::Int(sum) => {
                    assert_eq!(sum, 3); // 1 + 2 = 3
                }
                PythonTaskResult::Error(e) => {
                    panic!("Received an error: {:?}", e);
                }
                _ => {
                    panic!("Received an unexpected result");
                }
            }
        }
    }
}

fn main() {
    // Initialize the Python interpreter
    pyo3::prepare_freethreaded_python();

    // Start processing tasks in a separate thread
    std::thread::spawn(move || {
        start_processing_host_python_tasks();
    });

    std::thread::sleep(std::time::Duration::from_secs(2)); // Whait pool initialize!

    // Create a sample dictionary (if required by your function)
    let sample_dict = HashMap::new();

    // Use the macro-enabled function to compute the sum
    let result = compute_sum(&sample_dict);

    println!("Result: {:?}", result);

    match result {
        Ok(PythonTaskResult::Int(value)) => {
            println!("The int is: {}", value);
        }
        Ok(PythonTaskResult::Float(value)) => {
            println!("The float is: {}", value);
        }
        Ok(PythonTaskResult::Str(value)) => {
            println!("The string is: {}", value);
        }
        Ok(PythonTaskResult::Map(value)) => {
            println!("The map is: {:?}", value);
        }
        Ok(PythonTaskResult::List(value)) => {
            println!("The list is: {:?}", value);
        }
        Ok(PythonTaskResult::Bool(value)) => {
            println!("The bool is: {}", value);
        }
        Ok(PythonTaskResult::Error(value)) => {
            println!("The Error is: {}", value);
        }
        Ok(PythonTaskResult::Empty) => {
            println!("Empty");
        }

        Err(PythonTaskError::PythonError(err)) => println!("Python error: {}", err),
        Err(PythonTaskError::UnsupportedNumberType) => println!("Error: Unsupported number type"),
        Err(PythonTaskError::UnsupportedValueType) => println!("Error: Unsupported value type"),
        Err(PythonTaskError::OtherError(err)) => println!("Other error: {}", err),
        // ... handle other variants of PythonTaskResult and error variants ...
    }

    // Keep the main thread alive (you can add an exit condition if needed)
    // loop {
    //     std::thread::sleep(std::time::Duration::from_secs(1));
    // }
}
