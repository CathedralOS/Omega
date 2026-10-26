//! Compiler-supplied policy arguments retain the policy's actual signature.
//!
//! Layout, access and wire scheduling supply schema/plan values, and a policy
//! may also declare private working storage as its explicit data subject. The
//! context is an ordinary positional value at the resolved subject
//! position. Neither the qualifier's spelling nor a parameter named `self`
//! selects that position. Every invocation gets fresh storage, so policy
//! mutations cannot escape into another schema evaluation.
//!
//! The adapter establishes zero for the declared subject before materializing
//! it. The shared supplied-invocation admission then establishes and projects
//! these values, proves entry contracts and checks the execution floor before
//! interpretation. Argument order and returned-plan validation remain with the
//! policy consumer. Unclaimed reflection metadata is not a declared field.

use build_time_values::BuildTimeValue;
use typed_trees::{TypedTrees, machine::Machine, types::TypeReferenceNode};

pub(super) fn policy_arguments(
    typed: &TypedTrees,
    machine: &Machine,
    mut arguments: Vec<BuildTimeValue>,
) -> Result<Vec<BuildTimeValue>, String> {
    let entry = typed
        .machine_states(machine)
        .first()
        .ok_or_else(|| format!("policy `{}` has no entry state", machine.name.as_str()))?;
    let parameters = typed.state_parameters(entry);
    let positional_count = parameters.len();
    if positional_count != arguments.len() {
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
            .data_definition_by_symbol(*symbol)
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
        let context = checked_interpreter::materialize_zero_initialized_build_time_value(
            typed,
            context_type,
        )?;
        arguments.insert(position, context);
    }
    Ok(arguments)
}
