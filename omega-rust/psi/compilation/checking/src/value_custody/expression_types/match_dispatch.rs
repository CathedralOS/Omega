//! Ordered value-pattern compatibility and coverage. Destination checks still
//! visit every result arm, including arms whose runtime execution is excluded.
//! Result loans and owned transfers are established by their mandatory borrow
//! and multiplicity passes. A second type-only source-shape gate cannot decide
//! that custody: it rejected ordinary selected constructors before those passes
//! could join their exact leaves. Pattern and arm effects keep their selected
//! evaluation order in those same mandatory ownership checks.

use super::expression_result_type_reference as declared_value_type;
use diagnostics::Diagnostic;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, MatchPattern, TableMatchExpression,
};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::PrimitiveType;

mod structural_patterns;
pub use structural_patterns::{
    structural_match_cases_are_covered, structural_match_pattern_case,
    structural_match_pattern_is_total, structural_match_subject_type,
};

/// Find a retained subject carrier or an already-typed value-pattern peer.
/// Anonymous patterns never choose a default width.
pub fn match_subject_primitive_type(
    program: &dyn crate::CheckingInput,
    dispatch: &TableMatchExpression,
) -> Option<PrimitiveType> {
    retained_primitive(program, dispatch.subject).or_else(|| {
        program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .find_map(|arm| {
                if let MatchPattern::Value(value) = arm.pattern {
                    retained_primitive(program, value)
                } else {
                    None
                }
            })
    })
}

fn retained_primitive(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<PrimitiveType> {
    let reference = match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(_) => return Some(PrimitiveType::Bool),
        ExpressionNode::Integer(_) => {
            crate::declarations::operators::landed_integer_literal_type_reference(
                program, expression,
            )?
        }
        ExpressionNode::Name(path) => super::named_value_type_reference(program, path)?,
        ExpressionNode::Call(call) => {
            crate::machine_calls::calls::call_result_type(program, expression).or_else(|| {
                typed_trees::operator::resolve_named_expression_call(program, call)
                    .map(|operator| operator.return_type)
            })?
        }
        ExpressionNode::Cast(cast) => cast.target_type,
        ExpressionNode::Match(dispatch) => {
            return program
                .expression_table
                .match_arms(dispatch.arms)
                .iter()
                .find_map(|arm| retained_primitive(program, arm.value));
        }
        // Projections need their actual machine/state scope. The computation
        // owner has that scope and performs its ordinary operand query; do
        // not select a coincidentally compatible field from another machine.
        _ => return None,
    };
    crate::value_custody::places::unwrapped_type_reference(program, reference)
        .and_then(|reference| program.primitive_type_reference(reference))
}

/// Check scalar pattern compatibility, coverage, and every result arm before
/// an early evaluator erases the dispatch. The caller supplies a valid acyclic
/// expression graph; this check does not evaluate landed arm operations.
pub fn validate_match_dispatch(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    dispatch: &TableMatchExpression,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let arms = program.expression_table.match_arms(dispatch.arms);
    let structural = structural_match_subject_type(program, machine, state, dispatch);
    let mut covered_cases = Vec::new();
    let mut wildcard = false;
    let mut boolean_values = [false; 2];
    let subject_type = declared_value_type(program, machine, state, dispatch.subject);
    let subject_class = super::value_classification::value_class(
        program,
        Some(machine),
        Some(state),
        dispatch.subject,
    );
    let subject_boolean = subject_type
        .and_then(|reference| {
            crate::value_custody::places::unwrapped_type_reference(program, reference)
        })
        .and_then(|reference| program.primitive_type_reference(reference))
        == Some(PrimitiveType::Bool)
        || subject_class == Some(super::ValueClass::Boolean)
        || match_subject_primitive_type(program, dispatch) == Some(PrimitiveType::Bool);
    for arm in arms {
        match arm.pattern {
            MatchPattern::Wildcard => wildcard = true,
            MatchPattern::Destructure(_) | MatchPattern::Value(_) if structural.is_some() => {
                if let Some((case, total)) = structural_patterns::validate(
                    program,
                    machine,
                    state,
                    dispatch,
                    arm.pattern,
                    arm.source_span,
                    diagnostics,
                ) && total
                    && !covered_cases.contains(&case)
                {
                    covered_cases.push(case);
                }
            }
            MatchPattern::Destructure(_) => diagnostics.push(
                Diagnostic::error(
                    "structural match pattern requires a record or case-bearing subject",
                )
                .with_source_span(arm.source_span),
            ),
            MatchPattern::Value(pattern) => {
                if matches!(
                    program.expression_table.expression(pattern),
                    ExpressionNode::ArrayLiteral(_)
                        | ExpressionNode::StructLiteral(_)
                        | ExpressionNode::Range(_)
                        | ExpressionNode::Borrow(_)
                ) {
                    diagnostics.push(Diagnostic::error(
                        "match structural, range, and binding patterns are not supported by value dispatch yet",
                    ).with_source_span(arm.source_span));
                    continue;
                }
                if let Some(value) = program.expression_table.boolean_node(pattern) {
                    boolean_values[usize::from(*value)] = true;
                }
                if !is_scalar_value(program, machine, state, dispatch.subject)
                    || !is_scalar_value(program, machine, state, pattern)
                {
                    diagnostics.push(Diagnostic::error(
                        "match value patterns require scalar subjects and scalar pattern values; structural, domain, and case patterns are not supported yet",
                    ).with_source_span(arm.source_span));
                    continue;
                }
                check_compatible_values(
                    program,
                    machine,
                    state,
                    dispatch.subject,
                    pattern,
                    "match pattern is incompatible with its subject",
                    diagnostics,
                );
            }
        }
    }
    if !wildcard
        && !(subject_boolean && boolean_values == [true, true])
        && !structural.is_some_and(|reference| {
            structural_match_cases_are_covered(program, reference, &covered_cases)
        })
    {
        diagnostics.push(Diagnostic::error(
            "match does not cover its subject; cover every admitted case, both Boolean alternatives, or add a wildcard arm",
        ).with_source_span(program.expression_table.source_span(expression)));
    }
    if arms.is_empty() {
        return;
    }
    // A declared peer supplies a destination for anonymous arms. If every arm
    // remains anonymous, its enclosing storage/call/result supplies that
    // destination instead; selecting a fixed integer width here would move the
    // language's literal landing boundary.
    let peer = arms
        .iter()
        .find(|arm| declared_value_type(program, machine, state, arm.value).is_some())
        .unwrap_or(&arms[0])
        .value;
    if let Some(destination) = declared_value_type(program, machine, state, peer) {
        for arm in arms {
            crate::proof_contracts::arithmetic_domains::validate_anonymous_integer_range(
                program,
                destination,
                arm.value,
                "typed match result",
                diagnostics,
            );
        }
    }
    for arm in arms {
        check_compatible_values(
            program,
            machine,
            state,
            peer,
            arm.value,
            "match arms produce incompatible values",
            diagnostics,
        );
    }
}

