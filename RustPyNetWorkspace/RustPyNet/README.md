# RustPyNet: Python Operations Processing Pool in Rust

RustPyNet is designed to bridge the gap between Python and Rust, offering a Python operations processing pool that integrates seamlessly with the PyO3 crate.

## Objective

The primary goal of RustPyNet is to address the limitations posed by Python's Global Interpreter Lock (GIL). By facilitating multi-threaded operations, RustPyNet allows for parallel execution of Python functions, even though it's bound to a single interpreter. This makes it particularly suitable for scenarios where the bulk of the workload is handled by Rust, but there's a need to execute smaller tasks in Python.

Key Features:
- **Bypass GIL Mechanism**: Enables parallel execution of Python code sections that would otherwise be limited by the GIL.
- **Thread-Safe Python Object Transfer**: Facilitates the transfer of Python objects between threads.
- **Procedural Macros**: Simplifies the integration of Python into Rust code with procedural macros that auto-index to the Python pool.
- **Efficient for Medium-Small Tasks**: Optimized for tasks that have a light Python workload but are heavy on the Rust side.

More info in: https://github.com/letalboy/RustPyNet