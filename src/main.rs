use std::{collections::HashMap, env};
use syntax::{Expression, Program, Statement, Value};

enum AnalysisError {
    OutOfScopes,
    VariableAlreadyDeclared(String),
    UndeclaredVariable(String),
    FunctionAlreadyDefined(String),
    FunctionNotDefined(String),
    FunctionArgumentCountMismatch(String, usize, usize),
    CouldNotReadFile(String),
}

struct SemanticEnvironment {
    scopes: Vec<HashMap<String, Symbol>>,
    functions: HashMap<String, FunctionSymbol>,
}
impl SemanticEnvironment {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
        }
    }
    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define(&mut self, name: String, symbol: Symbol) -> Result<(), AnalysisError> {
        let scope = self.scopes.last_mut().ok_or(AnalysisError::OutOfScopes)?;

        if scope.contains_key(&name) {
            return Err(AnalysisError::VariableAlreadyDeclared(name.clone()));
        }

        scope.insert(name, symbol);
        Ok(())
    }

    fn get(&self, name: &str) -> Option<&Symbol> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
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
struct Symbol {
    mutable: bool,
}
struct FunctionSymbol {
    parameters: Vec<String>,
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
    match resolve_names(&program, &mut environment) {
        Ok(_) => {}
        Err(e) => match e {
            AnalysisError::OutOfScopes => todo!(),
            AnalysisError::VariableAlreadyDeclared(_) => todo!(),
            AnalysisError::FunctionAlreadyDefined(_) => todo!(),
            AnalysisError::UndeclaredVariable(_) => todo!(),
            AnalysisError::CouldNotReadFile(_) => todo!(),
            AnalysisError::FunctionNotDefined(_) => todo!(),
            AnalysisError::FunctionArgumentCountMismatch(_, _, _) => todo!(),
        },
    }
}

fn get_ast_from_file(file_path: &str) -> Result<Program, AnalysisError> {
    let ast = std::fs::read_to_string(&file_path)
        .map_err(|e| AnalysisError::CouldNotReadFile(e.to_string()))?;
    let program: Program =
        serde_json::from_str(&ast).map_err(|e| AnalysisError::CouldNotReadFile(e.to_string()))?;
    Ok(program)
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
        } => {
            resolve_expression(&expression, environment)?;
            let symbol = Symbol { mutable: true }; // currently all variables are mutable - let statements will have mutable field later.
            environment.define(identifier.clone(), symbol)?;
        }
        Statement::Assignment {
            identifier,
            expression,
        } => {
            resolve_expression(expression, environment)?;
            environment
                .get(identifier)
                .ok_or(AnalysisError::UndeclaredVariable(identifier.clone()))?;
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
            identifier,
            parameters,
            body,
        } => {
            let func_symbol = FunctionSymbol {
                parameters: parameters.clone(),
            };
            environment.define_func(identifier.clone(), func_symbol)?;
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
            environment
                .get(identifier)
                .ok_or(AnalysisError::UndeclaredVariable(identifier.clone()))?;
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
