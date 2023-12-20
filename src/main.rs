mod blueprint;
mod assembly;
mod lexer;
mod parser;
mod ast;
mod compiler;
mod error_handling;

use std::sync::Arc;

use assembly::Instruction;
use error_handling::{SourceFile, CompileResult};

use crate::parser::TokenIterator;

fn try_compile(source: Arc<SourceFile>) -> CompileResult<Vec<Instruction>>  {
    let tokens = lexer::tokenize(source)?;
    let ast = parser::parse_module(&mut TokenIterator::new(tokens))?;

    return compiler::compile_module(ast)
}

fn main() {
    let path = match std::env::args().nth(1) {
        Some(file_path) => file_path,
        None => {
            eprintln!("Expected file path to compile");
            return;
        }
    };
       
    let display_assembly = std::env::args().any(|arg| arg == "--assembly");

    let source_file = match SourceFile::load_from_path(path.to_string()) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("Failed to read source: {err}");
            return;
        }
    };

    let instructions = match try_compile(Arc::new(source_file)) {
        Ok(inst) => inst,
        Err(err) => {
            eprintln!("{err}");
            return;
        }
    };

    if display_assembly {
        println!("Assembly:");
        for (idx, instruction) in instructions.iter().enumerate() {
            println!("{}: {instruction}", idx + 1);
        }
    }   else {
        println!("ROM Blueprint:");
        let bp_string = blueprint::SerializedBlueprint {
            blueprint: blueprint::generate_rom_blueprint(&instructions)
        }.save();


        println!("{}", bp_string);
    }
}
