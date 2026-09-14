use syntax::{
    ast::{Expression, Program, Statement, TokenType},
    functions::FunctionTable,
    semantics::Symbol,
    types::{Number, Type, Value},
};

use crate::{AnalysisError, SemanticEnvironment};

pub fn check_types(
    program: &mut Program,
    environment: &mut SemanticEnvironment,
    func_table: &FunctionTable,
) -> Result<(), AnalysisError> {
    for statement in &mut program.statements {
        if matches!(statement, Statement::Let { .. }) {
            check_statement_types(statement, environment, None, func_table)?;
        }
    }
    // globals must be type checked before functions so they're available inside the functions
    for statement in &mut program.statements {
        if matches!(statement, Statement::FunctionDefinition { .. }) {
            check_statement_types(statement, environment, None, func_table)?;
        }
    }
    Ok(())
}

pub fn check_statement_types(
    statement: &mut Statement,
    environment: &mut SemanticEnvironment,
    expected_return_type: Option<Type>,
    func_table: &FunctionTable,
) -> Result<(), AnalysisError> {
    match statement {
        Statement::Let {
            identifier,
            expression,
            mutable,
            var_type,
        } => {
            let expr_type = check_expression_types(expression, environment, *var_type, func_table)?;
            let symbol = Symbol {
                mutable: *mutable,
                var_type: expr_type,
            };
            *var_type = Some(expr_type);
            environment.define_typed(identifier.clone(), symbol)?;
        }
        Statement::Assignment {
            identifier,
            expression,
        } => {
            let stored_var = environment.get_typed(identifier);
            if !stored_var.mutable {
                return Err(AnalysisError::CannotMutate(identifier.clone()));
            }
            check_expression_types(
                expression,
                environment,
                Some(stored_var.var_type),
                func_table,
            )?;
        }
        Statement::If {
            condition,
            body,
            else_block,
        } => {
            check_expression_types(condition, environment, Some(Type::Bool), func_table)?;
            check_block_types(body, environment, None, func_table)?;
            if let Some(else_block) = else_block {
                check_block_types(else_block, environment, None, func_table)?;
            }
        }
        Statement::While { condition, body } => {
            check_expression_types(condition, environment, Some(Type::Bool), func_table)?;
            check_block_types(body, environment, None, func_table)?;
        }
        Statement::For {
            initialiser,
            condition,
            increment,
            body,
        } => {
            environment.push_scope_typed();
            if let Some(initialiser) = initialiser {
                check_statement_types(initialiser, environment, None, func_table)?;
            }
            if let Some(condition) = condition {
                check_expression_types(condition, environment, Some(Type::Bool), func_table)?;
            }
            if let Some(increment) = increment {
                check_statement_types(increment, environment, None, func_table)?;
            }
            check_block_types(body, environment, None, func_table)?;
            environment.pop_scope_typed();
        }
        Statement::Scope { body } => {
            check_block_types(body, environment, None, func_table)?;
        }
        Statement::FunctionDefinition {
            identifier: _,
            parameters,
            body,
            return_type,
        } => {
            environment.push_scope_typed();
            for param in parameters {
                let param_symbol = Symbol {
                    mutable: false,
                    var_type: param.param_type,
                };
                environment.define_typed(param.name.clone(), param_symbol)?;
            }
            check_block_types(body, environment, Some(*return_type), func_table)?;
            environment.pop_scope_typed();
        }
        Statement::ExpressionStatement { expression } => {
            check_expression_types(expression, environment, None, func_table)?;
        }
        Statement::Return { value } => {
            if let Some(expr) = value {
                check_expression_types(expr, environment, expected_return_type, func_table)?;
            }
        }
        Statement::Break | Statement::Continue | Statement::Pass => {}
    }

    Ok(())
}

