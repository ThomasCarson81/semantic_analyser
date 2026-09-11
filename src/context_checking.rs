use crate::AnalysisError;

use syntax::{
    ast::Statement,
    functions::{Function, FunctionImplementation, FunctionTable},
    types::ControlFlow,
};

pub fn check_contexts(func_table: &FunctionTable) -> Result<(), AnalysisError> {
    for function in &func_table.functions {
        check_function_context(function)?;
    }
    Ok(())
}

fn check_function_context(function: &Function) -> Result<(), AnalysisError> {
    let FunctionImplementation::Oxide(body) = &function.implementation else {
        return Ok(());
    };
    check_block_context(&body.statements, false)
}

fn check_block_context(statements: &[Statement], in_loop: bool) -> Result<(), AnalysisError> {
    let mut statements_iter = statements.iter().peekable();
    while let Some(statement) = statements_iter.next() {
        match check_statement_context(statement, in_loop)? {
            ControlFlow::Normal => {}
            _ => {
                if !statements_iter.peek().is_none() {
                    return Err(AnalysisError::InvalidContext(
                        "Statements not allowed after control flow statement.".to_string(),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn check_statement_context(
    statement: &Statement,
    in_loop: bool,
) -> Result<ControlFlow, AnalysisError> {
    match statement {
        Statement::While { body, .. } => {
            check_block_context(body, true)?;
        }

        Statement::For {
            body,
            initialiser,
            condition: _,
            increment,
        } => {
            if let Some(init) = initialiser {
                check_statement_context(init, false)?;
            }
            if let Some(inc) = increment {
                check_statement_context(inc, false)?;
            }
            check_block_context(body, true)?;
        }

        Statement::If {
            body, else_block, ..
        } => {
            check_block_context(body, in_loop)?;
            if let Some(else_block) = else_block {
                check_block_context(else_block, in_loop)?;
            }
        }

        Statement::Scope { body } => {
            check_block_context(body, in_loop)?;
        }

        Statement::Break | Statement::Continue => {
            if !in_loop {
                return Err(AnalysisError::InvalidContext(
                    "Break or Continue statement found outside of a loop.".to_string(),
                ));
            }
            if matches!(statement, Statement::Break) {
                return Ok(ControlFlow::Break);
            } else {
                return Ok(ControlFlow::Continue);
            }
        }
        Statement::Return { .. } => {
            // return value doesn't matter
            return Ok(ControlFlow::Return(syntax::types::Value::Unit));
        }
        _ => {}
    }
    Ok(ControlFlow::Normal)
}
