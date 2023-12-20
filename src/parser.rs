//! Parses the tokens generated by the lexer to create an abstract syntax tree.

use crate::ast::Call;
use crate::ast::Function;
use crate::ast::IfSegment;
use crate::ast::Statement;
use crate::ast::UnaryOperator;
use crate::error_handling::CompileErrors;
use crate::error_handling::CompileResult;
use crate::error_handling::FileRef;
use crate::{lexer::Token, ast::{BinaryOperator, Expression}};
use crate::error;

// The order in which to execute operations.
// Each array consists of operators of equal precedence, which will be evaluated from left to right.
const PRECEDENCE: &[&[BinaryOperator]] = &[
    &[
        BinaryOperator::Power,
        BinaryOperator::ShiftLeft,
        BinaryOperator::ShiftRight
    ],
    &[
        BinaryOperator::Multiply,
        BinaryOperator::Divide,
        BinaryOperator::Remainder
    ],
    &[
        BinaryOperator::Add,
        BinaryOperator::Subtract
    ],
    &[
        BinaryOperator::NotEquals,
        BinaryOperator::Equals,
        BinaryOperator::GreaterThan,
        BinaryOperator::GreaterThanOrEqual,
        BinaryOperator::LessThan,
        BinaryOperator::LessThanOrEqual,
    ],
    &[
        BinaryOperator::And,
        BinaryOperator::Or,
        BinaryOperator::Xor,
    ]
];

// Iterates through the tokens in a file.
pub struct TokenIterator {
    tokens: Vec<(Token, FileRef)>,
    position: usize
}

impl TokenIterator {
    // Creates a new token iterator from a list of tokens and the locations of each token within a source file.
    pub fn new(tokens: Vec<(Token, FileRef)>) -> Self {
        Self {
            tokens,
            position: 0
        }
    }

    fn get_token_or_eof(&self, position: usize) -> &(Token, FileRef) {
        self.tokens.get(position)
            .unwrap_or(self.tokens.last().expect("Must have at least an EndOfFile token"))
    }

    // Gets the next token, and advances the iterator forwards.
    fn consume(&mut self) -> Token {
        self.position += 1;
        self.get_token_or_eof(self.position - 1).0.clone()
    }

    // Moves back to the previous token.
    fn move_back(&mut self) {
        self.position -= 1;
    }

    // Returns the location of the token just consumed from the file.
    fn prev_token_ref(&self) -> FileRef {
        self.get_token_or_eof(self.position - 1).1.clone()
    }

    // Get the index of the next/previous token
    fn next_token_index(&self) -> usize {
        self.position
    }

    fn prev_token_index(&self) -> usize {
        self.position - 1
    }

    // Creates a FileRef ranging between two tokens in the iterator.
    // Useful to get the reference that highlights a whole expression, etc..
    fn get_ref_range(&mut self, from: usize, to: usize) -> FileRef {
        let start_token = self.tokens[from].1.clone();
        let end_token = self.tokens[to].1.clone();

        // TODO: The start/end tokens being on separate lines is currently improperly handled due to a limitation in the FileRef struct.
        let end_char_index = if start_token.line_index != end_token.line_index {
            start_token.begin_char_index + 1
        }   else    {
            end_token.begin_char_index + end_token.length
        };

        FileRef {
            file: start_token.file.clone(),
            line_index: start_token.line_index,
            begin_char_index: start_token.begin_char_index,
            length: end_char_index - start_token.begin_char_index
        }
    }
}

macro_rules! prev_token_error {
    ($iter:expr, $($arg:tt)*) => {
        error!($iter.prev_token_ref(), $($arg)*)
    };
}


