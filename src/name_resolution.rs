use crate::{AnalysisError, SemanticEnvironment};
use syntax::{
    ast::{Expression, Program, Statement},
    functions::FunctionTable,
};

pub fn resolve_names(
    program: &mut Program,
    environment: &mut SemanticEnvironment,
    func_table: &FunctionTable,
) -> Result<(), AnalysisError> {
    // Resolve globals first so every function sees the complete global scope,
    // regardless of where its definition appears in the source file.
    for statement in &mut program.statements {
        if matches!(statement, Statement::Let { .. }) {
            resolve_statement(statement, environment, func_table)?;
        }
    }
    for statement in &mut program.statements {
        if matches!(statement, Statement::FunctionDefinition { .. }) {
            resolve_statement(statement, environment, func_table)?;
        }
    }
    for statement in &mut program.statements {
        if !matches!(
            statement,
            Statement::Let { .. } | Statement::FunctionDefinition { .. }
        ) {
            resolve_statement(statement, environment, func_table)?;
        }
    }
    Ok(())
}

pub fn resolve_statement(
    statement: &mut Statement,
    environment: &mut SemanticEnvironment,
    func_table: &FunctionTable,
) -> Result<(), AnalysisError> {
    match statement {
        Statement::Let {
            identifier,
            expression,
            mutable: _,
            var_type: _,
        } => {
            resolve_expression(expression, environment, func_table)?;
            environment.define(identifier.clone())?;
        }
        Statement::Assignment {
            identifier,
            expression,
        } => {
            resolve_expression(expression, environment, func_table)?;
            if !environment.get(identifier) {
                return Err(AnalysisError::UndeclaredVariable(identifier.clone()));
            }
        }
        Statement::If {
            condition,
            body,
            else_block,
        } => {
            resolve_expression(condition, environment, func_table)?;
            resolve_block(body, environment, func_table)?;
            if let Some(else_block) = else_block {
                resolve_block(else_block, environment, func_table)?;
            }
        }
        Statement::While { condition, body } => {
            resolve_expression(condition, environment, func_table)?;
            resolve_block(body, environment, func_table)?;
        }
        Statement::For {
            initialiser,
            condition,
            increment,
            body,
        } => {
            environment.push_scope();
            if let Some(init) = initialiser {
                resolve_statement(init, environment, func_table)?;
            }
            if let Some(condition) = condition {
                resolve_expression(condition, environment, func_table)?;
            }
            resolve_block(body, environment, func_table)?;
            if let Some(increment) = increment {
                resolve_statement(increment, environment, func_table)?;
            }
            environment.pop_scope();
        }
        Statement::Scope { body } => {
            resolve_block(body, environment, func_table)?;
        }
        Statement::FunctionDefinition {
            identifier: _,
            parameters,
            body,
            return_type: _,
        } => {
            environment.push_scope();
            for param in parameters {
                environment.define(param.name.clone())?;
            }
            resolve_block(body, environment, func_table)?;
            environment.pop_scope();
        }
        Statement::ExpressionStatement { expression } => {
            resolve_expression(expression, environment, func_table)?;
        }
        Statement::Return { value } => {
            if let Some(expression) = value {
                resolve_expression(expression, environment, func_table)?;
            }
        }
        Statement::Break | Statement::Continue | Statement::Pass => (),
    }
    Ok(())
}

pub fn resolve_expression(
    expression: &mut Expression,
    environment: &SemanticEnvironment,
    func_table: &FunctionTable,
) -> Result<(), AnalysisError> {
    match expression {
        Expression::Literal(_) => {}
        Expression::Identifier(identifier) => {
            if !environment.get(identifier) {
                return Err(AnalysisError::UndeclaredVariable(identifier.clone()));
            }
        }
        Expression::BinaryOp {
            left,
            operator: _,
            right,
        } => {
            resolve_expression(left, environment, func_table)?;
            resolve_expression(right, environment, func_table)?;
        }
        Expression::UnaryOp {
            operator: _,
            operand,
        } => {
            resolve_expression(operand, environment, func_table)?;
        }
        Expression::UnresolvedFunctionCall {
            identifier,
            arguments,
        } => {
            let overloads: Vec<_> = func_table.functions_named(identifier).collect();
            let first_overload = overloads
                .first()
                .ok_or_else(|| AnalysisError::FunctionNotDefined(identifier.clone()))?;
            let parameter_count = first_overload.signature.parameters.len();
            if !overloads
                .iter()
                .any(|function| function.signature.parameters.len() == arguments.len())
            {
                return Err(AnalysisError::FunctionArgumentCountMismatch(
                    identifier.clone(),
                    parameter_count,
                    arguments.len(),
                ));
            }
            for argument in &mut *arguments {
                resolve_expression(argument, environment, func_table)?;
            }
            // Overload selection requires argument types, so retain the
            // unresolved node until the type-checking pass.
        }
        Expression::ResolvedFunctionCall { .. } => {
            unreachable!("function call already resolved before semantic analysis")
        }
    }

    Ok(())
}

pub fn resolve_block(
    block: &mut Vec<Statement>,
    environment: &mut SemanticEnvironment,
    func_table: &FunctionTable,
) -> Result<(), AnalysisError> {
    environment.push_scope();
    for statement in block {
        resolve_statement(statement, environment, func_table)?;
    }
    environment.pop_scope();
    Ok(())
}
