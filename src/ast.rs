//! Representation of the syntax of the language.

use std::fmt::Debug;

use crate::error_handling::FileRef;


// A function definition.
#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub name_ref: FileRef,
    pub argument_names: Vec<String>,
    pub block: Vec<Statement>,
    pub returns_value: bool
}

// A statement within a block of code
#[derive(Clone, Debug)]
pub enum Statement {
    Assignment {
        variable_name: String,
        variable_name_ref: FileRef,
        value: Expression
    },
    If {
        // Each `if` or `else if` block has its own segment.
        segments: Vec<IfSegment>,
        r#else: Option<Vec<Statement>>
    },
    While {
        condition: Expression,
        block: Vec<Statement>
    },
    Call(Call),
    Return(FileRef), // Position of the return keyword
    ReturnValue {
        value: Expression,
        value_ref: FileRef // Position of the expression returned.
    },
    // Position of each keyword
    Continue(FileRef),
    Break(FileRef)
}

// A function call.
#[derive(Clone, Debug)]
pub struct Call {
    pub function_name: String,
    pub function_name_ref: FileRef,
    pub arguments: Vec<Expression>,
    pub arguments_ref: FileRef
}

// An `if` or `else if` section of an `if` statement.
#[derive(Clone, Debug)]
pub struct IfSegment {
    pub condition: Expression,
    pub block: Vec<Statement>
}

#[derive(Clone, Debug)]
pub enum Expression {
    Binary {
        left: Box<Expression>,
        right: Box<Expression>,
        operator: BinaryOperator
    },
    Unary {
        value: Box<Expression>,
        operator: UnaryOperator
    },
    Call(Call),
    Variable {
        name: String,
        pos: FileRef
    },
    Literal(i32)
}

#[derive(PartialEq, Clone, Debug, Copy)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    And,
    Or,
    Xor,
    ShiftLeft,
    ShiftRight,
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    Remainder,
    LessThanOrEqual,
    Power
}

#[derive(Clone, Debug, PartialEq, Copy)]
pub enum UnaryOperator {
    Not,
    Negate
}