// Parses a binary operator, which may be made up of multiple tokens, e.g. !=, or ==
// If returning None then the iterator will have been moved back to where it was before calling.
fn parse_binary_operator(iter: &mut TokenIterator) -> Option<BinaryOperator> {
    match iter.consume() {
        Token::Plus => Some(BinaryOperator::Add),
        Token::Minus => Some(BinaryOperator::Subtract),
        Token::Star => Some(BinaryOperator::Multiply),
        Token::ForwardSlash => Some(BinaryOperator::Divide),
        Token::Ampersand => Some(BinaryOperator::And),
        Token::Percent => Some(BinaryOperator::Remainder),
        Token::Bar => Some(BinaryOperator::Or),
        // TODO: consider the xor operator. Could do it like python where ** is power and ^ is XOR.
        Token::Carat => Some(BinaryOperator::Power),

        Token::Equals => {
            match iter.consume() {
                Token::Equals => Some(BinaryOperator::Equals),
                _ => { iter.move_back(); iter.move_back(); None }
            }
        },
        Token::LeftArrow => {
            match iter.consume() {
                Token::Equals => Some(BinaryOperator::LessThanOrEqual),
                _ => { iter.move_back(); Some(BinaryOperator::LessThan) }
            }
        },
        Token::RightArrow => {
            match iter.consume() {
                Token::Equals => Some(BinaryOperator::GreaterThanOrEqual),
                _ => { iter.move_back(); Some(BinaryOperator::GreaterThan) }
            }
        },
        Token::Bang => {
            match iter.consume() {
                Token::Equals => Some(BinaryOperator::NotEquals),
                _ => { iter.move_back(); iter.move_back(); None }
            }
        },
        _ => { iter.move_back(); None }
    }
}

