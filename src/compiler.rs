//! Compiles the ast into the code used for the factorio computer.

use std::collections::HashMap;

use crate::{ast::{Statement, Expression, BinaryOperator, UnaryOperator, Function, Call}, assembly::Instruction, error_handling::{CompileResult, FileRef, CompileErrors}, error, untagged_err};

// Number of signals we can read from or write to.
const SIGNAL_COUNT: i32 = 5;

const ENTRY_POINT: &str = "main";

// Keeps track of information in a particular scope.
#[derive(Clone, PartialEq)]
enum ScopeState {
    // Keep track of all places where `continue` or `break` statements have been placed 
    // so that we can update them with the correct jump address once this is known.
    While {
        continue_inst_addresses: Vec<usize>,
        break_inst_addresses: Vec<usize>
    },
    Other
}

// Each scope needs to pop off its local variables after it exits.
struct Scope {
    // The variables in the scope, as an offset from the bottom of the stack
    // `0` is the first local variable.
    scope_vars: HashMap<String, i32>,
    // The stack size before the scope was opened.
    starting_stack_size: i32,
    scope_type: ScopeState
}

// Keeps track of information about a function after the Function struct has been consumed.
// Used for linking between functions.
#[derive(Copy, Clone)]
struct FunctionInfo {
    arg_count: usize,
    returns_value: bool,
    id: i32,
    start_offset: i32
}

// Keeps track of the state of compilation within a particular function.
struct CompileCtx<'a> {
    instructions: Vec<Instruction>,
    // Current size of the stack.
    // Instructions such as LOAD and SAVE are relative to the top of the stack.
    // Keeping track of the stack size allows us to use certain stack values as local variables.
    stack_size: i32,
    // The scopes that are currently open, from outermost first to innermost last.
    scopes: Vec<Scope>,
    // The offset of the return value of the function from the bottom of the stack for this function.
    return_value_save_offset: Option<i32>,
    function_ids_in_module: &'a mut HashMap<String, FunctionInfo>
}

impl <'a> CompileCtx<'a> {
    // Creates a new scope with the given state.
    fn open_scope(&mut self, scope_type: ScopeState) {
        self.scopes.push(Scope {
            scope_type,
            scope_vars: HashMap::new(),
            starting_stack_size: self.stack_size
        });
    }

    // Ends the current scope and returns its state.
    fn end_scope(&mut self) -> ScopeState {
        let scope: Scope = self.scopes.pop().expect("No scope to end");

        for _ in 0..(self.stack_size - scope.starting_stack_size) {
            self.emit(Instruction::Pop);
        }

        scope.scope_type
    }

    // Prepares for an early end to a scope, i.e. with a return, continue or break statement
    // scope_idx is the last scope that will be removed.
    fn prepare_for_premature_scope_end(&mut self, scope_idx: usize) {
        // Pop but without modifying the tracked stack size so that future instructions still have the correct stack length.
        let scope: &Scope = &self.scopes[scope_idx];
        for _ in 0..(self.stack_size - scope.starting_stack_size) {
            self.instructions.push(Instruction::Pop);
        }
    }

    fn emit(&mut self, instruction: Instruction) {
        self.instructions.push(instruction);
        self.stack_size += match instruction {
            Instruction::JumpIfNonZero(_) => -1,
            Instruction::JumpIfZero(_) => -1,
            Instruction::Save(_) => -1,
            Instruction::Load(_) => 1,
            Instruction::Constant(_) => 1,
            Instruction::Add => -1,
            Instruction::Subtract => -1,
            Instruction::Divide => -1,
            Instruction::Multiply => -1,
            Instruction::Power => -1,
            Instruction::Remainder => -1,
            Instruction::ShiftLeft => -1,
            Instruction::ShiftRight => -1,
            Instruction::And => -1,
            Instruction::Or => -1,
            Instruction::Xor => -1,
            Instruction::Equal => -1,
            Instruction::NotEqual => -1,
            Instruction::GreaterThan => -1,
            Instruction::LessThan => -1,
            Instruction::GreaterThanOrEqual => -1,
            Instruction::LessThanOrEqual => -1,
            Instruction::Pop => -1,
            _ => 0
        }
    }

    fn get_variable_pos(&self, name: String, name_ref: FileRef) -> CompileResult<i32> {
        for scope in self.scopes.iter() {
            match scope.scope_vars.get(&name) {
                Some(offset) => return Ok(*offset),
                None => {}
            }
        }

        error!(name_ref, "No variable exists with this name")
    }

