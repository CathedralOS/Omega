//! A static binder calls its complete declared parameter roster explicitly.
//!
//! Nominal selection and its complete effect and proof contract remain with
//! the ordinary binder admission owner; every argument is checked against its
//! declared parameter type.

use super::*;
use typed_trees::signature::StateSignature;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_machine_parameter_arguments(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    value_environment: &ValueEnvironment,
    arguments: &[ExpressionHandle],
    signature: &StateSignature,
    static_arguments: &[typed_trees::expression::StaticMachineArgument],
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(bindings) = type_arguments::callable_type_arguments(
        program,
        machine,
        state,
        program.state_signature_type_parameters(signature),
        program.state_signature_parameters(signature),
        static_arguments,
        arguments,
    ) else {
        diagnostics.push(Diagnostic::error(format!(
            "call `{}` has an invalid static type argument correspondence",
            signature.name
        )));
        return;
    };
    // Ordinary entries have a producer-retained application checked by static
    // call admission. A symbolic callable invocation owns its local telescope
    // here: an unused type slot cannot disappear merely because no operand
    // mentions it. Forwarding a caller binder is a binding, absence is not.
    if program
        .state_signature_type_parameters(signature)
        .iter()
        .any(|parameter| {
            matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                && !bindings
                    .iter()
                    .any(|(symbol, _)| *symbol == parameter.symbol)
        })
    {
        diagnostics.push(Diagnostic::error(format!(
            "call `{}` has an invalid static type argument correspondence",
            signature.name
        )));
        return;
    }
    validate_call_arguments(
        program,
        machine,
        state,
        value_environment,
        arguments,
        signature.name.as_str(),
        signature.name.as_str(),
        program.state_signature_parameters(signature),
        writable_roots.statements,
        false,
        &[],
        |argument, required| {
            argument_matches_carrier(program, machine.symbol, argument, required, &bindings)
        },
        diagnostics,
    );
}
