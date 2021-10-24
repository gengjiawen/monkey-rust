#[macro_use]
extern crate lazy_static;

mod op_code;
mod compiler;
mod vm;
mod op_code_test;
mod compiler_test;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
