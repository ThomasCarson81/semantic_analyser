use std::{
    collections::{HashMap, HashSet},
    env,
};
use syntax::{
    Expression, FunctionSymbol, Number, Program, Statement, Symbol, TokenType, Type, Value,
};

enum AnalysisError {
    OutOfScopes,
    VariableAlreadyDeclared(String),
    UndeclaredVariable(String),
    FunctionAlreadyDefined(String),
    FunctionNotDefined(String),
    FunctionArgumentCountMismatch(String, usize, usize),
    TypeMismatch(Type, Type),
    BinOpTypeError(TokenType, Type, Type),
    UnOpTypeError(TokenType, Type),
    IntegerLiteralOutOfRange(i64, Type),
    FloatLiteralOutOfRange(f64, Type),
    CannotMutate(String),
    CouldNotReadFile(String),
}

struct SemanticEnvironment {
    scopes: Vec<HashSet<String>>,
    scopes_typed: Vec<HashMap<String, Symbol>>,
    functions: HashMap<String, FunctionSymbol>,
}
impl SemanticEnvironment {
    fn new() -> Self {
        Self {
            scopes: vec![HashSet::new()],
            scopes_typed: vec![HashMap::new()],
            functions: HashMap::new(),
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashSet::new());
    }

    fn push_scope_typed(&mut self) {
        self.scopes_typed.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn pop_scope_typed(&mut self) {
        self.scopes_typed.pop();
    }

    fn define(&mut self, name: String) -> Result<(), AnalysisError> {
        let scope = self.scopes.last_mut().ok_or(AnalysisError::OutOfScopes)?;

        if !scope.insert(name.clone()) {
            return Err(AnalysisError::VariableAlreadyDeclared(name));
        }
        Ok(())
    }

    fn define_typed(&mut self, name: String, symbol: Symbol) -> Result<(), AnalysisError> {
        let scope = self
            .scopes_typed
            .last_mut()
            .ok_or(AnalysisError::OutOfScopes)?;

        if scope.contains_key(&name) {
            return Err(AnalysisError::VariableAlreadyDeclared(name));
        }

        scope.insert(name.clone(), symbol);
        Ok(())
    }

    fn get(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|scope| scope.contains(name))
    }

    fn get_typed(&self, name: &str) -> Option<&Symbol> {
        self.scopes_typed
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
    }

    fn define_func(&mut self, name: String, symbol: FunctionSymbol) -> Result<(), AnalysisError> {
        if self.functions.contains_key(&name) {
            return Err(AnalysisError::FunctionAlreadyDefined(name));
        }
        self.functions.insert(name, symbol);
        Ok(())
    }

    fn get_func(&self, name: &String) -> Result<&FunctionSymbol, AnalysisError> {
        self.functions
            .get(name)
            .ok_or(AnalysisError::FunctionNotDefined(name.clone()))
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Please provide a file to analyse.");
        return;
    }
    let program = match get_ast_from_file(&args[1]) {
        Ok(ast) => ast,
        Err(e) => {
            match e {
                AnalysisError::CouldNotReadFile(s) => {
                    println!("Error reading file: {}", s);
                }
                _ => unreachable!("Unexpected error when reading ast from file."),
            }
            std::process::exit(1);
        }
    };
    let mut environment = SemanticEnvironment::new();

    match collect_functions(&program, &mut environment) {
        Ok(_) => {}
        Err(e) => {
            match e {
                AnalysisError::FunctionAlreadyDefined(name) => {
                    println!(
                        "Function '{name}' already defined. Overrides are not currently supported."
                    );
                }
                _ => unreachable!(
                    "Semantic Analyser encountered an unexpected error when collecting functions."
                ),
            }
            std::process::exit(1);
        }
    }