pub fn check_expression_types(
    expression: &mut Expression,
    environment: &SemanticEnvironment,
    expected_type: Option<Type>,
    func_table: &FunctionTable,
) -> Result<Type, AnalysisError> {
    Ok(match expression {
        Expression::Literal(value) => match value {
            Value::Unit => Type::Unit,
            Value::String(_) => Type::String,
            Value::Bool(_) => Type::Bool,
            Value::Number(number) => match number {
                Number::IntegerLiteral(value) => {
                    let (resolved, ty) = resolve_integer_literal(*value, expected_type)?;

                    *number = resolved;
                    ty
                }

                Number::FloatLiteral(value) => {
                    let (resolved, ty) = resolve_float_literal(*value, expected_type)?;

                    *number = resolved;
                    ty
                }

                Number::I8(_) => Type::I8,
                Number::I16(_) => Type::I16,
                Number::I32(_) => Type::I32,
                Number::I64(_) => Type::I64,
                Number::U8(_) => Type::U8,
                Number::U16(_) => Type::U16,
                Number::U32(_) => Type::U32,
                Number::U64(_) => Type::U64,
                Number::F32(_) => Type::F32,
                Number::F64(_) => Type::F64,
            },
        },
        Expression::Identifier(name) => environment.get_typed(name).var_type,
        Expression::BinaryOp {
            left,
            operator,
            right,
            inferred_type,
        } => {
            let expected_operand_type = if binary_operator_uses_operand_type(operator) {
                expected_type
            } else {
                None
            };
            let left_mutable = match left.as_ref() {
                Expression::Identifier(ident) => environment.get_typed(ident).mutable,
                _ => false,
            };
            let left_name = match left.as_ref() {
                Expression::Identifier(ident) => Some(ident.clone()),
                _ => None,
            };
            let left_type =
                check_expression_types(left, environment, expected_operand_type, func_table)?;
            let right_type =
                check_expression_types(right, environment, expected_operand_type, func_table)?;
            let result_type = check_binary_operator_types(
                &left_type,
                operator,
                &right_type,
                left_mutable,
                left_name,
            )?;
            if let Some(expected_type) = expected_type {
                if result_type != expected_type {
                    return Err(AnalysisError::TypeMismatch(expected_type, result_type));
                }
            }
            *inferred_type = Some(left_type);
            result_type
        }
        Expression::UnaryOp {
            operator,
            operand,
            inferred_type,
        } => {
            if *operator == TokenType::Minus {
                if let Expression::Literal(Value::Number(Number::IntegerLiteral(value))) =
                    operand.as_mut()
                {
                    let value = -*value;

                    let (resolved_number, resolved_type) =
                        resolve_integer_literal(value, expected_type)?;

                    *operand = Box::new(Expression::Literal(Value::Number(resolved_number)));
                    *inferred_type = Some(resolved_type);
                    return Ok(resolved_type);
                }
            }
            let operand_mutable = match operand.as_ref() {
                Expression::Identifier(ident) => environment.get_typed(ident).mutable,
                _ => false,
            };
            let operand_name = match operand.as_ref() {
                Expression::Identifier(ident) => Some(ident.clone()),
                _ => None,
            };
            let operand_type =
                check_expression_types(operand, environment, expected_type, func_table)?;

            *inferred_type = Some(check_unary_operator_types(
                operator,
                &operand_type,
                operand_mutable,
                operand_name,
            )?);
            operand_type
        }
        Expression::ResolvedFunctionCall {
            function_id,
            arguments,
        } => {
            let function = func_table
                .get(&function_id)
                .ok_or_else(|| AnalysisError::FunctionNotDefined(function_id.0.to_string()))?;
            for (parameter_type, argument) in function.signature.parameters.iter().zip(arguments) {
                let argument_type = check_expression_types(
                    argument,
                    environment,
                    Some(*parameter_type),
                    func_table,
                )?;
                if *parameter_type != argument_type {
                    return Err(AnalysisError::TypeMismatch(*parameter_type, argument_type));
                }
            }
            if let Some(expected_type) = expected_type {
                if function.signature.return_type != expected_type {
                    return Err(AnalysisError::TypeMismatch(
                        expected_type,
                        function.signature.return_type,
                    ));
                }
            }
            function.signature.return_type
        }
        Expression::UnresolvedFunctionCall {
            identifier,
            arguments,
        } => {
            let candidates: Vec<_> = func_table
                .functions_named(identifier)
                .filter(|function| function.signature.parameters.len() == arguments.len())
                .collect();
            let mut first_error = None;
            let mut selected = None;

            for function in candidates {
                let mut candidate_arguments = arguments.clone();
                let result = function
                    .signature
                    .parameters
                    .iter()
                    .zip(&mut candidate_arguments)
                    .try_for_each(|(parameter_type, argument)| {
                        check_expression_types(
                            argument,
                            environment,
                            Some(*parameter_type),
                            func_table,
                        )
                        .and_then(|argument_type| {
                            if argument_type == *parameter_type {
                                Ok(())
                            } else {
                                Err(AnalysisError::TypeMismatch(*parameter_type, argument_type))
                            }
                        })
                    });

                match result {
                    Ok(())
                        if expected_type
                            .is_none_or(|expected| expected == function.signature.return_type) =>
                    {
                        selected = Some(function.id.clone());
                        break;
                    }
                    Ok(()) => {
                        first_error = Some(AnalysisError::TypeMismatch(
                            expected_type.expect("guarded by is_none_or"),
                            function.signature.return_type,
                        ));
                    }
                    Err(error) if first_error.is_none() => first_error = Some(error),
                    Err(_) => {}
                }
            }

            let function_id = selected.ok_or_else(|| {
                first_error.unwrap_or_else(|| AnalysisError::FunctionNotDefined(identifier.clone()))
            })?;
            let function = func_table
                .get(&function_id)
                .expect("selected function must exist in the function table");
            for (parameter_type, argument) in
                function.signature.parameters.iter().zip(&mut *arguments)
            {
                check_expression_types(argument, environment, Some(*parameter_type), func_table)?;
            }
            let return_type = function.signature.return_type;
            let arguments = std::mem::take(arguments);
            *expression = Expression::ResolvedFunctionCall {
                function_id,
                arguments,
            };
            return_type
        }
    })
}