// Parses a block of statements, which must start and end with `{` and `}`
pub fn parse_block(iter: &mut TokenIterator) -> CompileResult<Vec<Statement>> {
    if iter.consume() != Token::OpenBrace {
        return prev_token_error!(iter, "Expected `{{`");
    }

    // Continue to parse statements until a } is discovered.
    let mut statements = Vec::new();
    let mut errors = Vec::new();

    loop {
        let token = iter.consume();
        let is_block_statement = match token {
            Token::CloseBrace => break,
            Token::If | Token::While => true,
            Token::EndOfFile => break,
            _ => false
        };

        iter.move_back();

        match parse_statement(iter) {
            Ok(statement) => statements.push(statement),

            Err(mut err) => {
                errors.append(&mut err.0);

                // Implement a "panic" strategy for collecting multiple errors.
                // If an error is found, continue until the end of that line (by looking for a `;` or a `}`), depending on if the first character of the statement indicated a block based statement or not.

                loop {
                    let token = iter.consume();
                    if token == Token::EndOfFile // Avoid getting in an infinite loop at the end of the file.
                    || (token == Token::CloseBrace && is_block_statement)
                    || (token == Token::Semicolon && !is_block_statement)  {
                        break;
                    }
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(statements)
    }   else {
        Err(CompileErrors(errors))
    }
}

// Parses an `if` statement, assuming that the initial `if` has already been consumed. 
fn parse_if_statement(iter: &mut TokenIterator) -> CompileResult<Statement> {
    // Parse the first segment

    let mut segments = Vec::new();
    segments.push(IfSegment {
        condition: parse_expression(iter)?,
        block: parse_block(iter)?,
    });

    loop {
        // No `else` or `else if` after the previous block.
        if iter.consume() != Token::Else {
            iter.move_back();
            return Ok(Statement::If { segments, r#else: None });
        }

        // `else if` block 
        if iter.consume() == Token::If {
            segments.push(IfSegment {
                condition: parse_expression(iter)?,
                block: parse_block(iter)?,
            });
        }   else {
            // `else` block
            iter.move_back();
            return Ok(Statement::If { segments, r#else: Some(parse_block(iter)?) });
            // cannot have anything after the `else` block
        }
    }
}

// Parses a `+=`, `-=`, etc. type statement, assuming the operator has already been read. 
fn parse_modify_in_place(iter: &mut TokenIterator, ident: String, ident_ref: FileRef, operator: BinaryOperator) -> CompileResult<Statement> {
    if iter.consume() != Token::Equals {
        prev_token_error!(iter, "Expected `=`")
    }   else {
        Ok(Statement::Assignment {
            variable_name: ident.clone(),
            variable_name_ref: ident_ref,
            value: Expression::Binary {
                left: Box::new(Expression::Variable {
                    name: ident,
                    pos: iter.prev_token_ref()
                }),
                right: Box::new(parse_expression(iter)?),
                operator
            }
        })
    }
}

// Parses all of the functions within the root of a module.
pub fn parse_module(iter: &mut TokenIterator) -> CompileResult<Vec<Function>> {
    let mut module = Vec::new();
    let mut errors = Vec::new();

    // Continue until EOF
    while iter.consume() != Token::EndOfFile {
        iter.move_back();
        match parse_function(iter) {
            Ok(function) => module.push(function),
            Err(mut errs) => {
                errors.append(&mut errs.0);

                // Continue until we find the start of another function, i.e. an int or void keyword
                loop {
                    match iter.consume() {
                        Token::Int | Token::Void | Token::EndOfFile => break,
                        _ => {}
                    }
                }

                iter.move_back();
            }
        }
    }

    if errors.is_empty() {
        Ok(module)
    }   else {
        Err(CompileErrors(errors))
    }
}

pub fn parse_function(iter: &mut TokenIterator) -> CompileResult<Function> {
    let returns_value = match iter.consume() {
        Token::Void => false,
        Token::Int => true,
        _ => return prev_token_error!(iter, "Expected function return type: `int` or `void`")
    };

    let name = match iter.consume() {
        Token::Identifier(name) => name,
        _ => return prev_token_error!(iter, "Expected function name")
    };
    let name_ref = iter.prev_token_ref();

    if iter.consume() != Token::OpenParen {
        return prev_token_error!(iter, "Expected `(`")
    }

    let mut argument_names = Vec::new();
    while let Token::Identifier(ident) = iter.consume() {
        argument_names.push(ident);

        match iter.consume() {
            Token::Comma => {},
            _ => break
        }
    }

    iter.move_back(); // Move back to allow the ending chracter to be handled, as it has already been consumed.

    match iter.consume() {
        Token::CloseParen => {},
        _ => return prev_token_error!(iter, "Expected ')'")
    };

    // Parse the block
    let block = parse_block(iter)?;
    Ok(Function {
        name,
        argument_names,
        block,
        returns_value,
        name_ref
    })

}

fn expect_semicolon_and_then<T>(iter: &mut TokenIterator, value: T) -> CompileResult<T> {
    return if iter.consume() != Token::Semicolon {
        prev_token_error!(iter, "Expected `;`")
    }   else    {
        Ok(value)
    }
}

// Parses a statement
pub fn parse_statement(iter: &mut TokenIterator) -> CompileResult<Statement> {
    let ident = match iter.consume() {
        // If beginning with an identifier, this is an assignment or call expression, which will be handled separately.
        Token::Identifier(ident) => ident,

        Token::If => return parse_if_statement(iter),
        Token::While => return Ok(Statement::While {
            condition: parse_expression(iter)?,
            block: parse_block(iter)?,
        }),

        Token::Continue => return expect_semicolon_and_then(iter, Statement::Continue(iter.prev_token_ref())),
        Token::Break => return expect_semicolon_and_then(iter, Statement::Break(iter.prev_token_ref())),

        Token::Return => match iter.consume() {
            Token::Semicolon => {
                // Return statement with no value
                return Ok(Statement::Return(iter.tokens[iter.position - 2].1.clone()));
            },
            _ => {
                // Return statement with a value
                iter.move_back();

                let idx_before_expr = iter.next_token_index();
                let expr = parse_expression(iter)?;

                let expr_ref = iter.get_ref_range(idx_before_expr, iter.prev_token_index());

                // Check that the ; is present.
                return if iter.consume() != Token::Semicolon {
                    prev_token_error!(iter, "Expected `;`")
                }   else    {
                    Ok(Statement::ReturnValue {
                        value: expr,
                        value_ref: expr_ref
                    })
                }
            }
        }
        _ => return prev_token_error!(iter, "Expected statement")
    };

    let ident_ref = iter.prev_token_ref();

    let statement = match iter.consume() {
        Token::Equals => {
            let value = parse_expression(iter)?;

            Statement::Assignment { variable_name: ident, value, variable_name_ref: ident_ref }
        },
        Token::Plus => parse_modify_in_place(iter, ident, ident_ref, BinaryOperator::Add)?,
        Token::Minus => parse_modify_in_place(iter, ident, ident_ref, BinaryOperator::Subtract)?,
        Token::Star => parse_modify_in_place(iter, ident, ident_ref, BinaryOperator::Multiply)?,
        Token::ForwardSlash => parse_modify_in_place(iter, ident, ident_ref, BinaryOperator::Divide)?,
        Token::Carat => parse_modify_in_place(iter, ident, ident_ref, BinaryOperator::Power)?,
        Token::Ampersand => parse_modify_in_place(iter, ident, ident_ref, BinaryOperator::And)?,
        Token::Bar => parse_modify_in_place(iter, ident, ident_ref, BinaryOperator::Or)?,
        Token::OpenParen => {
            iter.move_back();
            iter.move_back();

            Statement::Call(parse_call(iter)?)
        },
        _ => return prev_token_error!(iter, "Expected valid statement")
    };

    match iter.consume() {
        Token::Semicolon => Ok(statement),
        _ => prev_token_error!(iter, "Expected `;`")
    }
}

fn parse_call(iter: &mut TokenIterator) -> CompileResult<Call> {
    let function_name = match iter.consume() {
        Token::Identifier(ident) => ident,
        _ => return prev_token_error!(iter, "Expected identifier")
    };
    
    let function_name_ref = iter.prev_token_ref();

    if iter.consume() != Token::OpenParen {
        return prev_token_error!(iter, "Expected `(`");
    }

    let before_args_idx = iter.next_token_index();

    // Parse arguments
    let mut args = Vec::new();
    while let Ok(expr) = parse_expression(iter) {
        args.push(expr);

        match iter.consume() {
            Token::Comma => {},
            _ => { break; }
        }
    }
    iter.move_back();

    let after_args = iter.prev_token_index();

    match iter.consume() {
        Token::CloseParen => Ok(Call {
            arguments: args,
            function_name,
            function_name_ref,
            arguments_ref: iter.get_ref_range(before_args_idx, after_args)
        }),
        _ => prev_token_error!(iter, "Expected `)`")
    }
}


// Parses the unary section of an expression, typically a variable reference or call, but also includes a bracketed inner expression ()
fn parse_unary_expression(iter: &mut TokenIterator) -> CompileResult<Expression> {
    match iter.consume() {
        Token::Minus => Ok(Expression::Unary {
            value: Box::new(parse_unary_expression(iter)?),
            operator: UnaryOperator::Negate
        }),
        Token::Tilda => Ok(Expression::Unary {
            value: Box::new(parse_unary_expression(iter)?),
            operator: UnaryOperator::Not
        }),

        Token::Identifier(ident) => {
            match iter.consume() {
                Token::OpenParen => {
                    iter.move_back();
                    iter.move_back();

                    Ok(Expression::Call(parse_call(iter)?))
                },
                _ => {
                    iter.move_back();
                    Ok(Expression::Variable {
                        name: ident,
                        pos: iter.prev_token_ref()
                    })
                }
            }
        },
        Token::Number(n) => Ok(Expression::Literal(n)),
        Token::OpenParen => {
            let inner = parse_expression(iter)?;
            match iter.consume() {
                Token::CloseParen => Ok(inner),
                _ => prev_token_error!(iter, "Expected `)`")
            }
        },
        _ => prev_token_error!(iter, "Expected unary expression"),
    }
}

// Parses an expression.
pub fn parse_expression(iter: &mut TokenIterator) -> CompileResult<Expression> {
    let mut expressions = Vec::new();
    let mut operators = Vec::new();

    // Keep parsing expressions until we no longer have a valid binary operator to continue.
    loop {
        let expr = parse_unary_expression(iter)?;
        expressions.push(expr);

        match parse_binary_operator(iter) {
            None => break,
            Some(operator) => operators.push(operator)
        }
    }

    // Reduce the list of binary operations into one according to the operator precedence.
    for operator_set in PRECEDENCE {
        let mut reduced_expressions = Vec::new();
        let mut reduced_operators = Vec::new();

        let mut expr_iter = expressions.into_iter();
        reduced_expressions.push(expr_iter.next().expect("Must have at least one expression"));

        let mut operator_iter = operators.into_iter();
        while let Some(next_expr) = expr_iter.next() {
            if let Some(operator) = operator_iter.next() {
                if operator_set.contains(&operator) {
                    let prev_expr = reduced_expressions.pop().unwrap();

                    reduced_expressions.push(Expression::Binary {
                        left: Box::new(prev_expr),
                        // Should be unreachable
                        right: Box::new(next_expr),
                        operator: operator
                    });

                    continue;
                }   else {
                    reduced_operators.push(operator);
                }
            } 

            // Cannot reduce this expression, simply add it to the output,
            reduced_expressions.push(next_expr);
        }

        expressions = reduced_expressions;
        operators = reduced_operators;
    }

    assert!(expressions.len() == 1, "Operator precedence failed to reduce an expression to one binary operation. This is a bug.
        Check that all operators have an assigned precedence.");
    Ok(expressions.into_iter().next().unwrap())
}