    match resolve_names(&program, &mut environment) {
        Ok(_) => {}
        Err(e) => {
            match e {
                AnalysisError::CouldNotReadFile(msg) => {
                    println!("Semantic Analyser failed to read AST file. {msg}");
                }
                AnalysisError::OutOfScopes => {
                    println!("Semantic Analyser popped too many scopes and ran out.")
                }
                AnalysisError::VariableAlreadyDeclared(name) => {
                    println!("Variable '{name}' already declared in this scope.");
                }
                AnalysisError::UndeclaredVariable(name) => {
                    println!("Variable '{name}' does not exist.");
                }
                AnalysisError::FunctionAlreadyDefined(name) => {
                    println!(
                        "Function '{name}' already defined. Overrides are not currently supported."
                    );
                }
                AnalysisError::FunctionNotDefined(name) => {
                    println!("Function '{name}' does not exist.");
                }
                AnalysisError::FunctionArgumentCountMismatch(name, expected, found) => {
                    println!(
                        "Function '{name}' expects {expected} arguments, but {found} were provided."
                    );
                }
                _ => unreachable!(
                    "Semantic Analyser encountered an unexpected error when resolving names."
                ),
            }
            std::process::exit(1);
        }
    }

    // clear name resolution data
    environment.scopes.clear();
    environment.push_scope();
    match check_types(&program, &mut environment) {
        Ok(_) => {}
        Err(e) => {
            match e {
                AnalysisError::TypeMismatch(expected, found) => {
                    println!("Type mismatch: expected '{expected}', found '{found}'.")
                }
                AnalysisError::CannotMutate(name) => {
                    println!(
                        "Cannot mutate immutable variable '{name}'. Consider adding 'mut' to the declaration. (let mut {name} = ...;)"
                    )
                }
                AnalysisError::BinOpTypeError(operator, left, right) => {
                    println!(
                        "Operator: '{}' is incompatible with types '{}' and '{}'.",
                        operator, left, right
                    )
                }
                AnalysisError::UnOpTypeError(operator, operand) => {
                    println!(
                        "Operator: '{}' is incompatible with type '{}'.",
                        operator, operand
                    )
                }
                AnalysisError::IntegerLiteralOutOfRange(value, target_type) => {
                    println!("Value {value} is out of range of target type '{target_type}'")
                }
                AnalysisError::FloatLiteralOutOfRange(value, target_type) => {
                    println!("Value {value} is out of range of target type '{target_type}'")
                }
                AnalysisError::OutOfScopes
                | AnalysisError::FunctionAlreadyDefined(_)
                | AnalysisError::FunctionNotDefined(_)
                | AnalysisError::FunctionArgumentCountMismatch(_, _, _)
                | AnalysisError::VariableAlreadyDeclared(_)
                | AnalysisError::UndeclaredVariable(_)
                | AnalysisError::CouldNotReadFile(_) => unreachable!(
                    "Semantic Analyser encountered an unexpected error when checking types."
                ),
            }
            std::process::exit(1);
        }
    }

    // context check for control flow

    // output checked ast? perhaps not necessary
    // output function table??
}

fn get_ast_from_file(file_path: &str) -> Result<Program, AnalysisError> {
    let ast = std::fs::read_to_string(&file_path)
        .map_err(|e| AnalysisError::CouldNotReadFile(e.to_string()))?;
    let program: Program =
        serde_json::from_str(&ast).map_err(|e| AnalysisError::CouldNotReadFile(e.to_string()))?;
    Ok(program)
}

fn collect_functions(
    program: &Program,
    environment: &mut SemanticEnvironment,
) -> Result<(), AnalysisError> {
    for statement in &program.statements {
        match statement {
            Statement::FunctionDefinition {
                identifier,
                parameters,
                body: _,
                return_type,
            } => {
                let func_symbol = FunctionSymbol {
                    parameters: parameters.clone(),
                    return_type: *return_type,
                };
                environment.define_func(identifier.clone(), func_symbol)?;
            }
            _ => {
                // TODO: disallow top level statements
                // return Err(...)
            }
        }
    }
    Ok(())
}

fn resolve_names(
    program: &Program,
    environment: &mut SemanticEnvironment,
) -> Result<(), AnalysisError> {
    for statement in &program.statements {
        resolve_statement(statement, environment)?;
    }
    Ok(())
}

