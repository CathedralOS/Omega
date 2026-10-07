//! A static binder calls its complete declared parameter roster explicitly.
//!
//! Nominal selection and its complete effect and proof contract remain with
//! the ordinary binder admission owner; every argument is checked against its
//! declared parameter type.

use super::*;
use typed_trees::signature::StateSignature;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_machine_parameter_arguments(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    value_environment: &ValueEnvironment,
    arguments: &[ExpressionHandle],
    signature: &StateSignature,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_call_arguments_with_type_correspondence(
        program,
        machine,
        state,
        value_environment,
        arguments,
        signature.name.as_str(),
        program.state_signature_parameters(signature),
        writable_roots,
        false,
        &[],
        |argument, required| argument_matches_carrier(program, argument, required),
        diagnostics,
    );
}
