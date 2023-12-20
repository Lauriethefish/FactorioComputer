use core::fmt;
use std::fmt::Display;
use phf::phf_map;
use anyhow::anyhow;

use crate::blueprint::SignalId;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Instruction {
    Jump(i32),
    JumpIfNonZero(i32),
    JumpIfZero(i32),
    Save(i32),
    Load(i32),
    Constant(i32),
    Add,
    Subtract,
    Divide,
    Multiply,
    Power,
    Remainder,
    ShiftLeft,
    ShiftRight,
    And,
    Or,
    Xor,
    Not,
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
    Pop,
    JumpSubRoutine(i32),
    Return
}

static NO_ARG_INSTRUCTIONS: phf::Map<&'static str, Instruction> = phf_map! {
    "ADD" => Instruction::Add,
    "SUB" => Instruction::Subtract,
    "MUL" => Instruction::Multiply,
    "DIV" => Instruction::Divide,
    "REM" => Instruction::Remainder,
    "SHL" => Instruction::ShiftLeft,
    "SHR" => Instruction::ShiftRight,
    "AND" => Instruction::And,
    "POW" => Instruction::Power,
    "OR" => Instruction::Or,
    "XOR" => Instruction::Xor,
    "NOT" => Instruction::Not,
    "EQ" => Instruction::Equal,
    "NE" => Instruction::NotEqual,
    "GT" => Instruction::GreaterThan,
    "LT" => Instruction::LessThan,
    "GTE" => Instruction::GreaterThanOrEqual,
    "LTE" => Instruction::LessThanOrEqual,
    "POP" => Instruction::Pop,
    "RET" => Instruction::Return
};

impl TryFrom<&str> for Instruction {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> anyhow::Result<Self> {
        match value.find(' ') {
            Some(index) => {
                let (label, arg_str) = value.split_at(index);

                let parsed_arg = arg_str[1..].parse::<i32>()?;
                if label == "JUMP" {
                    Ok(Instruction::Jump(parsed_arg))
                }   else if label == "JMPIF" {
                    Ok(Instruction::JumpIfNonZero(parsed_arg))
                }   else if label == "JMPNIF" {
                    Ok(Instruction::JumpIfZero(parsed_arg))
                }   else if label == "SAVE" {
                    Ok(Instruction::Save(parsed_arg))
                }   else if label == "LOAD" {
                    Ok(Instruction::Load(parsed_arg))
                }   else if label == "CNST" {
                    Ok(Instruction::Constant(parsed_arg))
                }   else {
                    Err(anyhow!("Unknown instruction {value}"))
                }
            },
            None => match NO_ARG_INSTRUCTIONS.get(value) {
                Some(inst) => Ok(*inst),
                None => Err(anyhow!("Unknown instruction {value}")),
            },
        }
    }
}

impl Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Instruction::Jump(addr) => write!(f, "JUMP {addr}"),
            Instruction::JumpIfNonZero(addr) => write!(f, "JMPIF {addr}"),
            Instruction::JumpIfZero(addr) => write!(f, "JMPNIF {addr}"),
            Instruction::Save(addr) => write!(f, "SAVE {addr}"),
            Instruction::Load(addr) => write!(f, "LOAD {addr}"),
            Instruction::Constant(value) => write!(f, "CNST {value}"),
            Instruction::Add => write!(f, "ADD"),
            Instruction::Subtract => write!(f, "SUB"),
            Instruction::Divide => write!(f, "DIV"),
            Instruction::Multiply => write!(f, "MUL"),
            Instruction::Power => write!(f, "POW"),
            Instruction::Remainder => write!(f, "REM"),
            Instruction::ShiftLeft => write!(f, "SHL"),
            Instruction::ShiftRight => write!(f, "SHR"),
            Instruction::And => write!(f, "AND"),
            Instruction::Or => write!(f, "OR"),
            Instruction::Xor => write!(f, "XOR"),
            Instruction::Not => write!(f, "NOT"),
            Instruction::Equal => write!(f, "EQ"),
            Instruction::NotEqual => write!(f, "NE"),
            Instruction::GreaterThan => write!(f, "GT"),
            Instruction::LessThan => write!(f, "LT"),
            Instruction::GreaterThanOrEqual => write!(f, "GTE"),
            Instruction::LessThanOrEqual => write!(f, "LTE"),
            Instruction::Pop => write!(f, "POP"),
            Instruction::JumpSubRoutine(addr) => write!(f, "JSR {addr}"),
            Instruction::Return => write!(f, "RET"),
        }
    }
}

impl Instruction {
    pub fn get_opcode(&self) -> i32 {
        match self {
            Instruction::Jump(_) => 1,
            Instruction::JumpIfNonZero(_) => 2,
            Instruction::Save(_) => 3,
            Instruction::Load(_) => 4,
            Instruction::Constant(_) => 5,
            Instruction::JumpIfZero(_) => 25,
            Instruction::Add => 6,
            Instruction::Subtract => 7,
            Instruction::Divide => 8,
            Instruction::Multiply => 9,
            Instruction::Power => 10,
            Instruction::Remainder => 11,
            Instruction::ShiftLeft => 12,
            Instruction::ShiftRight => 13,
            Instruction::And => 14,
            Instruction::Or => 15,
            Instruction::Xor => 16,
            Instruction::Not => 17,
            Instruction::Equal => 18,
            Instruction::NotEqual => 19,
            Instruction::GreaterThan => 20,
            Instruction::LessThan => 21,
            Instruction::GreaterThanOrEqual => 22,
            Instruction::LessThanOrEqual => 23,
            Instruction::Pop => 24,
            Instruction::JumpSubRoutine(_) => 26,
            Instruction::Return => 27,
        }
    }

    pub fn get_argument_signal(&self) -> Option<(SignalId, i32)> {
        let address_signal = SignalId {
            r#type: "virtual".to_owned(),
            name: "signal-A".to_owned(),
        };

        let data_signal = SignalId {
            r#type: "virtual".to_owned(),
            name: "signal-D".to_owned(),
        };

        match self {
            Instruction::Jump(addr) => Some((address_signal, *addr)),
            Instruction::JumpIfNonZero(addr) => Some((address_signal, *addr)),
            Instruction::JumpIfZero(addr) => Some((address_signal, *addr)),
            Instruction::Save(addr) => Some((address_signal, *addr)),
            Instruction::Load(addr) => Some((address_signal, *addr)),
            Instruction::Constant(value) => Some((data_signal, *value)),
            Instruction::JumpSubRoutine(addr) => Some((address_signal, *addr)),
            _ => None
        }
    }
}