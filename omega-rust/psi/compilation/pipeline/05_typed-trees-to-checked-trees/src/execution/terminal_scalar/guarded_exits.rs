//! Ordered scalar exits retain every reachable authored guard and selected destination.
//! Coverage is separate from evaluation: a final case guard is not a wildcard.
use super::{CheckedScalarBranchDestination, StatementNode, TransitionGuardNode};
use crate::execution::terminal_scalar::checked_branch_destination;
use checked_trees::{CheckedScalarGuardedExit, CheckedScalarGuardedTail};

pub(super) fn build(
    program: &dyn ::validation::CheckingInput,
    expressions: &checked_trees::CheckedScalarExpressionPlans,
    computations: &checked_trees::CheckedScalarComputationPlans,
) -> (
    arena::Arena<CheckedScalarGuardedExit>,
    Vec<CheckedScalarGuardedTail>,
) {
    let mut exits = arena::Arena::default();
    let mut tails = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            if let Some((arms, fallback)) = tail(program, machine, state, expressions, computations)
            {
                let mut retained = arena::HandleSpan::empty();
                for arm in arms {
                    exits.append_to_span(&mut retained, arm);
                }
                tails.push(CheckedScalarGuardedTail {
                    state: state.symbol,
                    arms: retained,
                    fallback,
                });
            }
        }
    }
    (exits, tails)
}

/// The tested subject of a `StructuralCaseMembership`/`CaseMembership`
/// guard, whichever coordinate retained it. A pure expression names its
/// parameter-field path; a computation root names the structural argument
/// it observes, which may be a non-parameter subject such as a local.
#[derive(PartialEq)]
enum CaseSubject {
    Expression(checked_trees::CheckedStructuralCaseSubject),
    Computation(checked_trees::CheckedScalarComputationStructuralArgument),
}

mod tuple_coverage;
use tuple_coverage::{computation_row, tuple_row, tuple_rows_cover};

fn tail(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expressions: &checked_trees::CheckedScalarExpressionPlans,
    computations: &checked_trees::CheckedScalarComputationPlans,
) -> Option<(
    Vec<CheckedScalarGuardedExit>,
    Option<CheckedScalarBranchDestination>,
)> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let prefix = statements
        .iter()
        .take_while(|statement| {
            matches!(
                statement,
                StatementNode::LocalData(_) | StatementNode::Assignment(_) | StatementNode::Call(_)
            )
        })
        .count();
    let mut arms = Vec::new();
    let mut fallback = None;
    let mut case_subject = None;
    let mut cases = Vec::new();
    let mut exact_cases = true;
    let mut rows = Vec::new();
    let tail = typed_trees::statement::reachable_transition_prefix(&statements[prefix..]);
    for (offset, statement) in tail.iter().enumerate() {
        let ordinal = u32::try_from(prefix.checked_add(offset)?).ok()?;
        if fallback.is_some() {
            return None;
        }
        let Some(transition) = statement.as_transition() else {
            if let StatementNode::Expression(expression) = statement
                && !arms.is_empty()
            {
                fallback = Some(
                    if validation::expression_normal_completion_possible(program, *expression)
                        == Some(false)
                    {
                        CheckedScalarBranchDestination::InvokeNoNormal {
                            statement_ordinal: ordinal,
                            is_continuation: false,
                        }
                    } else {
                        CheckedScalarBranchDestination::Return {
                            statement_ordinal: ordinal,
                            is_continuation: false,
                        }
                    },
                );
                continue;
            }
            return None;
        };
        if transition.continuation.is_valid() {
            return None;
        }
        let destination = checked_branch_destination(program, machine, ordinal, transition, false)?;
        match transition.guard {
            TransitionGuardNode::Always if !arms.is_empty() => fallback = Some(destination),
            TransitionGuardNode::When(guard) => {
                let role = checked_trees::CheckedScalarExpressionRole::Guard;
                let mut selected = expressions.expressions.iter().filter(|expression| {
                    expression.state == state.symbol
                        && expression.statement_ordinal == ordinal
                        && expression.role == role
                });
                let row;
                let case_evidence = if let Some(expression) = selected.next() {
                    if selected.next().is_some() {
                        return None;
                    }
                    let checked_trees::CheckedScalarExpression::Boolean(boolean) =
                        &expression.expression
                    else {
                        return None;
                    };
                    row = Some(tuple_row(boolean.as_ref()));
                    if let checked_trees::CheckedBooleanExpression::StructuralCaseMembership {
                        subject,
                        ..
                    } = boolean.as_ref()
                    {
                        let (_, case) =
                            crate::published::proof::exact_outcome_case_test(program, guard)?;
                        Some((CaseSubject::Expression(subject.clone()), case))
                    } else {
                        None
                    }
                } else {
                    let root = computations
                        .root_at(state.symbol, ordinal, role)
                        .filter(|root| root.machine == machine.symbol)?;
                    row = Some(computation_row(program, computations, root.root));
                    let node = computations.nodes.get(root.root);
                    if node.primitive_type != checked_trees::types::PrimitiveType::Bool {
                        return None;
                    }
                    match &node.kind {
                        checked_trees::CheckedScalarComputationKind::CaseMembership {
                            subject,
                            case,
                            ..
                        } => Some((CaseSubject::Computation(subject.clone()), *case)),
                        _ => None,
                    }
                };
                match case_evidence {
                    Some((subject, case)) => {
                        if case_subject.as_ref().is_some_and(|prior| *prior != subject) {
                            exact_cases = false;
                        }
                        case_subject = Some(subject);
                        if cases.contains(&case) {
                            exact_cases = false;
                        }
                        cases.push(case);
                    }
                    None => exact_cases = false,
                }
                rows.push(row);
                arms.push(CheckedScalarGuardedExit {
                    guard_statement_ordinal: ordinal,
                    destination,
                });
            }
            _ => return None,
        }
    }
    if arms.is_empty() {
        return None;
    }
    let complementary_pair = arms.len() == 2
        && fallback.is_none()
        && crate::execution::guard_complement::complementary(
            program,
            expressions,
            state,
            u32::try_from(prefix).ok()?,
        );
    if fallback.is_none() && !complementary_pair {
        if exact_cases {
            let first = *cases.first()?;
            let owner = program.symbols.get(first).parent;
            let data = program.data_definition_by_symbol(owner)?;
            // Coverage ranges over the owner's cases only. A mixed shape's
            // common fields are present in every case, so they select nothing
            // and an exhaustive dispatch names each variant exactly once.
            let expected = program
                .data_members(data)
                .iter()
                .filter_map(|member| match member {
                    typed_trees::data::DataMember::Variant(variant) => Some(variant.symbol),
                    typed_trees::data::DataMember::Field(_) => None,
                })
                .collect::<Vec<_>>();
            if expected.len() != cases.len() || expected.iter().any(|case| !cases.contains(case)) {
                return None;
            }
        } else {
            // A multi-subject tail has no single case owner to enumerate:
            // closure is the Cartesian question of whether the rows' finite
            // columns tile their subjects' domains.
            tuple_rows_cover(program, state, &rows)?;
        }
    }
    Some((arms, fallback))
}
