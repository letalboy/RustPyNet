use crate::python_pool::pool::PythonTaskQueue;
use lazy_static::lazy_static;
use std::sync::{Arc, Mutex};
pub mod python_pool;

// RustPyNet/src/lib.rs or RustPyNet/src/mod.rs

/// The `run_with_py` procedural macro facilitates the execution of a given function within a Python context.
///
/// It dynamically creates a struct and its implementation based on the provided function. The function is then executed
/// within a Python context, and its results are passed back through a channel.
///
/// # Usage
///
/// ```ignore
/// #[run_with_py]
/// fn your_function_name(dict: &HashMap<String, String>) -> YourReturnType {
///     // Your function implementation here
/// }
/// ```
///
/// This macro will create the necessary infrastructure for the function to be run in a Python context.
///
/// # Parameters
///
/// - `dict`: A `HashMap` containing data that you wish to pass to the Python context.
///
/// # Returns
///
/// Returns whatever your function is intended to return, wrapped in the necessary channel and context management code.
///
/// # Errors
///
/// If there are any issues with obtaining the Python context or executing the function, an error will be returned.
pub use rustpynet_macros::run_with_py;

lazy_static! {
    pub static ref CLIENT_PYTHON_PROCESS_QUEUE: Mutex<PythonTaskQueue> =
        Mutex::new(PythonTaskQueue::new());
}
