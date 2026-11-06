//! Wording for every obligation the checker cannot prove.

use crate::checker::float_ranges::float_range_display;
use crate::obligations::{
    BoundedAssignmentObligation, BoundedCallArgumentObligation, BoundedInitializerObligation,
    BoundedStateReturnObligation, BoundedTransitionArgumentObligation, FloatRange, IntegerRange,
    ProofPlan,
};
pub(crate) use diagnostics::Diagnostic;
use typed_trees::expression::ExpressionHandle;

pub(crate) fn cannot_prove_bounded_transition_integer(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove transition argument `{}` satisfies bounded parameter `{}` in `{}.{}`; expected {}..={}",
        expression_display_name(proof_plan, obligation.argument),
        obligation.parameter,
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

pub(crate) fn cannot_prove_bounded_assignment_integer(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove assignment value `{}` satisfies bounded target `{}` in `{}.{}`; expected {}..={}",
        expression_display_name(proof_plan, obligation.value),
        expression_display_name(proof_plan, obligation.target),
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

pub(crate) fn cannot_prove_bounded_return_integer(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove return value `{}` satisfies bounded return type in `{}.{}`; expected {}..={}",
        expression_display_name(proof_plan, obligation.value),
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

pub(crate) fn cannot_prove_bounded_initializer_integer(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove initializer `{}` satisfies the bounded value of {}; expected {}..={}",
        expression_display_name(proof_plan, obligation.value),
        obligation.owner,
        target_range.minimum,
        target_range.maximum
    ))
}

pub(crate) fn cannot_prove_bounded_transition_float(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    target_range: FloatRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove transition argument `{}` satisfies bounded parameter `{}` in `{}.{}`; {}",
        expression_display_name(proof_plan, obligation.argument),
        obligation.parameter,
        obligation.machine,
        obligation.state,
        float_range_display(target_range)
    ))
}

pub(crate) fn cannot_prove_bounded_assignment_float(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    target_range: FloatRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove assignment value `{}` satisfies bounded target `{}` in `{}.{}`; {}",
        expression_display_name(proof_plan, obligation.value),
        expression_display_name(proof_plan, obligation.target),
        obligation.machine,
        obligation.state,
        float_range_display(target_range)
    ))
}

pub(crate) fn cannot_prove_bounded_return_float(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
    target_range: FloatRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove return value `{}` satisfies bounded return type in `{}.{}`; {}",
        expression_display_name(proof_plan, obligation.value),
        obligation.machine,
        obligation.state,
        float_range_display(target_range)
    ))
}

pub(crate) fn cannot_prove_bounded_initializer_float(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    target_range: FloatRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove initializer `{}` satisfies the bounded value of {}; {}",
        expression_display_name(proof_plan, obligation.value),
        obligation.owner,
        float_range_display(target_range)
    ))
}

pub(crate) fn cannot_prove_transition_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    constraint: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove transition argument `{}` satisfies `{}` for bounded parameter `{}` in `{}.{}`",
        expression_display_name(proof_plan, obligation.argument),
        constraint,
        obligation.parameter,
        obligation.machine,
        obligation.state
    ))
}

pub(crate) fn cannot_prove_assignment_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    constraint: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove assignment value `{}` satisfies `{}` for bounded target `{}` in `{}.{}`",
        expression_display_name(proof_plan, obligation.value),
        constraint,
        expression_display_name(proof_plan, obligation.target),
        obligation.machine,
        obligation.state
    ))
}

pub(crate) fn cannot_prove_return_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
    constraint: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove return value `{}` satisfies `{}` for bounded return type in `{}.{}`",
        expression_display_name(proof_plan, obligation.value),
        constraint,
        obligation.machine,
        obligation.state
    ))
}

pub(crate) fn cannot_prove_initializer_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    constraint: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove initializer `{}` satisfies `{}` for the bounded value of {}",
        expression_display_name(proof_plan, obligation.value),
        constraint,
        obligation.owner
    ))
}

pub(crate) fn expression_display_name(
    proof_plan: &ProofPlan,
    expression: ExpressionHandle,
) -> String {
    proof_plan.program.expression_table.display_name(expression)
}

/// `receiver.target` when the call names a receiver, else the bare target --
/// the callee spelling all three call-argument refusals print.
fn call_argument_target(obligation: &BoundedCallArgumentObligation) -> String {
    obligation
        .receiver
        .as_ref()
        .map(|receiver| format!("{receiver}.{}", obligation.target))
        .unwrap_or_else(|| obligation.target.to_string())
}

pub(crate) fn cannot_prove_bounded_call_integer(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    let target = call_argument_target(obligation);

    Diagnostic::error(format!(
        "cannot prove call argument `{}` satisfies bounded parameter `{}` for `{}` in `{}.{}`; expected {}..={}",
        expression_display_name(proof_plan, obligation.argument),
        obligation.parameter,
        target,
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

pub(crate) fn cannot_prove_bounded_call_float(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    target_range: FloatRange,
) -> Diagnostic {
    let target = call_argument_target(obligation);

    Diagnostic::error(format!(
        "cannot prove call argument `{}` satisfies bounded parameter `{}` for `{}` in `{}.{}`; {}",
        expression_display_name(proof_plan, obligation.argument),
        obligation.parameter,
        target,
        obligation.machine,
        obligation.state,
        float_range_display(target_range)
    ))
}

pub(crate) fn cannot_prove_call_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    constraint: &str,
) -> Diagnostic {
    let target = call_argument_target(obligation);

    Diagnostic::error(format!(
        "cannot prove call argument `{}` satisfies `{}` for bounded parameter `{}` for `{}` in `{}.{}`",
        expression_display_name(proof_plan, obligation.argument),
        constraint,
        obligation.parameter,
        target,
        obligation.machine,
        obligation.state
    ))
}
