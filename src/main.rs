use std::{
    collections::{HashMap, HashSet},
    env, fs,
};

use syntax::{
    ast::{Expression, Program, Statement, TokenType},
    functions::{
        CheckedProgram, Function, FunctionBody, FunctionId, FunctionImplementation,
        FunctionImplementation::Oxide, FunctionSignature, FunctionTable,
    },
    semantics::Symbol,
    stdlib::builtin_functions,
    types::{Number, Type, Value},
};

#[derive(Debug)]
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
    CannotMutate(String),
    TopLevelStatement,
    NoMainFunction,
    CouldNotReadFile(String),
}

struct SemanticEnvironment {
    scopes: Vec<HashSet<String>>,
    scopes_typed: Vec<HashMap<String, Symbol>>,
}
impl SemanticEnvironment {
    fn new() -> Self {
        Self {
            scopes: vec![HashSet::new()],
            scopes_typed: vec![HashMap::new()],
            // functions: HashMap::new(),
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
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Please provide a file to analyse.");
        return;
    }
    let mut program = match get_ast_from_file(&args[1]) {
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
    let mut function_table = FunctionTable {
        functions: builtin_functions(),
    };
    let mut global_table: Vec<Statement> = Vec::new();
    match collect_functions_and_globals(&program, &mut function_table, &mut global_table) {
        Ok(_) => {}
        Err(e) => {
            match e {
                AnalysisError::FunctionAlreadyDefined(name) => {
                    println!(
                        "Function '{name}' already defined. Overrides are not currently supported."
                    );
                }
                AnalysisError::TopLevelStatement => {
                    println!(
                        "The only top level statements allowed are global variable declarations and function definitions."
                    )
                }
                AnalysisError::NoMainFunction => {
                    println!("Please provide a function called 'main' with no parameters.")
                }
                _ => unreachable!(
                    "Semantic Analyser encountered an unexpected error when collecting functions."
                ),
            }
            std::process::exit(1);
        }
    }

    match resolve_names(&mut program, &mut environment, &function_table) {
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
    match check_types(&mut program, &mut environment, &function_table) {
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
                AnalysisError::OutOfScopes
                | AnalysisError::FunctionAlreadyDefined(_)
                | AnalysisError::FunctionNotDefined(_)
                | AnalysisError::FunctionArgumentCountMismatch(_, _, _)
                | AnalysisError::VariableAlreadyDeclared(_)
                | AnalysisError::UndeclaredVariable(_)
                | AnalysisError::TopLevelStatement
                | AnalysisError::NoMainFunction
                | AnalysisError::CouldNotReadFile(_) => unreachable!(
                    "Semantic Analyser encountered an unexpected error when checking types."
                ),
            }
            std::process::exit(1);
        }
    }

    let checked_program = build_checked_program(&program, &function_table)
        .expect("function collection must produce a complete checked program");

    // context check for control flow

    if args.len() == 3 {
        output_checked_program(&checked_program, &args[2]);
    }
}

fn get_ast_from_file(file_path: &str) -> Result<Program, AnalysisError> {
    let ast = std::fs::read_to_string(&file_path)
        .map_err(|e| AnalysisError::CouldNotReadFile(e.to_string()))?;
    let program: Program =
        serde_json::from_str(&ast).map_err(|e| AnalysisError::CouldNotReadFile(e.to_string()))?;
    Ok(program)
}

fn output_checked_program(program: &CheckedProgram, file_path: &str) {
    let json = serde_json::to_string_pretty(program);
    match json {
        Ok(json) => {
            fs::write(file_path, json).expect("Unable to write to file");
        }
        Err(e) => {
            println!("Error serialising checked AST JSON: {}", e);
            std::process::exit(1);
        }
    }
}

fn build_checked_program(
    program: &Program,
    collected_functions: &FunctionTable,
) -> Result<CheckedProgram, AnalysisError> {
    let globals = program
        .statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::Let { .. } => Some(statement.clone()),
            _ => None,
        })
        .collect();

    let mut functions: Vec<Function> = collected_functions
        .functions
        .iter()
        .filter(|function| matches!(&function.implementation, FunctionImplementation::Builtin(_)))
        .cloned()
        .collect();
    let oxide_functions = program
        .statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::FunctionDefinition {
                identifier,
                parameters,
                body,
                return_type,
            } => Some((identifier, parameters, body, return_type)),
            _ => None,
        })
        .map(|(identifier, parameters, body, return_type)| {
            let signature = FunctionSignature {
                name: identifier.clone(),
                parameters: parameters
                    .iter()
                    .map(|parameter| parameter.param_type)
                    .collect(),
                return_type: *return_type,
            };
            let collected_function = collected_functions
                .functions
                .iter()
                .find(|function| function.signature == signature)
                .ok_or_else(|| AnalysisError::FunctionNotDefined(identifier.clone()))?;

            Ok(Function {
                id: collected_function.id.clone(),
                signature,
                parameters: parameters.clone(),
                implementation: Oxide(FunctionBody {
                    statements: body.clone(),
                }),
            })
        })
        .collect::<Result<Vec<_>, AnalysisError>>()?;
    functions.extend(oxide_functions);
    let function_table = FunctionTable { functions };
    let main_function_id = function_table
        .functions
        .iter()
        .find(|function| {
            function.signature
                == (FunctionSignature {
                    name: "main".into(),
                    parameters: vec![],
                    return_type: Type::Unit,
                })
        })
        .map(|function| function.id.clone())
        .ok_or(AnalysisError::NoMainFunction)?;

    Ok(CheckedProgram {
        function_table,
        globals,
        main_function_id,
    })
}