    fn get_variable_address(&self, name: String, name_ref: FileRef, reading: bool) -> CompileResult<i32> {
        if name.starts_with("signal_") {
            let signal_number = match name[7..].parse::<i32>() {
                Ok(num) => num,
                Err(_) => return error!(name_ref, "Signal number must be a valid integer")
            };

            if signal_number <= 0 || signal_number > SIGNAL_COUNT {
                return error!(name_ref, "Invalid signal number. Must be in range [0-{}]", SIGNAL_COUNT)
            }   else {
                Ok(if reading { -(SIGNAL_COUNT + signal_number)} else { -signal_number })
            }

        }   else {
            let offset_from_bottom_of_stack = self.get_variable_pos(name, name_ref)?;

            // Stack addresses are 1 indexed, 1 is the topmost value in the stack
            Ok(self.stack_size - offset_from_bottom_of_stack)
        }
    }

    fn save_to_variable(&mut self, name: String, name_ref: FileRef) -> CompileResult<()> {
        self.emit(Instruction::Save(self.get_variable_address(name, name_ref, false)?));
        Ok(())
    }

    fn load_from_variable(&mut self, name: String, name_ref: FileRef) -> CompileResult<()> {
        self.emit(Instruction::Load(self.get_variable_address(name, name_ref, true)?));
        Ok(())
    }

    fn add_variable(&mut self, name: String) {
        self.scopes.last_mut().expect("No scope to add variable within").scope_vars.insert(name, self.stack_size - 1);
    }
}

fn compile_function(function: Function, functions_in_module: &mut HashMap<String, FunctionInfo>) 
    -> CompileResult<Vec<Instruction>> {
    // Calling convention is to push
    // - a space for the return value to end up.
    // - the arguments
    // followed by a JSR instruction which pushes a return address

    // 0 is the first variable in our function,
    // so -1 is the return address
    // -2 is the start of the arguments
    // -2 -arg_count is the return value

    let mut scope_vars = HashMap::new();

    let arguments_start = -1 - function.argument_names.len() as i32;
    for (idx, argument) in function.argument_names.iter().enumerate() {
        scope_vars.insert(argument.clone(), arguments_start + idx as i32);
    }

    let mut ctx = CompileCtx {
        instructions: Vec::new(),
        stack_size: 0,
        scopes: vec![Scope {
            scope_type: ScopeState::Other,
            starting_stack_size: 0,
            scope_vars
        }],
        return_value_save_offset: if function.returns_value {
            Some(arguments_start - 1)
        }   else    {
            None
        },
        function_ids_in_module: functions_in_module
    };

    emit_block(function.block, &mut ctx)?;

    ctx.end_scope();
    if ctx.instructions.last() != Some(&Instruction::Return) {
        ctx.emit(Instruction::Return);
    }

    Ok(ctx.instructions)

}

pub fn compile_module(module: Vec<Function>) -> CompileResult<Vec<Instruction>> {
    let mut functions_by_name = HashMap::new();
    for (idx, function) in module.iter().enumerate() {
        if functions_by_name.contains_key(&function.name) {
            return error!(function.name_ref.clone(), "A function with this name already exists - overloading is not supported");
        }

        functions_by_name.insert(function.name.clone(), FunctionInfo {
            id: idx as i32,
            arg_count: function.argument_names.len(),
            returns_value: function.returns_value,
            start_offset: -1
        });
    }

    let mut functions_by_idx = Vec::new();
    let mut compiled_funs = Vec::new();
    let mut errors = Vec::new();
    for function in module {
        functions_by_idx.push(*functions_by_name.get(&function.name).unwrap());

        match compile_function(function, &mut functions_by_name) {
            Ok(code) => compiled_funs.push(code),
            Err(mut err) => errors.append(&mut err.0) 
        }
    }

    if !errors.is_empty() {
        return Err(CompileErrors(errors))
    }

    let main_idx = match functions_by_name.get(ENTRY_POINT) {
        Some(main) => {
            if main.returns_value {
                return untagged_err!("Entry point cannot return a value");
            }

            if main.arg_count != 0 {
                return untagged_err!("Entry point must have no arguments")
            }

            main.id
        },
        None => return untagged_err!("No entry point found: A zero-arg function returning void called {ENTRY_POINT} should be created"),
    };

    // Now need to link it, steps:
    // Write all functions one-by-one into a new array of instructions, offsetting the jump instructions in the function by the start of that function
    // Keep track of the start index of each function
    // Overwrite JSR instructions with the correct index to jump to

    let mut program = vec![
        Instruction::JumpSubRoutine(main_idx),
        Instruction::Jump(-1)
    ];


    // Write in all the functions, applying necessary offsets.
    for idx in 0..functions_by_name.len() {
        let offset = program.len() as i32;
        functions_by_idx[idx].start_offset = offset;

        for instruction in &compiled_funs[idx] {
            let offset_instruction = match *instruction {
                Instruction::Jump(addr) => Instruction::Jump(addr + offset),
                Instruction::JumpIfZero(addr) => Instruction::JumpIfZero(addr + offset),
                Instruction::JumpIfNonZero(addr) => Instruction::JumpIfNonZero(addr + offset),
                _ => *instruction
            };

            program.push(offset_instruction);
        }
    }
    

    // Overwrite JSR instructions
    for instruction in program.iter_mut() {
        if let Instruction::JumpSubRoutine(idx) = instruction {
            *instruction = Instruction::JumpSubRoutine(functions_by_idx[*idx as usize].start_offset + 1)
        }
    }

    Ok(program)
}

