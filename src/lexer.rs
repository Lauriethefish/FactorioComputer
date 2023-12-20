//! Splits the code up into tokens that are easier to parse.
//! The lexer here does not parse operators made of multiple symbols, such as !=,
//! and these are handled in the parser instead.

use std::{str::Chars, iter::Enumerate, sync::Arc};

use phf::phf_map;

use crate::error_handling::{CompileResult, FileRef, SourceFile, FileTaggedError, CompileErrors};

// A token is a small group of characters that conveys a particular meaning to the compiler.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    OpenParen,
    CloseParen,
    OpenBrace,
    CloseBrace,
    Identifier(String),
    Number(i32),
    If,
    While,
    Else,
    Semicolon,
    Plus,
    Minus,
    Int,
    Void,
    Percent,
    Comma,
    Star,
    ForwardSlash,
    Ampersand,
    Carat,
    Bar,
    LeftArrow,
    RightArrow,
    Equals,
    Bang,
    Tilda,
    Return,
    Continue,
    Break,
    EndOfFile
}

// Use a pht for keywords to avoid a massive if/elseif block.
static KEYWORDS: phf::Map<&'static str, Token> = phf_map! {
    "if" => Token::If,
    "while" => Token::While,
    "else" => Token::Else,
    "int" => Token::Int,
    "void" => Token::Void,
    "continue" => Token::Continue,
    "break" => Token::Break,
    "return" => Token::Return
};

const NUMBER_BASE: u32 = 10;

fn is_valid_for_identifier(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn parse_number(iter: &mut Enumerate<Chars>, first_digit: i32) -> i32 {
    let mut current: i32 = first_digit;
    loop {
        match iter.clone().next() {
            None => break current, // EOF
            Some((_, c)) => match c.to_digit(NUMBER_BASE) {
                Some(digit) => {
                    current = current * NUMBER_BASE as i32 + digit as i32;
                    iter.next().unwrap();
                },
                None => break current
            }
        }
    }
}

fn parse_identifier(iter: &mut Enumerate<Chars>, first_char: char) -> String {
    let mut result = String::new();
    result.push(first_char);

    loop {
        // Continue until a character is found that is not valid for an identifier
        match iter.clone().next() { // Clone the iterator as to not consume the next character, which may not be part of the identifier.
            None => break result, // EOF
            Some((_, c)) => if is_valid_for_identifier(c) {
                result.push(c);
                iter.next().unwrap();
            }   else    {
                break result
            }
        }
    }
}


// Takes in a string and splits it into a list of tokens.
// If an error is encountered, the character is skipped and the error is kept in a log.
// This allows any other errors later in the file to be logged. No tokens will be returned from the function, even though more may be parsed.
// The last token is always a Token::EndOfFile
pub fn tokenize(source: Arc<SourceFile>) -> CompileResult<Vec<(Token, FileRef)>> {
    let mut iter = source.text.chars().enumerate();
    let mut result = Vec::new();
    let mut errors = Vec::new();

    let mut line_index = 0;
    let mut begin_line_char_index = 0;
    while let Some((idx, c)) = iter.next() {
        if c == '\n' {
            line_index += 1;
            begin_line_char_index = idx + 1;
        }

        if c.is_whitespace() {
            continue;
        }

        let token = if let Some(first_digit) = c.to_digit(NUMBER_BASE) {
            Token::Number(parse_number(&mut iter, first_digit as i32))
        }   else if  is_valid_for_identifier(c) {
            let ident = parse_identifier(&mut iter, c);

            if let Some(keyword) = KEYWORDS.get(&ident) {
                keyword.clone()
            }   else {
                Token::Identifier(ident)
            }
        }   else { match c {
            '(' => Token::OpenParen,
            ')' => Token::CloseParen,
            '{' => Token::OpenBrace,
            '}' => Token::CloseBrace,
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '/' => Token::ForwardSlash,
            '^' => Token::Carat,
            '|' => Token::Bar,
            '%' => Token::Percent,
            '&' => Token::Ampersand,
            '<' => Token::LeftArrow,
            ',' => Token::Comma,
            '>' => Token::RightArrow,
            '=' => Token::Equals,
            '~' => Token::Tilda,
            '!' => Token::Bang,
            ';' => Token::Semicolon,
            _ => {
                errors.push(FileTaggedError {
                    msg: "Invalid character".to_owned(), 
                    position: Some(FileRef {
                        line_index,
                        file: source.clone(),
                        begin_char_index: (idx - begin_line_char_index) as u32,
                        length: 1
                    })
                });

                continue;
            }
        } 
        };

        // Locate the final character of the token.
        let final_char = match iter.clone().next() {
            Some((next_idx, _)) => next_idx,
            None => idx + 1
        };

        // Tag the token with the correct position within the file.
        result.push((token, FileRef {
            file: source.clone(),
            line_index,
            begin_char_index: (idx - begin_line_char_index) as u32,
            length: (final_char - idx) as u32
        }))
    }

    if !errors.is_empty() {
        Err(CompileErrors(errors))
    }   else {
        result.push((Token::EndOfFile, FileRef {
            file: source,
            line_index: line_index + 1,
            begin_char_index: 0,
            length: 5, // Could literally be anything, just for UI purposes.
        }));

        Ok(result)        
    }
}