pub fn check_block_types(
    block: &mut Vec<Statement>,
    environment: &mut SemanticEnvironment,
    expected_return_type: Option<Type>,
    func_table: &FunctionTable,
) -> Result<(), AnalysisError> {
    environment.push_scope_typed();
    for statement in block {
        check_statement_types(statement, environment, expected_return_type, func_table)?;
    }
    environment.pop_scope_typed();
    Ok(())
}

pub fn check_binary_operator_types(
    left: &Type,
    operator: &TokenType,
    right: &Type,
    left_mutable: bool,
    left_name: Option<String>,
) -> Result<Type, AnalysisError> {
    match operator {
        TokenType::Plus => check_plus_types(left, operator, right),
        TokenType::Minus => check_number_only_bin_op_types(left, operator, right),
        TokenType::Asterisk => check_number_only_bin_op_types(left, operator, right),
        TokenType::Slash => check_number_only_bin_op_types(left, operator, right),
        TokenType::Modulo => check_number_only_bin_op_types(left, operator, right),
        TokenType::GreaterThan => check_order_comparison_op_types(left, operator, right),
        TokenType::LessThan => check_order_comparison_op_types(left, operator, right),
        TokenType::GreaterThanOrEqualTo => check_order_comparison_op_types(left, operator, right),
        TokenType::LessThanOrEqualTo => check_order_comparison_op_types(left, operator, right),
        TokenType::EqualTo => check_equivalence_comparison_op_types(left, operator, right),
        TokenType::NotEqualTo => check_equivalence_comparison_op_types(left, operator, right),
        TokenType::LogicalAnd => check_logical_bin_op_types(left, operator, right),
        TokenType::LogicalOr => check_logical_bin_op_types(left, operator, right),
        TokenType::Walrus => {
            if !left_mutable {
                return Err(AnalysisError::CannotMutate(
                    left_name.expect("missing left operand name"),
                ));
            }
            check_walrus_types(left, operator, right)
        }
        _ => unreachable!(
            "Invalid binary operator during type checking: '{}'.",
            operator
        ),
    }
}

