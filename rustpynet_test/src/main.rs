use RustPyNet::python_pool::pool::PythonTaskError;
use RustPyNet::python_pool::pool::PythonTaskQueue;
use RustPyNet::python_pool::pool::{start_processing_host_python_tasks, PythonTaskResult};
use RustPyNet::run_with_py;

use pyo3::ToPyObject;
use pyo3::{PyResult, Python};
use std::collections::HashMap;

use pyo3::types::IntoPyDict;
use pyo3::types::PyDict;
use pyo3::Py;
use pyo3::PyAny;
use pyo3::PyObject;
use std::sync::mpsc::Sender;
use RustPyNet::python_pool::pool::{MyResult, PythonTask, PythonTaskContext};

/// Computes the sum of two hardcoded integers.
///
/// # Example
///
/// This function computes `1 + 2`.
///
/// # Returns
///
/// Returns the result of the computation wrapped in a `PythonTaskResult`.
#[run_with_py]
fn compute_sum(context: PythonTaskContext) -> Result<PythonTaskResult, PythonTaskError> {
    // Sample Python code: compute the sum of 1 + 2
    let sum: i32 = py.eval("1 + 2", None, None)?.extract()?;
    Ok(PythonTaskResult::Int(sum))
}

/// Computes the product of two hardcoded integers.
///
/// # Example
///
/// This function computes `2 * 3`.
///
/// # Returns
///
/// Returns the result of the computation wrapped in a `PythonTaskResult`.
#[run_with_py]
fn compute_product(context: PythonTaskContext) -> Result<PythonTaskResult, PythonTaskError> {
    let product: i32 = py.eval("2 * 3", None, None)?.extract()?;
    Ok(PythonTaskResult::Int(product))
}

/// Computes the sum of two integers provided within a dictionary context.
///
/// This function demonstrates how to pass a Rust data structure (in this case, a map)
/// to Python and use it in Python code.
///
/// # Context
///
/// The `context` argument should be a `PythonTaskContext::Map` containing the integers
/// to be summed, mapped to the keys `'a'` and `'b'`.
///
/// # Procedure
///
/// 1. Convert the `context` to a PyObject.
/// 2. Extract the PyObject from the Py<PyAny>.
/// 3. Downcast to PyDict.
/// 4. Create a new PyDict `context_mapping` and map the key `context_dict` to our PyDict.
/// 5. Execute Python code to get and sum the integers from `context_dict`.
///
/// # Returns
///
/// Returns the result of the computation wrapped in a `PythonTaskResult`.
#[run_with_py]
fn compute_sum_with_dict(context: PythonTaskContext) -> Result<PythonTaskResult, PythonTaskError> {
    // Convert the context to a PyObject
    let py_context = context.to_object(py);

    // Extract the PyObject from the Py<PyAny>
    let py_object = py_context.as_ref(py);

    // Now, downcast to PyDict
    let py_dict = py_object.downcast::<PyDict>().expect("Expected a PyDict");

    let context_mapping = PyDict::new(py);
    context_mapping.set_item("context_dict", py_dict).unwrap();

    let result_sum: i32 = py
        .eval(
            "context_dict.get('a') + context_dict.get('b')",
            Some(context_mapping),
            None,
        )?
        .extract()?;
    Ok(PythonTaskResult::Int(result_sum))
}

/// Computes an invalid operation to demonstrate error handling.
///
/// This function deliberately tries to divide by zero in Python to trigger a `ZeroDivisionError`.
///
/// # Returns
///
/// Although the function attempts to return a `PythonTaskResult`, it will actually
/// always result in a `PythonTaskError` due to the zero division.
#[run_with_py]
fn compute_invalid_operation(
    context: PythonTaskContext,
) -> Result<PythonTaskResult, PythonTaskError> {
    let _: i32 = py.eval("1 / 0", None, None)?.extract()?; // This will raise a ZeroDivisionError in Python
    Ok(PythonTaskResult::Int(0)) // This line will never be reached
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::types::IntoPyDict;

    // This function is run before tests and it sets up necessary environment
    fn setup() {
        // Initialize the Python interpreter
        pyo3::prepare_freethreaded_python();

        // Start processing tasks in a separate thread
        std::thread::spawn(move || {
            start_processing_host_python_tasks();
        });

        std::thread::sleep(std::time::Duration::from_secs(2)); // for example, wait for 5 seconds
    }

    #[test]
    fn test_compute_sum() {
        setup();

        let context = PythonTaskContext::None;
        let result = compute_sum(&context);
        match result {
            Ok(PythonTaskResult::Int(value)) => assert_eq!(value, 3),
            _ => panic!("Test failed!"),
        }
    }

    #[test]
    fn test_compute_product() {
        setup();

        let context = PythonTaskContext::None;
        let result = compute_product(&context);
        match result {
            Ok(PythonTaskResult::Int(value)) => assert_eq!(value, 6), // expecting 2 * 3 = 6
            _ => panic!("Test failed!"),
        }
    }

    #[test]
    fn test_compute_sum_with_dict() {
        setup();

        let mut dict = HashMap::new();
        dict.insert("a".to_string(), PythonTaskContext::Int(5));
        dict.insert("b".to_string(), PythonTaskContext::Int(7));

        let context = PythonTaskContext::Map(dict);

        println!("context: {:?}", context);

        let result = compute_sum_with_dict(&context);
        println!("Result {:?}", result);

        match result {
            Ok(PythonTaskResult::Int(value)) => assert_eq!(value, 12),
            _ => panic!("Test failed!"),
        }
    }

    #[test]
    fn test_python_error() {
        setup();

        let context = PythonTaskContext::None;
        let result = compute_invalid_operation(&context);
        match result {
            Err(PythonTaskError::PythonError(_)) => assert!(true),
            _ => panic!("Test failed! Should have raised a Python error."),
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

    const NUM_TESTS: usize = 10;
    let (tx, rx) = std::sync::mpsc::channel();

    let handles: Vec<_> = (0..NUM_TESTS)
        .map(|_| {
            let tx = tx.clone();
            let context = PythonTaskContext::None;
            std::thread::spawn(move || {
                let result = compute_sum(&context);
                tx.send(result).unwrap();
            })
        })
        .collect();

    let mut correct_responses = 0;

    for _ in 0..NUM_TESTS {
        let result = rx.recv().unwrap();
        match result {
            Ok(PythonTaskResult::Int(value)) => {
                println!("The int is: {}", value);
                if value == 3 {
                    correct_responses += 1;
                }
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
            Ok(PythonTaskResult::None) => {
                println!("None");
            }

            Err(PythonTaskError::PythonError(err)) => println!("Python error: {}", err),

            Err(PythonTaskError::UnsupportedNumberType) => {
                println!("Error: Unsupported number type")
            }

            Err(PythonTaskError::UnsupportedValueType) => println!("Error: Unsupported value type"),

            Err(PythonTaskError::OtherError(err)) => println!("Other error: {}", err),
            // ... handle other variants of PythonTaskResult and error variants ...
        }
    }

    for handle in handles {
        handle.join().unwrap();
    }

    if correct_responses == NUM_TESTS {
        println!("All responses are correct!");
    } else {
        println!(
            "{} out of {} responses are correct",
            correct_responses, NUM_TESTS
        );
    }
}
