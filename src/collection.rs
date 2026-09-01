use crate::AnalysisError;
use syntax::{
    ast::{Program, Statement},
    functions::{
        Function, FunctionBody, FunctionId, FunctionImplementation, FunctionSignature,
        FunctionTable,
    },
    types::Type,
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
                    implementation: FunctionImplementation::Oxide(function_body),
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