pub fn check_plus_types(
    left: &Type,
    operator: &TokenType,
    right: &Type,
) -> Result<Type, AnalysisError> {
    match (left, right) {
        (Type::U8, Type::U8)
        | (Type::U16, Type::U16)
        | (Type::U32, Type::U32)
        | (Type::U64, Type::U64)
        | (Type::I8, Type::I8)
        | (Type::I16, Type::I16)
        | (Type::I32, Type::I32)
        | (Type::I64, Type::I64)
        | (Type::F32, Type::F32)
        | (Type::F64, Type::F64)
        | (Type::String, Type::String) => Ok(*left),
        _ => Err(AnalysisError::BinOpTypeError(*operator, *left, *right)),
    }
}

pub fn check_number_only_bin_op_types(
    left: &Type,
    operator: &TokenType,
    right: &Type,
) -> Result<Type, AnalysisError> {
    match (left, right) {
        (Type::U8, Type::U8)
        | (Type::U16, Type::U16)
        | (Type::U32, Type::U32)
        | (Type::U64, Type::U64)
        | (Type::I8, Type::I8)
        | (Type::I16, Type::I16)
        | (Type::I32, Type::I32)
        | (Type::I64, Type::I64)
        | (Type::F32, Type::F32)
        | (Type::F64, Type::F64) => Ok(*left),
        _ => Err(AnalysisError::BinOpTypeError(*operator, *left, *right)),
    }
}

pub fn check_order_comparison_op_types(
    left: &Type,
    operator: &TokenType,
    right: &Type,
) -> Result<Type, AnalysisError> {
    match (left, right) {
        (Type::U8, Type::U8)
        | (Type::U16, Type::U16)
        | (Type::U32, Type::U32)
        | (Type::U64, Type::U64)
        | (Type::I8, Type::I8)
        | (Type::I16, Type::I16)
        | (Type::I32, Type::I32)
        | (Type::I64, Type::I64)
        | (Type::F32, Type::F32)
        | (Type::F64, Type::F64) => Ok(Type::Bool),
        _ => Err(AnalysisError::BinOpTypeError(*operator, *left, *right)),
    }
}

pub fn check_equivalence_comparison_op_types(
    left: &Type,
    operator: &TokenType,
    right: &Type,
) -> Result<Type, AnalysisError> {
    match (left, right) {
        (Type::U8, Type::U8)
        | (Type::U16, Type::U16)
        | (Type::U32, Type::U32)
        | (Type::U64, Type::U64)
        | (Type::I8, Type::I8)
        | (Type::I16, Type::I16)
        | (Type::I32, Type::I32)
        | (Type::I64, Type::I64)
        | (Type::F32, Type::F32)
        | (Type::F64, Type::F64)
        | (Type::Bool, Type::Bool)
        | (Type::String, Type::String) => Ok(Type::Bool),
        _ => Err(AnalysisError::BinOpTypeError(*operator, *left, *right)),
    }
}

pub fn check_logical_bin_op_types(
    left: &Type,
    operator: &TokenType,
    right: &Type,
) -> Result<Type, AnalysisError> {
    match (left, right) {
        (Type::Bool, Type::Bool) => Ok(Type::Bool),
        _ => Err(AnalysisError::BinOpTypeError(*operator, *left, *right)),
    }
}

pub fn check_walrus_types(
    left: &Type,
    operator: &TokenType,
    right: &Type,
) -> Result<Type, AnalysisError> {
    if left != right {
        return Err(AnalysisError::BinOpTypeError(*operator, *left, *right));
    }
    Ok(*left)
}