fn collect_functions_and_globals(
    program: &Program,
    func_table: &mut FunctionTable,
    global_table: &mut Vec<Statement>,
) -> Result<(), AnalysisError> {
    for statement in &program.statements {
        match statement {
            Statement::FunctionDefinition {
                identifier,
                parameters,
                body,
                return_type,
            } => {
                let function_body = FunctionBody {
                    statements: body.clone(),
                };
                let function_signature = FunctionSignature {
                    name: identifier.clone(),
                    parameters: parameters.iter().map(|fp| fp.param_type).collect(),
                    return_type: *return_type,
                };
                if func_table
                    .has_function_with_parameters(identifier, &function_signature.parameters)
                {
                    return Err(AnalysisError::FunctionAlreadyDefined(identifier.clone()));
                }

                let func = Function {
                    id: FunctionId(func_table.functions.len()),
                    implementation: Oxide(function_body),
                    signature: function_signature,
                    parameters: parameters.clone(),
                };
                func_table.functions.push(func);
            }
            Statement::Let { .. } => {
                global_table.push(statement.clone());
            }
            _ => return Err(AnalysisError::TopLevelStatement),
        }
    }
    let main_signature = FunctionSignature {
        name: "main".into(),
        parameters: vec![],
        return_type: Type::Unit,
    };
    if !func_table.has_function(main_signature) {
        return Err(AnalysisError::NoMainFunction);
    }
    Ok(())
}