fn resolve_statement(
    statement: &Statement,
    environment: &mut SemanticEnvironment,
) -> Result<(), AnalysisError> {
    match statement {
        Statement::Let {
            identifier,
            expression,
            mutable: _,
            var_type: _,
        } => {
            resolve_expression(&expression, environment)?;
            environment.define(identifier.clone())?;
        }
        Statement::Assignment {
            identifier,
            expression,
        } => {
            resolve_expression(expression, environment)?;
            if !environment.get(identifier) {
                return Err(AnalysisError::UndeclaredVariable(identifier.clone()));
            }
        }
        Statement::If {
            condition,
            body,
            else_block,
        } => {
            resolve_expression(condition, environment)?;
            resolve_block(body, environment)?;
            if let Some(else_block) = else_block {
                resolve_block(else_block, environment)?;
            }
        }
        Statement::While { condition, body } => {
            resolve_expression(condition, environment)?;
            resolve_block(body, environment)?;
        }
        Statement::For {
            initialiser,
            condition,
            increment,
            body,
        } => {
            environment.push_scope();
            if let Some(init) = initialiser {
                resolve_statement(init, environment)?;
            }
            if let Some(condition) = condition {
                resolve_expression(condition, environment)?;
            }
            resolve_block(body, environment)?;
            if let Some(increment) = increment {
                resolve_statement(increment, environment)?;
            }
            environment.pop_scope();
        }
        Statement::Scope { body } => {
            resolve_block(body, environment)?;
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
            resolve_block(body, environment)?;
            environment.pop_scope();
        }
        Statement::ExpressionStatement { expression } => {
            resolve_expression(expression, environment)?;
        }
        Statement::Return { value } => {
            if let Some(expression) = value {
                resolve_expression(expression, environment)?;
            }
        }
        Statement::Break | Statement::Continue | Statement::Pass => (),
    }
    Ok(())
}

fn resolve_expression(
    expression: &Expression,
    environment: &SemanticEnvironment,
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
            resolve_expression(left, environment)?;
            resolve_expression(right, environment)?;
        }
        Expression::UnaryOp {
            operator: _,
            operand,
        } => {
            resolve_expression(operand, environment)?;
        }
        Expression::FunctionCall {
            identifier,
            arguments,
        } => {
            let func = environment.get_func(identifier)?;

            for argument in arguments {
                resolve_expression(argument, environment)?;
            }
            let num_args = arguments.len();
            let num_params = func.parameters.len();
            if num_args != num_params {
                return Err(AnalysisError::FunctionArgumentCountMismatch(
                    identifier.clone(),
                    num_params,
                    num_args,
                ));
            }
        }
    }

    Ok(())
}

fn resolve_block(
    block: &Vec<Statement>,
    environment: &mut SemanticEnvironment,
) -> Result<(), AnalysisError> {
    environment.push_scope();
    for statement in block {
        resolve_statement(statement, environment)?;
    }
    environment.pop_scope();
    Ok(())
}

fn check_types(
    program: &Program,
    environment: &mut SemanticEnvironment,
) -> Result<(), AnalysisError> {
    for statement in &program.statements {
        check_statement_types(statement, environment, None)?;
    }
    Ok(())
}

fn check_statement_types(
    statement: &Statement,
    environment: &mut SemanticEnvironment,
    expected_return_type: Option<Type>,
) -> Result<(), AnalysisError> {
    match statement {
        Statement::Let {
            identifier,
            expression,
            mutable,
            var_type,
        } => {
            let expr_type = check_expression_types(expression, environment, *var_type)?;
            let symbol = Symbol {
                mutable: *mutable,
                var_type: expr_type,
            };
            environment.define_typed(identifier.clone(), symbol)?;
        }
        Statement::Assignment {
            identifier,
            expression,
        } => {
            let stored_var = environment
                .get_typed(identifier)
                .ok_or(AnalysisError::UndeclaredVariable(identifier.clone()))?;
            if !stored_var.mutable {
                return Err(AnalysisError::CannotMutate(identifier.clone()));
            }
            check_expression_types(expression, environment, Some(stored_var.var_type))?;
        }
        Statement::If {
            condition,
            body,
            else_block,
        } => {
            check_expression_types(condition, environment, Some(Type::Bool))?;
            check_block_types(body, environment, None)?;
            if let Some(else_block) = else_block {
                check_block_types(else_block, environment, None)?;
            }
        }
        Statement::While { condition, body } => {
            check_expression_types(condition, environment, Some(Type::Bool))?;
            check_block_types(body, environment, None)?;
        }
        Statement::For {
            initialiser,
            condition,
            increment,
            body,
        } => {
            environment.push_scope_typed();
            if let Some(initialiser) = initialiser {
                check_statement_types(initialiser, environment, None)?;
            }
            if let Some(condition) = condition {
                check_expression_types(condition, environment, Some(Type::Bool))?;
            }
            if let Some(increment) = increment {
                check_statement_types(increment, environment, None)?;
            }
            check_block_types(body, environment, None)?;
            environment.pop_scope_typed();
        }
        Statement::Scope { body } => {
            check_block_types(body, environment, None)?;
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
            check_block_types(body, environment, Some(*return_type))?;
            environment.pop_scope_typed();
        }
        Statement::ExpressionStatement { expression } => {
            check_expression_types(expression, environment, None)?;
        }
        Statement::Return { value } => {
            if let Some(expr) = value {
                check_expression_types(expr, environment, expected_return_type)?;
            }
        }
        Statement::Break | Statement::Continue | Statement::Pass => {}
    }

    Ok(())
}