pub fn check_unary_operator_types(
    operator: &TokenType,
    operand: &Type,
    operand_mutable: bool,
    operand_name: Option<String>,
) -> Result<Type, AnalysisError> {
    match operator {
        TokenType::Increment => {
            if !operand_mutable {
                return Err(AnalysisError::CannotMutate(String::from(
                    operand_name.expect("missing operand name in unary op mut check"),
                )));
            }
            check_number_only_un_op_types(operator, operand)
        }
        TokenType::Decrement => {
            if !operand_mutable {
                return Err(AnalysisError::CannotMutate(String::from(
                    operand_name.expect("missing operand name in unary op mut check"),
                )));
            }
            check_number_only_un_op_types(operator, operand)
        }
        TokenType::Not => check_bool_only_un_op_types(operator, operand),
        TokenType::Minus => check_number_only_un_op_types(operator, operand),
        _ => unreachable!("Invalid unary operator during type checking: {}", operator),
    }
}

pub fn check_number_only_un_op_types(
    operator: &TokenType,
    operand: &Type,
) -> Result<Type, AnalysisError> {
    match operand {
        Type::U8
        | Type::U16
        | Type::U32
        | Type::U64
        | Type::I8
        | Type::I16
        | Type::I32
        | Type::I64
        | Type::F32
        | Type::F64 => Ok(*operand),
        _ => Err(AnalysisError::UnOpTypeError(*operator, *operand)),
    }
}

pub fn check_bool_only_un_op_types(
    operator: &TokenType,
    operand: &Type,
) -> Result<Type, AnalysisError> {
    match operand {
        Type::Bool => Ok(*operand),
        _ => Err(AnalysisError::UnOpTypeError(*operator, *operand)),
    }
}

pub fn binary_operator_uses_operand_type(operator: &TokenType) -> bool {
    match operator {
        TokenType::Plus
        | TokenType::Minus
        | TokenType::Asterisk
        | TokenType::Slash
        | TokenType::Walrus
        | TokenType::Modulo => true,
        TokenType::GreaterThan
        | TokenType::LessThan
        | TokenType::GreaterThanOrEqualTo
        | TokenType::LessThanOrEqualTo
        | TokenType::EqualTo
        | TokenType::NotEqualTo
        | TokenType::LogicalAnd
        | TokenType::LogicalOr => false,
        _ => unreachable!("not a binary operator: {}", operator),
    }
}

fn resolve_integer_literal(
    value: i64,
    expected_type: Option<Type>,
) -> Result<(Number, Type), AnalysisError> {
    let target_type = expected_type.unwrap_or(Type::I32);

    let number = match target_type {
        Type::I8 => Number::I8(
            i8::try_from(value)
                .map_err(|_| AnalysisError::IntegerLiteralOutOfRange(value, target_type))?,
        ),
        Type::I16 => Number::I16(
            i16::try_from(value)
                .map_err(|_| AnalysisError::IntegerLiteralOutOfRange(value, target_type))?,
        ),
        Type::I32 => Number::I32(
            i32::try_from(value)
                .map_err(|_| AnalysisError::IntegerLiteralOutOfRange(value, target_type))?,
        ),
        Type::I64 => Number::I64(value),

        Type::U8 => Number::U8(
            u8::try_from(value)
                .map_err(|_| AnalysisError::IntegerLiteralOutOfRange(value, target_type))?,
        ),
        Type::U16 => Number::U16(
            u16::try_from(value)
                .map_err(|_| AnalysisError::IntegerLiteralOutOfRange(value, target_type))?,
        ),
        Type::U32 => Number::U32(
            u32::try_from(value)
                .map_err(|_| AnalysisError::IntegerLiteralOutOfRange(value, target_type))?,
        ),
        Type::U64 => Number::U64(
            u64::try_from(value)
                .map_err(|_| AnalysisError::IntegerLiteralOutOfRange(value, target_type))?,
        ),

        _ => {
            return Err(AnalysisError::TypeMismatch(target_type, Type::I32));
        }
    };

    Ok((number, target_type))
}

fn resolve_float_literal(
    value: f64,
    expected_type: Option<Type>,
) -> Result<(Number, Type), AnalysisError> {
    let target_type = expected_type.unwrap_or(Type::F64);

    let number = match target_type {
        Type::F32 => Number::F32(value as f32),
        Type::F64 => Number::F64(value),

        _ => {
            return Err(AnalysisError::TypeMismatch(target_type, Type::F64));
        }
    };

    Ok((number, target_type))
}
