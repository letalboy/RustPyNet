use crate::python_pool::pool::PythonTaskQueue;
use lazy_static::lazy_static;
use std::sync::{Arc, Mutex};
pub mod python_pool;

// RustPyNet/src/lib.rs or RustPyNet/src/mod.rs
pub use rustpynet_macros::*;

lazy_static! {
    pub static ref CLIENT_PYTHON_PROCESS_QUEUE: Mutex<PythonTaskQueue> =
        Mutex::new(PythonTaskQueue::new());
}
