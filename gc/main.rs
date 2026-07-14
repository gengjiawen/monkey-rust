use compiler::compiler::{Bytecode, Compiler};
use compiler::snapshot::{read_bytecode, write_bytecode};
use compiler::symbol_table::SymbolTable;
use gc::runner::{compile_source, run_bytecode};
use gc::{GcRuntimeError, GcVM};
use object::Object;
use parser::parse;
use std::io::stdin;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

const USAGE: &str = "\
usage:
  monkey-gc                                                start the REPL
  monkey-gc compile <file.monkey> [-o <file.mbc>] [--strip]
  monkey-gc run <file.monkey|file.mbc> [--max-instructions <n>]";

struct Repl {
    symbol_table: SymbolTable,
    constants: Vec<Rc<Object>>,
    vm: GcVM,
}

impl Repl {
    fn new() -> Self {
        // Seed the persistent state from Compiler::new() so builtins like `len`
        // stay resolvable; new_with_state replaces the symbol table wholesale.
        let mut bootstrap_compiler = Compiler::new();
        let bootstrap = bootstrap_compiler
            .compile(&parse("").expect("empty program should parse"))
            .expect("empty program should compile");
        Self {
            symbol_table: bootstrap_compiler.symbol_table,
            constants: bootstrap_compiler.constants,
            vm: GcVM::new(bootstrap),
        }
    }

    fn eval_line(&mut self, input: &str) -> Result<String, String> {
        let program = parse(input).map_err(|errors| format!("parse error: {}", errors[0]))?;

        // Compile against clones and commit only after a successful run, so a
        // failed line cannot leak a half-defined binding into the next one.
        let mut compiler =
            Compiler::new_with_state(self.symbol_table.clone(), self.constants.clone());
        let bytecode = compiler.compile(&program)?;
        self.vm
            .set_global_names(compiler.symbol_table.global_symbols());
        self.vm.load_bytecode(bytecode);
        self.vm
            .run_with_budget(usize::MAX)
            .map_err(|error| error.message)?;
        self.symbol_table = compiler.symbol_table;
        self.constants = compiler.constants;
        Ok(self.vm.last_result_string())
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, rest) = match args.split_first() {
        None => {
            repl();
            return;
        }
        Some(split) => split,
    };
    let outcome = match command.as_str() {
        "compile" => compile_command(rest).map(|()| None),
        "run" => run_command(rest).map(Some),
        other => Err(CliError::usage(format!("unknown command `{}`", other))),
    };
    match outcome {
        Ok(Some(result)) => println!("{}", result),
        Ok(None) => {}
        Err(error) => {
            eprintln!("{}", error.message);
            std::process::exit(error.exit_code);
        }
    }
}

fn repl() {
    println!("Welcome to monkey gc by gengjiawen");
    let mut repl = Repl::new();

    loop {
        print!("> ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        stdin().read_line(&mut input).unwrap();

        if input.trim_end().is_empty() {
            println!("bye");
            std::process::exit(0);
        }

        match repl.eval_line(&input) {
            Ok(result) => println!("{}", result),
            Err(error) => eprintln!("{}", error),
        }
    }
}

#[derive(Debug)]
struct CliError {
    exit_code: i32,
    message: String,
}

impl CliError {
    /// Bad invocation: report the problem plus usage, exit code 2.
    fn usage(message: impl std::fmt::Display) -> Self {
        CliError {
            exit_code: 2,
            message: format!("error: {}\n{}", message, USAGE),
        }
    }

    /// Operational failure (I/O, parse, compile, bad snapshot): exit code 1.
    fn failure(message: impl std::fmt::Display) -> Self {
        CliError {
            exit_code: 1,
            message: format!("error: {}", message),
        }
    }

    fn runtime(error: &GcRuntimeError) -> Self {
        // Spans inside an .mbc file are untrusted integers, so they are
        // printed numerically and never used to slice source text.
        let message = match &error.span {
            Some(span) => format!(
                "runtime error: {} (source offset {}..{})",
                error.message, span.start, span.end
            ),
            None => format!("runtime error: {}", error.message),
        };
        CliError {
            exit_code: 1,
            message,
        }
    }
}

fn compile_command(args: &[String]) -> Result<(), CliError> {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut strip_debug = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--strip" => strip_debug = true,
            "-o" => {
                let path = iter
                    .next()
                    .ok_or_else(|| CliError::usage("-o needs an output path"))?;
                output = Some(PathBuf::from(path));
            }
            _ if input.is_none() && !arg.starts_with('-') => input = Some(PathBuf::from(arg)),
            _ => return Err(CliError::usage(format!("unexpected argument `{}`", arg))),
        }
    }
    let input = input.ok_or_else(|| CliError::usage("compile needs an input file"))?;
    if has_mbc_extension(&input) {
        return Err(CliError::usage(format!("{} is already compiled bytecode", input.display())));
    }
    let output = output.unwrap_or_else(|| input.with_extension("mbc"));
    let source = std::fs::read_to_string(&input).map_err(|error| {
        CliError::failure(format!("cannot read {}: {}", input.display(), error))
    })?;
    let bytecode = compile_source(&source).map_err(CliError::failure)?;
    let blob = write_bytecode(&bytecode, strip_debug).map_err(|error| {
        CliError::failure(format!("cannot serialize {}: {:?}", input.display(), error))
    })?;
    std::fs::write(&output, blob).map_err(|error| {
        CliError::failure(format!("cannot write {}: {}", output.display(), error))
    })?;
    Ok(())
}

fn run_command(args: &[String]) -> Result<String, CliError> {
    let mut input: Option<PathBuf> = None;
    let mut budget = usize::MAX;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--max-instructions" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::usage("--max-instructions needs a count"))?;
                budget = value.parse().map_err(|_| {
                    CliError::usage(format!("invalid instruction count `{}`", value))
                })?;
            }
            _ if input.is_none() && !arg.starts_with('-') => input = Some(PathBuf::from(arg)),
            _ => return Err(CliError::usage(format!("unexpected argument `{}`", arg))),
        }
    }
    let input = input.ok_or_else(|| CliError::usage("run needs an input file"))?;
    let bytecode = load_bytecode(&input)?;
    run_bytecode(bytecode, budget).map_err(|error| CliError::runtime(&error))
}

/// Dispatch on the file extension (design doc §7): `.mbc` goes through the
/// validating snapshot reader, everything else is treated as Monkey source.
/// A corrupt `.mbc` therefore reports `BadMagic` instead of being handed to
/// the parser as source text.
fn load_bytecode(input: &Path) -> Result<Bytecode, CliError> {
    if has_mbc_extension(input) {
        let blob = std::fs::read(input).map_err(|error| {
            CliError::failure(format!("cannot read {}: {}", input.display(), error))
        })?;
        read_bytecode(&blob).map_err(|error| {
            CliError::failure(format!("invalid bytecode in {}: {:?}", input.display(), error))
        })
    } else {
        let source = std::fs::read_to_string(input).map_err(|error| {
            CliError::failure(format!("cannot read {}: {}", input.display(), error))
        })?;
        compile_source(&source).map_err(CliError::failure)
    }
}

fn has_mbc_extension(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()) == Some("mbc")
}

#[cfg(test)]
mod cli_test;
#[cfg(test)]
mod repl_test;
