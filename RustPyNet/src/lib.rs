use crate::python_pool::pool::PythonTaskQueue;
use lazy_static::lazy_static;
use std::sync::{Arc, Mutex};
pub mod python_pool;

// RustPyNet/src/lib.rs or RustPyNet/src/mod.rs
pub use rustpynet_macros::*;

lazy_static! {
    pub static ref CLIENT_PYTHON_PROCESS_QUEUE: Mutex<PythonTaskQueue> =
        Mutex::new(PythonTaskQueue::new());
    pub static ref CALLBACK_PATTERNS: Arc<Mutex<std::collections::HashMap<&'static str, Box<dyn Fn() + Send + Sync + 'static>>>> = {
        let m = std::collections::HashMap::new();
        Arc::new(Mutex::new(m))
    };
}

pub fn register_callback(name: &'static str, callback: Box<dyn Fn() + Send + Sync + 'static>) {
    let mut callbacks = CALLBACK_PATTERNS.lock().unwrap();
    callbacks.insert(name, callback);
}
