mod collection;
mod name_resolution;
mod type_checking;
use collection::collect_functions_and_globals;
use name_resolution::resolve_names;
use std::{
    collections::{HashMap, HashSet},
    env, fs,
};
use syntax::{
    ast::{Program, Statement, TokenType},
    functions::{
        CheckedProgram, Function, FunctionBody, FunctionImplementation,
        FunctionImplementation::Oxide, FunctionSignature, FunctionTable,
    },
    semantics::Symbol,
    stdlib::builtin_functions,
    types::Type,
};
use type_checking::check_types;

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

    // context check for control flow

    let checked_program = build_checked_program(&program, &function_table)
        .expect("function collection must produce a complete checked program");

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

#[cfg(test)]
mod tests {
    use super::*;
    use collection::collect_functions_and_globals;
    use syntax::{
        ast::Expression,
        functions::FunctionId,
        types::{Number, Value},
    };

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
