//! Compiler-supplied policy arguments retain the policy's actual signature.
//!
//! Layout, access and wire scheduling supply schema/plan values, and a policy
//! may also declare private working storage as its explicit data subject. The
//! old receiver form obtains that storage from the interpreter's instance;
//! the static form needs an ordinary positional value at the resolved subject
//! position. Neither the qualifier's spelling nor a parameter named `self`
//! selects that position. Every invocation gets fresh storage, so policy
//! mutations cannot escape into another schema evaluation.
//!
//! This adapter runs after ordinary Build admission. It establishes zero for
//! the declared subject before materializing it, and preserves all supplied
//! arguments and their order. Execution and returned-plan validation remain
//! with the existing interpreter and policy consumers.

use checked_interpreter::{BuildMachineEvaluationRequest, BuildTimeValue};
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
    let positional_count = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
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
    if parameter.is_self || positional_count != arguments.len() + 1 {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn evaluate(source: &str, arguments: Vec<BuildTimeValue>) -> Result<BuildTimeValue, String> {
        let typed = crate::front_end::typed_program(source);
        let machine = typed.realized_machine_named("Scratch::plan").unwrap();
        crate::BuildTimeAdmissionPlan::infer(&typed, None).require_common_floor(&typed, machine)?;
        let request = policy_request(&typed, machine, arguments)?;
        checked_interpreter::evaluate_build_time_machine(&typed, request)
            .map(checked_interpreter::BuildTimeOperationEvaluation::into_value)
    }

    #[test]
    fn explicit_context_uses_its_position_and_is_fresh_for_each_invocation() {
        let source = r#"
            data Scratch { count: u64; }
            machine Scratch::plan(input: u64, work: &mut Scratch) -> u64 {
                work.count = work.count + input;
                work.count
            }
        "#;
        let typed = crate::front_end::typed_program(source);
        let machine = typed.realized_machine_named("Scratch::plan").unwrap();
        crate::BuildTimeAdmissionPlan::infer(&typed, None)
            .require_common_floor(&typed, machine)
            .unwrap();
        for _ in 0..2 {
            let request = policy_request(&typed, machine, vec![BuildTimeValue::Int(7)]).unwrap();
            assert_eq!(
                checked_interpreter::evaluate_build_time_machine(&typed, request)
                    .unwrap()
                    .into_value(),
                BuildTimeValue::Int(7)
            );
        }
    }

    #[test]
    fn policies_without_a_context_keep_their_supplied_arguments() {
        assert_eq!(
            evaluate(
                r#"
            data Scratch { }
            machine Scratch::plan(input: u64) -> u64 { input }
        "#,
                vec![BuildTimeValue::Int(11)]
            )
            .unwrap(),
            BuildTimeValue::Int(11)
        );
    }

    #[test]
    fn context_whose_default_domain_excludes_zero_is_not_fabricated() {
        let error = evaluate(
            r#"
            data Scratch where count >= 1 { count: u64; }
            machine Scratch::plan(work: &mut Scratch, input: u64) -> u64 { input }
        "#,
            vec![BuildTimeValue::Int(7)],
        )
        .unwrap_err();
        assert!(
            error.contains("not an established zero-initialized value"),
            "{error}"
        );
    }

    #[test]
    fn context_with_a_reference_field_is_not_fabricated() {
        let error = evaluate(
            r#"
            data Scratch { bytes: &[u8]; }
            machine Scratch::plan(work: &mut Scratch, input: u64) -> u64 { input }
        "#,
            vec![BuildTimeValue::Int(7)],
        )
        .unwrap_err();
        assert!(
            error.contains("not an established zero-initialized value"),
            "{error}"
        );
    }
}