fn check_expression_types(
    expression: &Expression,
    environment: &SemanticEnvironment,
    expected_type: Option<Type>,
) -> Result<Type, AnalysisError> {
    Ok(match expression {
        Expression::Literal(value) => match value {
            Value::Unit => Type::Unit,
            Value::String(_) => Type::String,
            Value::Bool(_) => Type::Bool,
            Value::Number(number) => match number {
                Number::IntegerLiteral(value) => {
                    if let Some(expected_type) = expected_type {
                        if number_fits_type(number, &expected_type) {
                            expected_type
                        } else {
                            return Err(AnalysisError::IntegerLiteralOutOfRange(
                                *value,
                                expected_type,
                            ));
                        }
                    } else {
                        Type::I32
                    }
                }

                Number::FloatLiteral(value) => {
                    if let Some(expected_type) = expected_type {
                        if number_fits_type(number, &expected_type) {
                            expected_type
                        } else {
                            return Err(AnalysisError::FloatLiteralOutOfRange(
                                *value,
                                expected_type,
                            ));
                        }
                    } else {
                        Type::F64
                    }
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
        Expression::Identifier(name) => {
            environment
                .get_typed(name)
                .ok_or(AnalysisError::UndeclaredVariable(name.clone()))?
                .var_type
        }
        Expression::BinaryOp {
            left,
            operator,
            right,
        } => {
            let expected_operand_type = if binary_operator_uses_operand_type(operator) {
                expected_type
            } else {
                None
            };
            let left_type = check_expression_types(left, environment, expected_operand_type)?;
            let right_type = check_expression_types(right, environment, expected_operand_type)?;
            let result_type = check_binary_operator_types(&left_type, operator, &right_type)?;
            if let Some(expected_type) = expected_type {
                if result_type != expected_type {
                    return Err(AnalysisError::TypeMismatch(expected_type, result_type));
                }
            }
            result_type
        }
        Expression::UnaryOp { operator, operand } => {
            if *operator == TokenType::Minus {
                if let Expression::Literal(Value::Number(Number::IntegerLiteral(value))) =
                    operand.as_ref()
                {
                    let value = -*value;

                    if let Some(expected_type) = expected_type {
                        if number_fits_type(&Number::IntegerLiteral(value), &expected_type) {
                            return Ok(expected_type);
                        } else {
                            return Err(AnalysisError::IntegerLiteralOutOfRange(
                                value,
                                expected_type,
                            ));
                        }
                    }

                    // No expected type -> default integer type
                    return Ok(Type::I32);
                }
            }

            let operand_type = check_expression_types(operand, environment, expected_type)?;

            check_unary_operator_types(operator, &operand_type)?
        }
        Expression::FunctionCall {
            identifier,
            arguments,
        } => {
            let func_symbol = environment.get_func(identifier)?;
            for (param, arg) in func_symbol.parameters.iter().zip(arguments) {
                let arg_type = check_expression_types(arg, environment, Some(param.param_type))?;
                if param.param_type != arg_type {
                    return Err(AnalysisError::TypeMismatch(param.param_type, arg_type));
                }
            }
            func_symbol.return_type
        }
    })
}

fn check_block_types(
    block: &Vec<Statement>,
    environment: &mut SemanticEnvironment,
    expected_return_type: Option<Type>,
) -> Result<(), AnalysisError> {
    environment.push_scope_typed();
    for statement in block {
        check_statement_types(statement, environment, expected_return_type)?;
    }
    environment.pop_scope_typed();
    Ok(())
}

fn check_binary_operator_types(
    left: &Type,
    operator: &TokenType,
    right: &Type,
) -> Result<Type, AnalysisError> {
    match operator {
        TokenType::Plus => check_plus_types(left, operator, right),
        TokenType::Minus => check_number_only_bin_op_types(left, operator, right),
        TokenType::Asterisk => check_number_only_bin_op_types(left, operator, right),
        TokenType::Slash => check_number_only_bin_op_types(left, operator, right),
        TokenType::GreaterThan => check_order_comparison_op_types(left, operator, right),
        TokenType::LessThan => check_order_comparison_op_types(left, operator, right),
        TokenType::GreaterThanOrEqualTo => check_order_comparison_op_types(left, operator, right),
        TokenType::LessThanOrEqualTo => check_order_comparison_op_types(left, operator, right),
        TokenType::EqualTo => check_equivalence_comparison_op_types(left, operator, right),
        TokenType::NotEqualTo => check_equivalence_comparison_op_types(left, operator, right),
        TokenType::LogicalAnd => check_logical_bin_op_types(left, operator, right),
        TokenType::LogicalOr => check_logical_bin_op_types(left, operator, right),
        _ => unreachable!(
            "Invalid binary operator during type checking: '{}'.",
            operator
        ),
    }
}

fn check_plus_types(
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

fn check_number_only_bin_op_types(
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

fn check_order_comparison_op_types(
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

fn check_equivalence_comparison_op_types(
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

fn check_logical_bin_op_types(
    left: &Type,
    operator: &TokenType,
    right: &Type,
) -> Result<Type, AnalysisError> {
    match (left, right) {
        (Type::Bool, Type::Bool) => Ok(Type::Bool),
        _ => Err(AnalysisError::BinOpTypeError(*operator, *left, *right)),
    }
}

fn check_unary_operator_types(operator: &TokenType, operand: &Type) -> Result<Type, AnalysisError> {
    match operator {
        TokenType::Increment => check_number_only_un_op_types(operator, operand),
        TokenType::Decrement => check_number_only_un_op_types(operator, operand),
        TokenType::Not => check_bool_only_un_op_types(operator, operand),
        TokenType::Minus => check_number_only_un_op_types(operator, operand),
        _ => unreachable!("Invalid unary operator during type checking: {}", operator),
    }
}

fn check_number_only_un_op_types(
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

fn check_bool_only_un_op_types(
    operator: &TokenType,
    operand: &Type,
) -> Result<Type, AnalysisError> {
    match operand {
        Type::Bool => Ok(*operand),
        _ => Err(AnalysisError::UnOpTypeError(*operator, *operand)),
    }
}

fn number_fits_type(number: &Number, expected_type: &Type) -> bool {
    match (number, expected_type) {
        (Number::IntegerLiteral(value), Type::I8) => i8::try_from(*value).is_ok(),
        (Number::IntegerLiteral(value), Type::I16) => i16::try_from(*value).is_ok(),
        (Number::IntegerLiteral(value), Type::I32) => i32::try_from(*value).is_ok(),
        (Number::IntegerLiteral(_), Type::I64) => true,

        (Number::IntegerLiteral(value), Type::U8) => u8::try_from(*value).is_ok(),
        (Number::IntegerLiteral(value), Type::U16) => u16::try_from(*value).is_ok(),
        (Number::IntegerLiteral(value), Type::U32) => u32::try_from(*value).is_ok(),
        (Number::IntegerLiteral(value), Type::U64) => *value >= 0,

        (Number::FloatLiteral(_), Type::F32) => true,
        (Number::FloatLiteral(_), Type::F64) => true,

        _ => false,
    }
}

fn binary_operator_uses_operand_type(operator: &TokenType) -> bool {
    match operator {
        TokenType::Plus | TokenType::Minus | TokenType::Asterisk | TokenType::Slash => true,
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
