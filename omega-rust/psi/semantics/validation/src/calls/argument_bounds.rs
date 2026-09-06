//! Numeric parameter delivery consumes the caller's evaluated argument bounds.

use crate::arithmetic_domains::{self, ValueEnv};
use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;

/// Check both carrier width and the declared range at the resolved parameter.
/// A wholly anonymous argument first lands here; its integrality diagnostics
/// cannot be discarded as if an earlier typed operation had already checked it.
pub(super) fn report_argument_bounds(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&State>,
    value_env: &ValueEnv,
    argument: ExpressionHandle,
    parameter: &StateParameter,
    target_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(parameter_primitive) = program.primitive_type_reference(parameter.type_reference)
    else {
        return;
    };
    let owner = format!(
        "machine `{}` state `{target_name}` argument `{}`",
        current_machine.name, parameter.name,
    );
    let before = diagnostics.len();
    if let Some((interval, _)) = arithmetic_domains::validate_anonymous_integer_range(
        program,
        parameter.type_reference,
        argument,
        &owner,
        diagnostics,
    ) {
        if diagnostics.len() == before {
            arithmetic_domains::check_range_containment(
                program,
                parameter.type_reference,
                interval,
                &owner,
                diagnostics,
            );
        }
        return;
    }
    arithmetic_domains::check_value_narrowing(
        program,
        current_machine,
        current_state,
        argument,
        parameter_primitive,
        value_env,
        &owner,
        diagnostics,
    );
    // Parameter delivery is a store too. Width compatibility alone does not
    // establish the narrower declared range that the callee's reads assume.
    if arithmetic_domains::enforced_declared_range_interval(program, parameter.type_reference)
        .is_some()
    {
        let mut arithmetic_diagnostics = Vec::new();
        let (interval, _) = arithmetic_domains::validate_value_range(
            program,
            current_machine,
            current_state,
            argument,
            value_env,
            Some(parameter_primitive),
            ArithmeticDomain::Exact,
            &owner,
            &mut arithmetic_diagnostics,
        );
        if arithmetic_diagnostics.is_empty() {
            arithmetic_domains::check_range_containment(
                program,
                parameter.type_reference,
                interval,
                &owner,
                diagnostics,
            );
        }
    }
}
