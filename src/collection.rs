use crate::AnalysisError;
use syntax::{
    ast::{Expression, Program, Statement},
    functions::{
        Function, FunctionBody, FunctionId, FunctionImplementation, FunctionSignature,
        FunctionTable,
    },
    types::{Type, TypeId, TypeTable},
};

pub fn collect_functions_and_globals(
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
                    parameters: parameters.iter().map(|fp| fp.param_type.clone()).collect(),
                    return_type: return_type.clone(),
                };
                if func_table
                    .has_function_with_parameters(identifier, &function_signature.parameters)
                {
                    return Err(AnalysisError::FunctionAlreadyDefined(identifier.clone()));
                }

                let func = Function {
                    id: FunctionId(func_table.functions.len()),
                    implementation: FunctionImplementation::Oxide(function_body),
                    signature: function_signature,
                    parameters: parameters.clone(),
                };
                func_table.functions.push(func);
            }
            Statement::Let { expression, .. } => {
                match expression {
                    Expression::Literal(_) => {}
                    Expression::Identifier(_)
                    | Expression::BinaryOp { .. }
                    | Expression::UnaryOp { .. }
                    | Expression::UnresolvedFunctionCall { .. }
                    | Expression::ResolvedFunctionCall { .. } => {
                        return Err(AnalysisError::InvalidGlobalDeclaration(
                            "Global variables can only be initialized with constant expressions."
                                .into(),
                        ));
                    }
                }
                global_table.push(statement.clone());
            }
            Statement::TypeDefinition(_) => {}
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

pub fn collect_types(
    program: &mut Program,
    type_table: &mut TypeTable,
) -> Result<(), AnalysisError> {
    collect_primitive_types(type_table);
    for statement in &mut program.statements {
        if let Statement::TypeDefinition(type_def) = statement {
            if type_table
                .types
                .iter()
                .any(|existing_type| existing_type.name == type_def.name)
            {
                return Err(AnalysisError::TypeAlreadyDefined(type_def.name.clone()));
            }
            type_def.id = TypeId(type_table.types.len());
            type_table.types.push(type_def.clone());
        }
    }
    Ok(())
}
macro_rules! generate_type_definition {
    ($name:expr, $type_name:expr, $id:expr) => {
        syntax::types::TypeDefinition {
            id: TypeId($id), // Placeholder, will be set later
            name: $name.into(),
            parameters: vec![],
            kind: syntax::types::TypeDefinitionKind::BuiltIn($type_name),
        }
    };
    () => {};
}
fn collect_primitive_types(type_table: &mut TypeTable) {
    type_table.types.push(generate_type_definition!(
        "u8",
        Type::U8,
        type_table.types.len()
    ));
    type_table.types.push(generate_type_definition!(
        "u16",
        Type::U16,
        type_table.types.len()
    ));
    type_table.types.push(generate_type_definition!(
        "u32",
        Type::U32,
        type_table.types.len()
    ));
    type_table.types.push(generate_type_definition!(
        "u64",
        Type::U64,
        type_table.types.len()
    ));
    type_table.types.push(generate_type_definition!(
        "i8",
        Type::I8,
        type_table.types.len()
    ));
    type_table.types.push(generate_type_definition!(
        "i16",
        Type::I16,
        type_table.types.len()
    ));
    type_table.types.push(generate_type_definition!(
        "i32",
        Type::I32,
        type_table.types.len()
    ));
    type_table.types.push(generate_type_definition!(
        "i64",
        Type::I64,
        type_table.types.len()
    ));
    type_table.types.push(generate_type_definition!(
        "f32",
        Type::F32,
        type_table.types.len()
    ));
    type_table.types.push(generate_type_definition!(
        "f64",
        Type::F64,
        type_table.types.len()
    ));
    type_table.types.push(generate_type_definition!(
        "bool",
        Type::Bool,
        type_table.types.len()
    ));
    type_table.types.push(generate_type_definition!(
        "string",
        Type::String,
        type_table.types.len()
    ));
    type_table.types.push(generate_type_definition!(
        "unit",
        Type::Unit,
        type_table.types.len()
    ));
}
