//! Explain an existing mixed-frontier rejection without selecting custody.
//! Raw guard descriptors and authenticated completed backing coordinates are
//! distinct observations. Reuse the production guard traversal and reaching-
//! store/loan query; neither a matching label nor a missing claim proves absence.

use super::super::linear_obligations::LinearPlace;
use checked_trees::{CheckFacts, FlowClaimJoinExit, FlowClaimJoinExitKind};
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::statement::{StatementNode, TransitionGuardNode};
use validation::CanonicalPlace;

pub(super) fn exit_detail(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    coordinate: FlowClaimJoinExit,
    place: &LinearPlace,
) -> String {
    let Some(StatementNode::Transition(transition)) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(coordinate.statement_index)
    else {
        return String::new();
    };
    let positive = match coordinate.kind {
        FlowClaimJoinExitKind::TransitionTarget => true,
        FlowClaimJoinExitKind::TransitionContinuation => false,
        _ => return String::new(),
    };
    let TransitionGuardNode::When(guard) = transition.guard else {
        return "; case selection: unconditional guard has no case test".to_owned();
    };
    let mut rows = Vec::new();
    // Keep the diagnostic bounded independently of the number of conjuncts.
    // Every visited test still follows the production traversal below.
    super::super::permission_events::visit_positive_case_tests(
        program,
        state.symbol,
        guard,
        &mut |expression, test| {
            if rows.len() < 4 {
                rows.push(match test {
                    Some((subject, variant)) => selected_subject_detail(
                        program,
                        facts,
                        state,
                        coordinate.statement_index,
                        expression,
                        subject,
                        variant,
                        place,
                    ),
                    None => unavailable_membership_detail(program, state, expression),
                });
            }
        },
    );
    let polarity = if positive {
        "selected guard"
    } else if super::super::permission_events::exact_positive_case_test(
        program,
        state.symbol,
        guard,
    )
    .is_some()
    {
        "missed exact guard"
    } else {
        "missed guard is not one exact positive case test"
    };
    format!("; case selection ({polarity}): {}", rows.join(" | "))
}

fn unavailable_membership_detail(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> String {
    let label = program.expression_table.display_name(expression);
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return format!("`{label}` has no binary case-membership operation");
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::CaseMembership
    ) {
        return format!("`{label}` is not a positive case-membership comparison");
    }
    let authenticated = program
        .machine_holding_state(state.symbol)
        .is_some_and(|machine| {
            validation::has_exact_case_membership_meaning(
                program,
                machine,
                Some(state),
                expression,
                binary,
            )
        });
    if authenticated {
        format!("`{label}` authenticates membership but cannot resolve its applied case")
    } else {
        format!("`{label}` has unavailable case-membership authentication")
    }
}

#[allow(clippy::too_many_arguments)]
fn selected_subject_detail(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statement: usize,
    expression: ExpressionHandle,
    subject: ExpressionHandle,
    variant: SymbolHandle,
    place: &LinearPlace,
) -> String {
    let direct = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement,
        subject,
    );
    let describe = |subject: &CanonicalPlace| {
        let relation = if subject.root != place.root || !place.path.starts_with(&subject.segments) {
            "authenticated unequal canonical subject"
        } else {
            match place.path.get(subject.segments.len()) {
                Some(facts::PlaceSegment::Case { variant: candidate }) if *candidate == variant => {
                    "compatible selected case"
                }
                Some(facts::PlaceSegment::Case { .. }) => "other case under this subject",
                _ => "subject does not guard this claim path",
            }
        };
        format!("`{}`: {relation}", subject.label(program))
    };
    let direct = direct.as_ref().map_or_else(
        || "canonical guard subject unavailable".to_owned(),
        |subject| describe(subject),
    );
    let Some(machine) = program.machine_holding_state(state.symbol) else {
        return format!(
            "case `{}`; {direct}; owning machine unavailable",
            program.symbols.name(variant)
        );
    };
    let completed = match crate::flow::captured_case_subject_result(
        program,
        facts.into(),
        machine,
        state,
        statement,
        expression,
    ) {
        Ok(captured) => {
            let source = facts.flow.control.operands.get(captured.source);
            validation::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                source.statement_index,
                source.expression,
            )
            .as_ref()
            .map_or_else(
                || "completed backing has no canonical source coordinate".to_owned(),
                |source| format!("completed backing {}", describe(source)),
            )
        }
        Err(reason) => format!(
            "completed backing correspondence unavailable: {}",
            reason.reason()
        ),
    };
    format!(
        "case `{}`; {direct}; {completed}",
        program.symbols.name(variant)
    )
}
