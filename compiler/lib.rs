#[macro_use]
extern crate lazy_static;

pub mod compiler;
mod compiler_function_test;
mod compiler_test;
mod frame;
pub mod op_code;
mod op_code_test;
pub mod symbol_table;
mod symbol_table_test;
pub mod vm;
mod vm_function_test;
mod vm_test;
