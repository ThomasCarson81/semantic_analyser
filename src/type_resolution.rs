use crate::AnalysisError;
use syntax::{
    ast::{Expression, Program, Statement},
    types::{EnumVariantFields, Type, TypeDefinitionKind, TypeTable},
};

pub fn resolve_types(
    program: &mut Program,
    type_table: &mut TypeTable,
) -> Result<(), AnalysisError> {
    for statement in &mut program.statements {
        type_resolve_statement(statement, type_table)?;
    }

    // TypeTable contains clones created during collection, so keep those
    // definitions in sync with the resolved definitions in the program.
    for statement in &program.statements {
        if let Statement::TypeDefinition(type_definition) = statement {
            if let Some(collected_definition) = type_table
                .types
                .iter_mut()
                .find(|definition| definition.id == type_definition.id)
            {
                *collected_definition = type_definition.clone();
            }
        }
    }

    Ok(())
}

pub fn type_resolve_statement(
    statement: &mut Statement,
    type_table: &TypeTable,
) -> Result<(), AnalysisError> {
    match statement {
        Statement::Let { var_type, .. } => {
            if let Some(inner_type) = var_type {
                *var_type = Some(resolve_type(inner_type, type_table, &Vec::new())?);
            }
        }
        Statement::Assignment {
            identifier: _,
            expression,
        } => {
            type_resolve_expression(expression, type_table)?;
        }
        Statement::If {
            condition,
            body,
            else_block,
        } => {
            type_resolve_expression(condition, type_table)?;
            type_resolve_block(body, type_table)?;
            if let Some(else_block) = else_block {
                type_resolve_block(else_block, type_table)?;
            }
        }
        Statement::While { condition, body } => {
            type_resolve_expression(condition, type_table)?;
            type_resolve_block(body, type_table)?;
        }
        Statement::For {
            initialiser,
            condition,
            increment,
            body,
        } => {
            if let Some(init) = initialiser {
                type_resolve_statement(init, type_table)?;
            }
            if let Some(condition) = condition {
                type_resolve_expression(condition, type_table)?;
            }
            type_resolve_block(body, type_table)?;
            if let Some(increment) = increment {
                type_resolve_statement(increment, type_table)?;
            }
        }
        Statement::Scope { body } => {
            type_resolve_block(body, type_table)?;
        }
        Statement::FunctionDefinition {
            identifier: _,
            parameters,
            body,
            return_type,
        } => {
            type_resolve_block(body, type_table)?;
            for param in parameters {
                param.param_type = resolve_type(&mut param.param_type, type_table, &Vec::new())?;
            }
            *return_type = resolve_type(return_type, type_table, &Vec::new())?;
        }
        Statement::ExpressionStatement { expression } => {
            type_resolve_expression(expression, type_table)?;
        }
        Statement::Return { value } => {
            if let Some(expression) = value {
                type_resolve_expression(expression, type_table)?;
            }
        }
        Statement::TypeDefinition(type_definition) => match &mut type_definition.kind {
            TypeDefinitionKind::Struct(struct_definition) => {
                for field in &mut struct_definition.fields {
                    field.field_type = resolve_type(
                        &mut field.field_type,
                        type_table,
                        &type_definition.parameters,
                    )?;
                }
            }
            TypeDefinitionKind::Enum(enum_definition) => {
                for variant in &mut enum_definition.variants {
                    match &mut variant.fields {
                        EnumVariantFields::Unit => {}
                        EnumVariantFields::Tuple(items) => {
                            for item in items {
                                *item =
                                    resolve_type(item, type_table, &type_definition.parameters)?;
                            }
                        }
                        EnumVariantFields::Struct(struct_fields) => {
                            for field in struct_fields {
                                resolve_type(
                                    &mut field.field_type,
                                    type_table,
                                    &type_definition.parameters,
                                )?;
                            }
                        }
                    }
                }
            }
            TypeDefinitionKind::BuiltIn(_) => {
                unreachable!("Built-in types should not be defined by the user")
            }
        },
        Statement::Break | Statement::Continue | Statement::Pass => (),
    }
    Ok(())
}

pub fn type_resolve_expression(
    expression: &mut Expression,
    type_table: &TypeTable,
) -> Result<(), AnalysisError> {
    match expression {
        Expression::BinaryOp {
            left,
            operator: _,
            right,
            inferred_type: _,
        } => {
            type_resolve_expression(left, type_table)?;
            type_resolve_expression(right, type_table)?;
        }
        Expression::UnaryOp {
            operator: _,
            operand,
            inferred_type: _,
        } => {
            type_resolve_expression(operand, type_table)?;
        }
        Expression::UnresolvedFunctionCall {
            identifier: _,
            arguments,
        } => {
            for arg in arguments {
                type_resolve_expression(arg, type_table)?;
            }
        }
        _ => {}
    }

    Ok(())
}

pub fn type_resolve_block(
    block: &mut Vec<Statement>,
    type_table: &TypeTable,
) -> Result<(), AnalysisError> {
    for statement in block {
        type_resolve_statement(statement, type_table)?;
    }
    Ok(())
}

pub fn resolve_type(
    ty: &mut Type,
    type_table: &TypeTable,
    generic_type_names: &Vec<String>,
) -> Result<Type, AnalysisError> {
    let mut resolved_type = ty.clone();
    println!("Resolving type: {:?}", ty);
    match ty {
        Type::Named { name, arguments } => {
            if arguments.len() == 0 && Type::from_str(name).is_some() {
                resolved_type = Type::from_str(name).unwrap();
                return Ok(resolved_type);
            }

            if type_table.get_by_name(name).is_none() {
                if generic_type_names.contains(name) {
                    resolved_type = Type::GenericParameter(name.clone());
                    return Ok(resolved_type);
                }
                return Err(AnalysisError::UndeclaredType(name.clone()));
            }
            for arg in arguments {
                *arg = resolve_type(arg, type_table, generic_type_names)?;
            }
        }
        Type::Tuple(items) => {
            for item in items {
                *item = resolve_type(item, type_table, generic_type_names)?;
            }
        }
        _ => {}
    }
    Ok(resolved_type)
}
