//! Compiler-supplied policy arguments retain the policy's actual signature.
//!
//! Layout, access and wire scheduling supply schema/plan values, and a policy
//! may also declare private working storage as its explicit data subject. The
//! context is an ordinary positional value at the resolved subject
//! position. Neither the qualifier's spelling nor a parameter named `self`
//! selects that position. Every invocation gets fresh storage, so policy
//! mutations cannot escape into another schema evaluation.
//!
//! This adapter runs after ordinary Build admission. It establishes zero for
//! the declared subject before materializing it, and preserves all supplied
//! arguments and their order. Execution and returned-plan validation remain
//! with the existing interpreter and policy consumers.

use build_time_values::BuildTimeValue;
use checked_interpreter::BuildMachineEvaluationRequest;
use typed_trees::{TypedTrees, machine::Machine, types::TypeReferenceNode};

pub(super) fn policy_request<'a>(
    typed: &TypedTrees,
    machine: &Machine,
    mut arguments: Vec<BuildTimeValue>,
) -> Result<BuildMachineEvaluationRequest<'a>, String> {
    let entry = typed
        .machine_states(machine)
        .first()
        .ok_or_else(|| format!("policy `{}` has no entry state", machine.name.as_str()))?;
    let parameters = typed.state_parameters(entry);
    let positional_count = parameters.len();
    if positional_count == arguments.len() {
        return Ok(BuildMachineEvaluationRequest::symbol(
            machine.symbol,
            arguments,
        ));
    }
    let missing_context = || {
        format!(
            "policy `{}` needs one unique declared data context in addition to its {} supplied argument(s)",
            machine.name.as_str(),
            arguments.len()
        )
    };
    let position = typed
        .data_binding_position(machine.attached_data_symbol, parameters)
        .ok_or_else(missing_context)?;
    let parameter = &parameters[position];
    if positional_count != arguments.len() + 1 {
        return Err(missing_context());
    }
    let context_type = match typed
        .type_reference_table
        .type_reference(parameter.type_reference)
    {
        TypeReferenceNode::Reference { referee, .. } => *referee,
        _ => parameter.type_reference,
    };
    let TypeReferenceNode::Named { symbol, .. } =
        typed.type_reference_table.type_reference(context_type)
    else {
        return Err(format!(
            "policy `{}` context must name its declared data type",
            machine.name.as_str()
        ));
    };
    let data = typed
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *symbol)
        .ok_or_else(missing_context)?;
    if validation::data_requires_establishment(typed, data)
        || !validation::zeroed_structural_parts(typed, context_type)
    {
        return Err(format!(
            "policy `{}` context `{}` is not an established zero-initialized value",
            machine.name.as_str(),
            parameter.name.as_str()
        ));
    }
    let context =
        checked_interpreter::materialize_zero_initialized_build_time_value(typed, context_type)?;
    arguments.insert(position, context);
    Ok(BuildMachineEvaluationRequest::symbol(
        machine.symbol,
        arguments,
    ))
}
