use std::{
    collections::{HashMap, HashSet},
    env,
};
use syntax::{Expression, FunctionSymbol, Program, Statement, Symbol, Type};

enum AnalysisError {
    OutOfScopes,
    VariableAlreadyDeclared(String),
    UndeclaredVariable(String),
    FunctionAlreadyDefined(String),
    FunctionNotDefined(String),
    FunctionArgumentCountMismatch(String, usize, usize),
    TypeMismatch(Type, Type),
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
            parameters: _,
            body,
            return_type: _,
        } => {
            resolve_block(body, environment)?;
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
        check_statement_types(statement, environment)?;
    }
    Ok(())
}

fn check_statement_types(
    statement: &Statement,
    environment: &mut SemanticEnvironment,
) -> Result<(), AnalysisError> {
    match statement {
        Statement::Let {
            identifier,
            expression,
            mutable,
            var_type,
        } => {
            let expr_type = check_expression_types(expression, environment)?;
            if let Some(annotation) = var_type {
                if *annotation != expr_type {
                    return Err(AnalysisError::TypeMismatch(*annotation, expr_type));
                }
            }
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
            let expr_type = check_expression_types(expression, environment)?;
            if stored_var.var_type != expr_type {
                return Err(AnalysisError::TypeMismatch(stored_var.var_type, expr_type));
            }
        }
        Statement::If {
            condition,
            body,
            else_block,
        } => {
            let condition_type = check_expression_types(condition, environment)?;
            if condition_type != Type::Bool {
                return Err(AnalysisError::TypeMismatch(Type::Bool, condition_type));
            }
            check_block_types(body, environment)?;
            if let Some(else_block) = else_block {
                check_block_types(else_block, environment)?;
            }
        }
        Statement::While { condition, body } => {
            let condition_type = check_expression_types(condition, environment)?;
            if condition_type != Type::Bool {
                return Err(AnalysisError::TypeMismatch(Type::Bool, condition_type));
            }
            check_block_types(body, environment)?;
        }
        Statement::For {
            initialiser,
            condition,
            increment,
            body,
        } => {
            environment.push_scope_typed();
            if let Some(initialiser) = initialiser {
                check_statement_types(initialiser, environment)?;
            }
            if let Some(condition) = condition {
                check_expression_types(condition, environment)?;
            }
            if let Some(increment) = increment {
                check_statement_types(increment, environment)?;
            }
            check_block_types(body, environment)?;
            environment.pop_scope_typed();
        }
        Statement::Scope { body } => {
            check_block_types(body, environment)?;
        }
        Statement::FunctionDefinition {
            identifier: _,
            parameters: _,
            body,
            return_type: _,
        } => {
            check_block_types(body, environment)?;
        }
        Statement::ExpressionStatement { expression } => {
            check_expression_types(expression, environment)?;
        }
        Statement::Return { value } => {
            // check the function return type matches the value of the return statement?
            todo!()
        }
        Statement::Break | Statement::Continue | Statement::Pass => {}
    }

    Ok(())
}

fn check_expression_types(
    expression: &Expression,
    environment: &SemanticEnvironment,
) -> Result<Type, AnalysisError> {
    Ok(Type::Unit)
}

fn check_block_types(
    block: &Vec<Statement>,
    environment: &mut SemanticEnvironment,
) -> Result<(), AnalysisError> {
    environment.push_scope_typed();
    for statement in block {
        check_statement_types(statement, environment)?;
    }
    environment.pop_scope_typed();
    Ok(())
}
