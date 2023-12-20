//! Module for error reporting that links to source files.

use std::{sync::Arc, io, fs, fmt::{Display, self}};

// A file from which code is read.
pub struct SourceFile {
    pub text: String,
    pub path: String
}

impl SourceFile {
    // Loads the text from a particular path into a source file.
    pub fn load_from_path(path: String) -> io::Result<Self> {
        Ok(Self {
            text: fs::read_to_string(&path)?,
            path,
        })
    }
}

// A reference to a particular character, or range of characters, within a source file.
#[derive(Clone)]
pub struct FileRef {
    pub file: Arc<SourceFile>,
    pub line_index: u32,
    pub begin_char_index: u32, // The first character of text included in the reference
    pub length: u32
}

impl fmt::Debug for FileRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file.path, self.line_index + 1, self.begin_char_index + 1)
    }
}

// A singular compilation error, linked to a location in the source file.
#[derive(Clone)]
pub struct FileTaggedError {
    pub position: Option<FileRef>, // May be None in the case of linking errors.
    pub msg: String
}

impl Display for FileTaggedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "-------------")?;

        match &self.position {
            Some(position) => {
                let line = position.file.text
                    .lines()
                    .nth(position.line_index as usize)
                    .unwrap_or("<end of file>");

                writeln!(f, "at {}:{}:", position.file.path, position.line_index + 1)?;
                writeln!(f)?;

                writeln!(f, "-> {line}")?;
                write!(f, "-> ")?;
                for _ in 0..(position.begin_char_index)  {
                    write!(f, " ")?;
                }

                for _ in 0..position.length {
                    write!(f, "^")?;
                }
                writeln!(f, " {}", self.msg)?;
            },
            None => writeln!(f, "{}", self.msg)?
        }

        Ok(())
    }
}

// Errors occuring during compilation
pub struct CompileErrors(pub Vec<FileTaggedError>);

impl Display for CompileErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.len() == 1 {
            writeln!(f, "1 error generated:")?;
        }   else {
            writeln!(f, "{} errors generated:", self.0.len())?;
        }

        for error in &self.0 {
            writeln!(f, "{error}")?;
        }

        Ok(())
    }
}

// Represents the result of compiling a program.
pub type CompileResult<T> = std::result::Result<T, CompileErrors>;

#[macro_export]
macro_rules! error {
    ($position: expr, $($arg:tt)*) => {
        Err($crate::error_handling::CompileErrors(vec![$crate::error_handling::FileTaggedError {
            position: Some($position),
            msg: format!($($arg)*)
        }]))
    };
}

#[macro_export]
macro_rules! untagged_err {
    ($($arg:tt)*) => {
        Err($crate::error_handling::CompileErrors(vec![$crate::error_handling::FileTaggedError {
            position: None,
            msg: format!($($arg)*)
        }]))
    };
}