fn resolve_names(
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

fn resolve_statement(
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

fn resolve_expression(
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

fn resolve_block(
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

fn check_types(
    program: &mut Program,
    environment: &mut SemanticEnvironment,
    func_table: &FunctionTable,
) -> Result<(), AnalysisError> {
    // Like name resolution, type globals before function bodies so globals are
    // available from every function definition.
    for statement in &mut program.statements {
        if matches!(statement, Statement::Let { .. }) {
            check_statement_types(statement, environment, None, func_table)?;
        }
    }
    for statement in &mut program.statements {
        if matches!(statement, Statement::FunctionDefinition { .. }) {
            check_statement_types(statement, environment, None, func_table)?;
        }
    }
    Ok(())
}

fn check_statement_types(
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

fn check_expression_types(
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
            let left_type =
                check_expression_types(left, environment, expected_operand_type, func_table)?;
            let right_type =
                check_expression_types(right, environment, expected_operand_type, func_table)?;
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
                    operand.as_mut()
                {
                    let value = -*value;

                    let (resolved_number, resolved_type) =
                        resolve_integer_literal(value, expected_type)?;

                    *operand = Box::new(Expression::Literal(Value::Number(resolved_number)));

                    return Ok(resolved_type);
                }
            }

            let operand_type =
                check_expression_types(operand, environment, expected_type, func_table)?;

            check_unary_operator_types(operator, &operand_type)?
        }
        Expression::ResolvedFunctionCall {
            function_id,
            arguments,
        } => {
            let function = func_table
                .get(function_id.clone())
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
                .get(function_id.clone())
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

fn check_block_types(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn integer_literal(value: i64) -> Expression {
        Expression::Literal(Value::Number(Number::IntegerLiteral(value)))
    }

    #[test]
    fn globals_are_available_to_functions_and_calls_are_resolved() {
        let mut program = Program {
            statements: vec![
                Statement::FunctionDefinition {
                    identifier: "read_global".into(),
                    parameters: vec![],
                    body: vec![Statement::Return {
                        value: Some(Expression::Identifier("answer".into())),
                    }],
                    return_type: Type::I32,
                },
                Statement::FunctionDefinition {
                    identifier: "main".into(),
                    parameters: vec![],
                    body: vec![Statement::Let {
                        identifier: "value".into(),
                        expression: Expression::UnresolvedFunctionCall {
                            identifier: "read_global".into(),
                            arguments: vec![],
                        },
                        mutable: false,
                        var_type: Some(Type::I32),
                    }],
                    return_type: Type::Unit,
                },
                Statement::Let {
                    identifier: "answer".into(),
                    expression: integer_literal(42),
                    mutable: false,
                    var_type: Some(Type::I32),
                },
            ],
        };
        let mut function_table = FunctionTable {
            functions: Vec::new(),
        };
        let mut globals = Vec::new();

        collect_functions_and_globals(&program, &mut function_table, &mut globals).unwrap();
        assert_eq!(function_table.functions.len(), 2);
        assert_eq!(globals.len(), 1);

        let mut names = SemanticEnvironment::new();
        resolve_names(&mut program, &mut names, &function_table).unwrap();

        let mut types = SemanticEnvironment::new();
        check_types(&mut program, &mut types, &function_table).unwrap();
        let checked_program = build_checked_program(&program, &function_table).unwrap();

        assert_eq!(checked_program.globals.len(), 1);
        assert_eq!(checked_program.main_function_id, FunctionId(1));
        let main = checked_program
            .function_table
            .get(checked_program.main_function_id.clone())
            .expect("main function must be in the checked function table");
        let Oxide(FunctionBody { statements: body }) = &main.implementation else {
            panic!("main must have an Oxide body");
        };
        let Statement::Let { expression, .. } = &body[0] else {
            panic!("expected local declaration");
        };
        assert!(matches!(
            expression,
            Expression::ResolvedFunctionCall { .. }
        ));
        assert!(serde_json::to_string(&checked_program).is_ok());
    }

    #[test]
    fn overloads_are_selected_using_argument_types() {
        let mut program = Program {
            statements: vec![
                Statement::FunctionDefinition {
                    identifier: "echo".into(),
                    parameters: vec![syntax::functions::FunctionParameter {
                        name: "value".into(),
                        param_type: Type::String,
                    }],
                    body: vec![],
                    return_type: Type::String,
                },
                Statement::FunctionDefinition {
                    identifier: "echo".into(),
                    parameters: vec![syntax::functions::FunctionParameter {
                        name: "value".into(),
                        param_type: Type::Bool,
                    }],
                    body: vec![],
                    return_type: Type::Bool,
                },
                Statement::FunctionDefinition {
                    identifier: "main".into(),
                    parameters: vec![],
                    body: vec![Statement::Let {
                        identifier: "result".into(),
                        expression: Expression::UnresolvedFunctionCall {
                            identifier: "echo".into(),
                            arguments: vec![Expression::Literal(Value::Bool(true))],
                        },
                        mutable: false,
                        var_type: Some(Type::Bool),
                    }],
                    return_type: Type::Unit,
                },
            ],
        };
        let mut function_table = FunctionTable {
            functions: Vec::new(),
        };
        let mut globals = Vec::new();
        collect_functions_and_globals(&program, &mut function_table, &mut globals).unwrap();

        let mut names = SemanticEnvironment::new();
        resolve_names(&mut program, &mut names, &function_table).unwrap();
        let mut types = SemanticEnvironment::new();
        check_types(&mut program, &mut types, &function_table).unwrap();

        let Statement::FunctionDefinition { body, .. } = &program.statements[2] else {
            panic!("expected main function");
        };
        let Statement::Let { expression, .. } = &body[0] else {
            panic!("expected local declaration");
        };
        assert!(matches!(
            expression,
            Expression::ResolvedFunctionCall {
                function_id: FunctionId(1),
                ..
            }
        ));
    }

    #[test]
    fn standard_library_functions_are_resolved_and_emitted() {
        let mut program = Program {
            statements: vec![Statement::FunctionDefinition {
                identifier: "main".into(),
                parameters: vec![],
                body: vec![
                    Statement::ExpressionStatement {
                        expression: Expression::UnresolvedFunctionCall {
                            identifier: "println".into(),
                            arguments: vec![Expression::Literal(Value::String("hello".into()))],
                        },
                    },
                    Statement::Let {
                        identifier: "name".into(),
                        expression: Expression::UnresolvedFunctionCall {
                            identifier: "input".into(),
                            arguments: vec![Expression::Literal(Value::String("Name: ".into()))],
                        },
                        mutable: false,
                        var_type: Some(Type::String),
                    },
                ],
                return_type: Type::Unit,
            }],
        };
        let mut function_table = FunctionTable {
            functions: builtin_functions(),
        };
        let mut globals = Vec::new();
        collect_functions_and_globals(&program, &mut function_table, &mut globals).unwrap();

        let mut names = SemanticEnvironment::new();
        resolve_names(&mut program, &mut names, &function_table).unwrap();
        let mut types = SemanticEnvironment::new();
        check_types(&mut program, &mut types, &function_table).unwrap();
        let checked_program = build_checked_program(&program, &function_table).unwrap();

        assert_eq!(checked_program.main_function_id, FunctionId(14));
        assert_eq!(checked_program.function_table.functions.len(), 15);
        let Oxide(FunctionBody { statements: body }) = &checked_program
            .function_table
            .get(FunctionId(14))
            .expect("main must be present")
            .implementation
        else {
            panic!("main must have an Oxide body");
        };
        let Statement::ExpressionStatement { expression } = &body[0] else {
            panic!("expected println call");
        };
        assert!(matches!(
            expression,
            Expression::ResolvedFunctionCall {
                function_id: FunctionId(1),
                ..
            }
        ));
        let Statement::Let { expression, .. } = &body[1] else {
            panic!("expected input declaration");
        };
        assert!(matches!(
            expression,
            Expression::ResolvedFunctionCall {
                function_id: FunctionId(13),
                ..
            }
        ));
    }
}
