//! Result regions are operands of each call occurrence.
//! Parameter-position inference includes anonymous references; copying a fresh
//! callable binder into its caller would manufacture an undeclared lifetime.
//! The source signature remains intact for independent loan-source admission.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::TableCallExpression;
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;

pub(super) fn bindings(
    program: &TypedTrees,
    caller: &Machine,
    state: &State,
    call: &TableCallExpression,
) -> Result<Vec<(Identifier, Option<Identifier>)>, Vec<Diagnostic>> {
    let (parameters, lifetimes) = if let Some((machine, entry)) =
        validation::find_state(program, call.target_symbol)
    {
        (
            program.state_parameters(entry),
            &machine.lifetime_parameters,
        )
    } else if let Some((_, signature)) = program.machine_parameter_signature(call.target_symbol) {
        (
            program.state_signature_parameters(signature),
            &signature.lifetime_parameters,
        )
    } else if let Some(signature) = program.requirement_call_signature(caller.symbol, call) {
        (
            program.state_signature_parameters(signature),
            &signature.lifetime_parameters,
        )
    } else {
        return Ok(Vec::new());
    };
    let mut regions = program
        .expression_table
        .static_machine_application_lifetimes(call.authored_machine_application)
        .iter()
        .map(|binding| (binding.parameter.clone(), Some(binding.argument.clone())))
        .collect::<Vec<_>>();
    let mut conflict = false;
    for (parameter, argument) in parameters
        .iter()
        .zip(program.expression_table.expression_handles(call.arguments))
    {
        let Some(actual) =
            validation::expression_result_type_reference(program, caller, state, *argument)
        else {
            continue;
        };
        crate::conformance::conformance_application_lifetimes::visit_lifetime_bindings(
            program,
            parameter.type_reference,
            actual,
            lifetimes,
            &mut |parameter, actual| {
                let actual = actual.cloned();
                if let Some((_, previous)) =
                    regions.iter_mut().find(|(bound, _)| bound == parameter)
                {
                    // An elided operand does not dispute an explicit caller
                    // application. Two named inferred operands must agree.
                    if previous.is_none() {
                        *previous = actual;
                    } else if actual.is_some() && previous.as_ref() != actual.as_ref() {
                        conflict = true;
                    }
                } else {
                    regions.push((parameter.clone(), actual));
                }
            },
        );
    }
    if conflict {
        return Err(vec![Diagnostic::error(
            "call result has conflicting lifetime operands",
        )]);
    }
    Ok(regions)
}
