extern crate proc_macro;

// In your macro crate or module

use heck::CamelCase;
use proc_macro::TokenStream;
use pyo3::{PyErr, PyObject, PyResult, Python};
use quote::quote;
use syn::ExprPath;
use syn::Ident;
use syn::{parse_macro_input, ItemFn, ReturnType};

#[proc_macro_attribute]
pub fn set_socket_callback(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(input as ItemFn);
    let fn_name = &input_fn.sig.ident;
    let fn_block = &input_fn.block;

    // Hash the function name to create a unique identifier
    let hashed_name = format!("{:x}", md5::compute(fn_name.to_string()));
    let register_fn_name = format_ident!("register_{}", hashed_name);

    let output = quote! {
        fn #fn_name() {
            #fn_block
        }

        // This code will be generated outside the function
        #[allow(clippy::redundant_closure)]
        #[ctor::ctor]
        fn #register_fn_name() {
            RustPyNet::register_callback(stringify!(#fn_name), Box::new(#fn_name) as Box<dyn Fn() + Send + Sync + 'static>);
        }
    };

    output.into()
}

macro_rules! with_python_queue {
    ($queue:expr, $code:expr) => {{
        let mut acquired = false;
        let mut result;
        while !acquired {
            match $queue.lock() {
                Ok(mut guard) => {
                    acquired = true;
                    result = $code(&mut *guard); // Pass the locked queue to the closure
                }
                Err(_) => {
                    let sleep_duration =
                        std::time::Duration::from_millis(rand::random::<u64>() % 1000);
                    println!("Not being able to lock on Pool Macros!");
                    std::thread::sleep(sleep_duration);
                }
            }
        }
        result
    }};
}

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

        struct #task_struct_name {
            // This structure will be used to capture any necessary input for the Python function.
            // For now, let's just keep a dictionary of strings.
            dict: HashMap<String, String>,
        }

        impl PythonTask for #task_struct_name {
            fn execute(&self, py: Python) -> MyResult<PythonTaskResult> {
                let result: PyResult<PythonTaskResult> = (|| {
                    #block
                })();
                match result {
                    Ok(val) => Ok(val),
                    Err(err) => Err(PythonTaskError::PythonError(format!("{:?}", err))),
                }
            }
        }

        fn #name(dict: &HashMap<String, String>) -> #ret_type {
            let task = #task_struct_name {
                dict: dict.clone(),
            };

            match RustPyNet::CLIENT_PYTHON_PROCESS_QUEUE.try_lock() {
                Ok(mut python_queue) => {
                    let rx = python_queue.enqueue(Box::new(task));
                    let result = PythonTaskQueue::wait_for_result(rx);
                    result
                },
                Err(_) => Err(PythonTaskError::OtherError("Not being able to lock on Pool".to_string()))
            }
        }
    };

    TokenStream::from(expanded)
}