fn emit_block(block: Vec<Statement>, ctx: &mut CompileCtx) -> CompileResult<()> {
    let mut errors = Vec::new();

    for statement in block {
        if let Err(mut err) = emit_statement(statement, ctx) {
            errors.append(&mut err.0);
        }
    }

    if errors.is_empty() {
        Ok(())
    }   else    {
        Err(CompileErrors(errors))
    }
}

fn emit_statement(statement: Statement, ctx: &mut CompileCtx) -> CompileResult<()> {
    match statement {
        Statement::Assignment { variable_name, value, variable_name_ref } => {
            emit_expression(value, ctx)?;
            match ctx.save_to_variable(variable_name.clone(), variable_name_ref) {
                Ok(_) => {},
                Err(_) => ctx.add_variable(variable_name),
            }

            Ok(())
        },
        Statement::If { segments, r#else } => {
            let mut skip_else_instruction_idxs = Vec::new();

            let last_idx = segments.len() - 1;
            for (idx, segment) in segments.into_iter().enumerate() {
                let is_last = idx == last_idx;

                emit_expression(segment.condition, ctx)?;

                let jump_inst_idx = ctx.instructions.len();
                ctx.emit(Instruction::JumpIfZero(-1)); // TODO: add in address later

                ctx.open_scope(ScopeState::Other);
                emit_block(segment.block, ctx)?;
                ctx.end_scope();

                // After each if segment, add an instruction to skip the else segment
                if !is_last || r#else.is_some() {
                    skip_else_instruction_idxs.push(ctx.instructions.len());
                    ctx.emit(Instruction::Jump(-1)); // TODO: add in address later
                }

                // Skip over the if block if the condition is false
                ctx.instructions[jump_inst_idx] = Instruction::JumpIfZero(ctx.instructions.len() as i32 + 1);
            }

            match r#else {
                Some(else_block) => {
                    ctx.open_scope(ScopeState::Other);
                    emit_block(else_block, ctx)?;
                    ctx.end_scope();

                    for idx in skip_else_instruction_idxs {
                        ctx.instructions[idx] = Instruction::Jump(ctx.instructions.len() as i32 + 1)
                    }
                },
                None => {}
            }
            

            Ok(())
        },
        Statement::While { condition, block } => {
            // Unconditional jump to end of loop
            let uncond_jump_idx = ctx.instructions.len();
            ctx.emit(Instruction::Jump(-1)); // TODO: set address later.

            ctx.open_scope(ScopeState::While {
                continue_inst_addresses: Vec::new(),
                break_inst_addresses: Vec::new()
            });
            emit_block(block, ctx)?;
            let scope_state = ctx.end_scope();

            let (continue_inst_addresses, break_inst_addresses) = match scope_state {
                ScopeState::While { continue_inst_addresses, break_inst_addresses } => (continue_inst_addresses, break_inst_addresses),
                _ => unreachable!()
            };

            let continue_instruction = Instruction::Jump(ctx.instructions.len() as i32 + 1);

            ctx.instructions[uncond_jump_idx] = continue_instruction;
            for addr in continue_inst_addresses {
                ctx.instructions[addr] = continue_instruction;
            }
            
            emit_expression(condition, ctx)?;
            ctx.emit(Instruction::JumpIfNonZero(uncond_jump_idx as i32 + 2));

            let break_instruction = Instruction::Jump(ctx.instructions.len() as i32 + 1);
            for addr in break_inst_addresses {
                ctx.instructions[addr] = break_instruction;
            }
            
            Ok(())
        },
        Statement::Return(position) => if ctx.return_value_save_offset.is_some() {
            error!(position, "Must return a value from this function")
        }   else    {
            Ok(emit_return(ctx))
        },
        Statement::ReturnValue {
            value,
            value_ref
        } => if let Some(offset) = ctx.return_value_save_offset {
            emit_expression(value, ctx)?;

            ctx.emit(Instruction::Save(ctx.stack_size - offset));
            Ok(emit_return(ctx))
        }   else    {
            error!(value_ref, "Cannot return a value from this function")
        },
        Statement::Continue(pos) => try_emit_loop_control_flow(true, pos, ctx),
        Statement::Break(pos) => try_emit_loop_control_flow(false, pos, ctx),
        Statement::Call(call) => emit_call(call, ctx, false),
    }
}

