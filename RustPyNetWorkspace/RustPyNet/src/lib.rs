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
/// ``` ignore
/// #[run_with_py]
/// fn compute_sum(
///     context: &PythonTaskContext,
/// ) -> Result<PythonTaskResult, PythonTaskError> {
///     // from here you can use both the context and py
///     // Sample Python code: compute the sum of 1 + 2
///     let sum: i32 = py.eval("1 + 2", None, None)?.extract()?;
///     Ok(PythonTaskResult::Int(sum))
/// }
///```
/// py in this case is injected by the proc macro, so don't worry because of not see the py arg in the args, it is added when proc macro wraps the fn that you decorated with he
///
/// ### Context use case exemple:
///
/// ```ignore
/// #[run_with_py]
/// fn compute_sum_with_dict(context: PythonTaskContext) -> Result<PythonTaskResult, PythonTaskError> {
///     // Convert the context to a PyObject
///     let py_context = context.to_object(py);
///
///     // Extract the PyObject from the Py<PyAny>
///     let py_object = py_context.as_ref(py);
///
///     // Now, downcast to PyDict
///     let py_dict = py_object.downcast::<PyDict>().expect("Expected a PyDict");
///
///     let context_mapping = PyDict::new(py);
///     context_mapping.set_item("context_dict", py_dict).unwrap();
///
///    let result_sum: i32 = py
///         .eval(
///             "context_dict.get('a') + context_dict.get('b')",
///             Some(context_mapping),
///             None,
///         )?
///         .extract()?;
///     Ok(PythonTaskResult::Int(result_sum))
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