fn is_scalar_value(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    if let Some(reference) = declared_value_type(program, machine, state, expression) {
        return crate::value_custody::places::unwrapped_type_reference(program, reference)
            .and_then(|reference| program.primitive_type_reference(reference))
            .is_some();
    }
    if let Some(class) =
        super::value_classification::value_class(program, Some(machine), Some(state), expression)
    {
        return matches!(
            class,
            super::ValueClass::Numeric | super::ValueClass::Boolean
        );
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            let arms = program.expression_table.match_arms(dispatch.arms);
            !arms.is_empty()
                && arms
                    .iter()
                    .all(|arm| is_scalar_value(program, machine, state, arm.value))
        }
        ExpressionNode::Binary(_) => {
            crate::has_builtin_bound_expression_meaning(program, machine, Some(state), expression)
        }
        _ => false,
    }
}

fn check_compatible_values(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    peer: ExpressionHandle,
    value: ExpressionHandle,
    message: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(dispatch) = program.expression_table.match_node(value) {
        for arm in program.expression_table.match_arms(dispatch.arms) {
            check_compatible_values(
                program,
                machine,
                state,
                peer,
                arm.value,
                message,
                diagnostics,
            );
        }
        return;
    }
    if let Some(dispatch) = program.expression_table.match_node(peer) {
        for arm in program.expression_table.match_arms(dispatch.arms) {
            check_compatible_values(
                program,
                machine,
                state,
                arm.value,
                value,
                message,
                diagnostics,
            );
        }
        return;
    }
    let data_symbol = |expression| match program.expression_table.expression(expression) {
        ExpressionNode::StructLiteral(literal) if literal.type_symbol.is_valid() => {
            Some(literal.type_symbol)
        }
        _ => None,
    };
    let peer_data = data_symbol(peer);
    let value_data = data_symbol(value);
    // Comparing a reference-backed scalar observes its referent. Result
    // custody is checked separately and never established by this read view.
    let peer_type = declared_value_type(program, machine, state, peer)
        .map(|reference| crate::value_custody::places::assignment_value_type(program, reference));
    let value_type = declared_value_type(program, machine, state, value)
        .map(|reference| crate::value_custody::places::assignment_value_type(program, reference));
    let mismatch = if let (Some(peer_type), Some(value_type)) = (peer_type, value_type) {
        // Numeric range predicates may weaken at a join, but semantic policy
        // may not. The result query retains only predicates shared by every
        // arm; accepting these operands does not export either arm's range.
        program.normalized_type_identity(peer_type) != program.normalized_type_identity(value_type)
            && !matches!(
                (super::result_type::arithmetic_carrier(program, peer_type),
                 super::result_type::arithmetic_carrier(program, value_type)),
                (Some(peer), Some(value)) if peer == value
            )
    } else if let Some(reference) = peer_type.or(value_type) {
        let anonymous = if peer_type.is_some() { value } else { peer };
        crate::value_custody::literals::validate_suffix_landing(
            program,
            anonymous,
            reference,
            diagnostics,
        );
        !super::argument_matches_type_reference_handle(program, anonymous, reference)
    } else {
        let peer_class =
            super::value_classification::value_class(program, Some(machine), Some(state), peer);
        let value_class =
            super::value_classification::value_class(program, Some(machine), Some(state), value);
        let class_mismatch = peer_class
            .zip(value_class)
            .is_some_and(|(peer, value)| peer != value);
        let data_mismatch = match (peer_data, value_data) {
            (Some(peer), Some(value)) => peer != value,
            (Some(_), None) => {
                value_class.is_some()
                    || matches!(
                        program.expression_table.expression(value),
                        ExpressionNode::ArrayLiteral(_)
                    )
            }
            (None, Some(_)) => {
                peer_class.is_some()
                    || matches!(
                        program.expression_table.expression(peer),
                        ExpressionNode::ArrayLiteral(_)
                    )
            }
            (None, None) => false,
        };
        class_mismatch || data_mismatch
    };
    if mismatch {
        diagnostics.push(
            Diagnostic::error(message)
                .with_source_span(program.expression_table.source_span(value)),
        );
    }
}
