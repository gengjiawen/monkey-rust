#[macro_use]
extern crate lazy_static;

mod op_code;
mod compiler;
mod vm;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
