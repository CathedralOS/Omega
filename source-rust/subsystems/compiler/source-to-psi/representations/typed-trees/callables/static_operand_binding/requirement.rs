use super::{Operand, Stop, bind, unserved};
use crate::{
    TypedTrees, expression::StaticMachineArgument,
    static_applications::StaticMachineApplicationHandle,
};
use symbols::SymbolHandle;

/// Product operands spend the same declared argument tuple as source calls.
/// Retain its binding judgment here so both entrances select the canonical
/// requirement header; a declaration name alone cannot substitute a contract.
pub fn prepare_requirement_application(
    program: &mut TypedTrees,
    target: SymbolHandle,
    application: StaticMachineApplicationHandle,
    arguments: &[StaticMachineArgument],
) -> Result<Option<SymbolHandle>, Stop> {
    let Some((owner, requirement)) = program.trait_machine_signature(target) else {
        return Ok(None);
    };
    if requirement.symbol != target {
        return Ok(None);
    }
    if !owner.type_parameters.is_empty() {
        return Err(unserved(
            "a requirement application still has its trait's static parameters",
        ));
    }
    if !owner.lifetime_parameters.is_empty() {
        return Err(unserved(
            "a requirement application still has its trait's lifetime parameters",
        ));
    }
    let environment =
        crate::source_state::trait_definition::AppliedTraitScope::abstract_scope(owner)
            .environment();
    prepare_requirement_in_scope(program, &environment, target, application, arguments)
}

/// Calls and Build selections close the same complete requirement in its
/// declaring trait application. The resulting header retains both argument
/// lists; a phantom trait parameter still distinguishes two requirements.
pub fn prepare_requirement_in_scope(
    program: &mut TypedTrees,
    environment: &crate::source_state::trait_definition::TraitApplicationEnvironment,
    target: SymbolHandle,
    application: StaticMachineApplicationHandle,
    arguments: &[StaticMachineArgument],
) -> Result<Option<SymbolHandle>, Stop> {
    let (owner, requirement) = program
        .trait_machine_signature(target)
        .ok_or(unserved("applied requirement lost its declaration"))?;
    let scope = environment
        .scope(program)
        .filter(|scope| scope.declaration.symbol == owner.symbol)
        .ok_or(unserved("applied requirement changes its declaring trait"))?;
    let parameters = program
        .state_signature_type_parameters(requirement)
        .to_vec();
    if parameters.is_empty() && scope.keeps_declared_signatures(program) {
        return Ok(None);
    }
    if !requirement.lifetime_parameters.is_empty() {
        return Err(unserved(
            "a requirement application still has its own lifetime parameters",
        ));
    }
    if parameters.len() != arguments.len() {
        return Err(unserved(
            "a requirement application needs one argument for each declared binder",
        ));
    }
    if program
        .expression_table
        .static_machine_application(application)
        .registration_operation
        != target
    {
        return Err(unserved(
            "a requirement application names a different registration operation",
        ));
    }
    let first_type = program.type_reference_table.type_reference_count();
    let trait_arguments = program
        .materialize_trait_application_arguments(environment)
        .map_err(unserved)?;
    let lexical_owner = program
        .expression_table
        .static_machine_application(application)
        .lexical_owner;
    if program
        .require_owned_application_operands(&trait_arguments, lexical_owner)
        .is_err()
    {
        return Ok(None);
    }
    let supplying_owner = program
        .expression_table
        .static_machine_application(application)
        .lexical_owner;
    let mut operands = Vec::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(arguments) {
        operands.push(Operand::read(
            program,
            parameter,
            argument,
            supplying_owner,
        )?);
    }
    bind(program, application, &operands)?;
    let table = &mut program.expression_table;
    table
        .static_machine_application_mut(application)
        .binding_declaration = target;
    table.set_static_machine_application_lifetimes(application, target, &[]);
    // The callee owns the tuple even when an operand names a caller binder.
    // Only the executable header must wait for a closed application.
    if operands.iter().any(|operand| operand.is_open(program)) {
        return Ok(None);
    }
    let closed = program
        .instantiate_requirement_application(environment, target, &trait_arguments, application)
        .map_err(unserved)?;
    // Substitution creates new occurrences in the public header. A Build may
    // supply it after source typing has already prepared the authored types.
    crate::data_applications::prepare_from(program, first_type).map_err(Stop::Rejected)?;
    Ok(Some(closed))
}