fn try_emit_loop_control_flow(is_continue: bool, keyword_ref: FileRef, ctx: &mut CompileCtx) -> CompileResult<()> {
    // Find the first while loop
    for (scope_idx, scope) in ctx.scopes.iter_mut().enumerate().rev() {
        if let ScopeState::While { ref mut continue_inst_addresses, ref mut break_inst_addresses } = scope.scope_type {
            if is_continue {
                continue_inst_addresses
            }   else {
                break_inst_addresses
            }.push(ctx.instructions.len());

            ctx.prepare_for_premature_scope_end(scope_idx);
            ctx.emit(Instruction::Jump(-1));
            return Ok(());
        }
    }

    error!(keyword_ref, "Not in a loop scope - cannot use break or continue keywords")
}

fn emit_return(ctx: &mut CompileCtx) {
    ctx.prepare_for_premature_scope_end(0);
    ctx.emit(Instruction::Return);
}

fn emit_call(call: Call, ctx: &mut CompileCtx, using_return_value: bool) -> CompileResult<()> {
    let info = *match ctx.function_ids_in_module.get(&call.function_name) {
        Some(info) => info,
        None => return error!(call.function_name_ref, "No function exists with name {}", call.function_name)
    };

    if !info.returns_value && using_return_value {
        return error!(call.function_name_ref, "Cannot use a function that does not return a value within an expression");
    }
    
    if info.arg_count != call.arguments.len() {
        return error!(call.arguments_ref, "Wrong number of arguments, expected {}, got {}", info.arg_count, call.arguments.len());
    }

    if info.returns_value {
        ctx.emit(Instruction::Constant(0)); // Add space for the return value
    }

    let arg_count = call.arguments.len();
    for expr in call.arguments {
        emit_expression(expr, ctx)?;
    }

    ctx.emit(Instruction::JumpSubRoutine(info.id)); // This will be overwritten with the correct address in the linking stage

    for _ in 0..arg_count {
        ctx.emit(Instruction::Pop);
    }

    // Get rid of return value if not needed
    if !using_return_value && info.returns_value {
        ctx.emit(Instruction::Pop);
    }

    Ok(())
}

fn emit_expression(expr: Expression, ctx: &mut CompileCtx) -> CompileResult<()> {
    match expr {
        Expression::Binary { left, right, operator } => {
            emit_expression(*right, ctx)?;
            emit_expression(*left, ctx)?;

            ctx.emit(match operator {
                BinaryOperator::Add => Instruction::Add,
                BinaryOperator::Subtract => Instruction::Subtract,
                BinaryOperator::Multiply => Instruction::Multiply,
                BinaryOperator::Divide => Instruction::Divide,
                BinaryOperator::And => Instruction::And,
                BinaryOperator::Or => Instruction::Or,
                BinaryOperator::Xor => Instruction::Multiply,
                BinaryOperator::ShiftLeft => Instruction::ShiftLeft,
                BinaryOperator::ShiftRight => Instruction::ShiftRight,
                BinaryOperator::Equals => Instruction::Equal,
                BinaryOperator::NotEquals => Instruction::NotEqual,
                BinaryOperator::GreaterThan => Instruction::GreaterThan,
                BinaryOperator::LessThan => Instruction::LessThan,
                BinaryOperator::GreaterThanOrEqual => Instruction::GreaterThanOrEqual,
                BinaryOperator::Remainder => Instruction::Remainder,
                BinaryOperator::LessThanOrEqual => Instruction::LessThanOrEqual,
                BinaryOperator::Power => Instruction::Power
            });
        },
        Expression::Unary { value, operator } => {
            match operator {
                UnaryOperator::Not => { 
                    emit_expression(*value, ctx)?;
                    ctx.emit(Instruction::Not)
                },
                UnaryOperator::Negate => {
                    match &*value {
                        Expression::Literal(value) => ctx.emit(Instruction::Constant(-value)),
                        _ => {
                            ctx.emit(Instruction::Constant(-1));
                            emit_expression(*value, ctx)?;

                            ctx.emit(Instruction::Multiply);
                        }
                    }

                    
                }
            }
        },
        Expression::Call(call) => emit_call(call, ctx, true)?,
        Expression::Variable {
            name,
            pos
        } => ctx.load_from_variable(name, pos)?,
        Expression::Literal(value) => ctx.emit(Instruction::Constant(value)),
    };

    Ok(())
}
