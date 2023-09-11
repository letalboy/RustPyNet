extern crate proc_macro;

// In your macro crate or module

use heck::CamelCase;
use proc_macro::TokenStream;
use pyo3::{PyErr, PyObject, PyResult, Python};
use quote::quote;
use syn::ExprPath;
use syn::Ident;
use syn::{parse_macro_input, ItemFn, ReturnType};
extern crate quote;
use quote::format_ident;

#[proc_macro_attribute]
pub fn run_with_py(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let name = &input.sig.ident;
    let block = &input.block;
    let ret_type = match &input.sig.output {
        ReturnType::Default => quote! { () },
        ReturnType::Type(_, ty) => quote! { #ty },
    };

    let task_struct_name = format_ident!("{}Task", name.to_string().to_camel_case());

    let expanded = quote! {
        use RustPyNet::python_pool::pool::{PythonTask, MyResult};
        use std::sync::mpsc::Sender;

        struct #task_struct_name {
            dict: HashMap<String, String>,
        }

        impl PythonTask for #task_struct_name {
            fn execute(&self, py: Python, tx: Sender<MyResult<PythonTaskResult>>) -> MyResult<PythonTaskResult> {
                let result: PyResult<PythonTaskResult> = (|| {
                    #block
                })();

                // Send the result back through the provided channel.
                let send_result = match result {
                    Ok(val) => tx.send(Ok(val)),
                    Err(err) => tx.send(Err(PythonTaskError::PythonError(format!("{:?}", err)))),
                };

                // Check if sending was successful.
                match send_result {
                    Ok(_) => Ok(PythonTaskResult::None),  // or some other suitable value indicating success
                    Err(_) => Err(PythonTaskError::OtherError("Failed to send result back.".to_string())),
                }
            }
        }

        fn #name(dict: &HashMap<String, String>) -> #ret_type {
            let task = #task_struct_name {
                dict: dict.clone(),
            };

            let rx: std::sync::mpsc::Receiver<MyResult<PythonTaskResult>>;

            loop {
                match RustPyNet::CLIENT_PYTHON_PROCESS_QUEUE.lock() {
                    Ok(mut python_queue) => {
                        rx = python_queue.enqueue(Box::new(task));
                        break;
                    }
                    Err(_) => {
                        let sleep_duration = std::time::Duration::from_millis(rand::random::<u64>() % 1000);
                        println!("Not being able to lock on Pool!");
                        std::thread::sleep(sleep_duration);
                    }
                }
            }


            PythonTaskQueue::wait_for_result(rx)
        }
    };

    TokenStream::from(expanded)
}
