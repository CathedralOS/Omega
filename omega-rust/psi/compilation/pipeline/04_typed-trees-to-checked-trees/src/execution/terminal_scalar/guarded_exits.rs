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

/// One row of a decomposed multi-subject guard: the finite columns an
/// authored tuple arm constrains. `Or` unions widen a single column;
/// conjuncts outside the column vocabulary leave `residual` — the row
/// then proves no cylinder and can only consume coverage other rows
/// provide.
#[derive(Default)]
struct TupleRow {
    columns: Vec<(TupleSubject, Vec<TupleValue>)>,
    residual: bool,
}

/// The operand one matrix column selects on. A case column is a
/// structural path whose active variant the arm requires; a Boolean
/// column is a scalar operand the arm pins to a literal.
#[derive(PartialEq)]
enum TupleSubject {
    Case(checked_trees::CheckedStructuralCaseSubject),
    Boolean(checked_trees::CheckedBooleanExpression),
}

#[derive(Clone, PartialEq)]
enum TupleValue {
    Case(checked_trees::CheckedMemberIdentity),
    Boolean(bool),
}

/// Conjunction semantics merge on a shared subject: two columns naming
/// the same operand intersect their admitted values.
fn merge_column(row: &mut TupleRow, subject: TupleSubject, allowed: Vec<TupleValue>) {
    if let Some((_, present)) = row
        .columns
        .iter_mut()
        .find(|(candidate, _)| *candidate == subject)
    {
        present.retain(|value| allowed.contains(value));
    } else {
        row.columns.push((subject, allowed));
    }
}

/// Decompose one `when` guard into its tuple-matrix row. Recognized
/// conjuncts are exact case memberships, same-subject `Or` unions of
/// them, and `operand == <Boolean literal>` equalities; anything else is
/// a residual the coverage pass cannot model.
fn tuple_row(boolean: &checked_trees::CheckedBooleanExpression) -> TupleRow {
    let mut row = TupleRow::default();
    tuple_conjunction(boolean, &mut row);
    row
}

fn tuple_conjunction(boolean: &checked_trees::CheckedBooleanExpression, row: &mut TupleRow) {
    use checked_trees::CheckedBooleanExpression as Boolean;
    match boolean {
        Boolean::And { left, right } => {
            tuple_conjunction(left, row);
            tuple_conjunction(right, row);
        }
        Boolean::StructuralCaseMembership { subject, case } => {
            merge_column(
                row,
                TupleSubject::Case(subject.clone()),
                vec![TupleValue::Case(case.clone())],
            );
        }
        Boolean::Or { .. } => {
            // A domain membership lowers to a case disjunction on one
            // subject; mixing subjects or non-membership leaves is not a
            // column.
            let mut pending = vec![boolean];
            let mut subject: Option<&checked_trees::CheckedStructuralCaseSubject> = None;
            let mut cases = Vec::new();
            while let Some(node) = pending.pop() {
                match node {
                    Boolean::Or { left, right } => {
                        pending.push(left);
                        pending.push(right);
                    }
                    Boolean::StructuralCaseMembership {
                        subject: leaf,
                        case,
                    } => {
                        if subject.is_some_and(|prior| *prior != *leaf) {
                            row.residual = true;
                            return;
                        }
                        subject = Some(leaf);
                        if !cases.contains(case) {
                            cases.push(case.clone());
                        }
                    }
                    _ => {
                        row.residual = true;
                        return;
                    }
                }
            }
            let Some(subject) = subject else {
                row.residual = true;
                return;
            };
            merge_column(
                row,
                TupleSubject::Case(subject.clone()),
                cases.into_iter().map(TupleValue::Case).collect(),
            );
        }
        Boolean::Equal { left, right } => {
            let (operand, literal) = match (left.as_ref(), right.as_ref()) {
                (operand, Boolean::Constant(literal)) | (Boolean::Constant(literal), operand) => {
                    (operand, *literal)
                }
                _ => {
                    row.residual = true;
                    return;
                }
            };
            merge_column(
                row,
                TupleSubject::Boolean(operand.clone()),
                vec![TupleValue::Boolean(literal)],
            );
        }
        _ => row.residual = true,
    }
}

/// Whether the decomposed tuple rows provably tile their subjects'
/// finite domains. Each case subject's domain enumerates through the
/// data owner its path resolves to; a Boolean subject's domain is the
/// two literals. A row covers a tuple when every one of its columns
/// admits the tuple's value — a subject the row does not constrain is a
/// wildcard — and rows carrying a residual prove no slice.
fn tuple_rows_cover(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    rows: &[Option<TupleRow>],
) -> Option<()> {
    // An unclassified/effectful guard proves no coverage. Other authored
    // rows can still cover every remaining value; prior alternatives may
    // select an outcome but cannot prevent a later total row from selecting.
    let rows: Vec<&TupleRow> = rows.iter().filter_map(Option::as_ref).collect();
    let mut subjects = Vec::new();
    for row in &rows {
        for (subject, _) in &row.columns {
            crate::collections::push_unique(&mut subjects, subject);
        }
    }
    if subjects.is_empty() {
        return None;
    }
    let parameters = program.state_parameters(state);
    let mut domains = Vec::with_capacity(subjects.len());
    for subject in &subjects {
        match subject {
            TupleSubject::Case(field) => {
                let field = field.parameter_field()?;
                let (_, _, receiver, _) = crate::values::resolve_structural_parameter_path(
                    program,
                    parameters,
                    field.parameter_position,
                    &field.path,
                )?;
                let definition = crate::values::structural_data(program, receiver)?;
                let cases: Vec<TupleValue> = program
                    .data_members(definition)
                    .iter()
                    .filter_map(|member| match member {
                        typed_trees::data::DataMember::Variant(variant) => Some(TupleValue::Case(
                            checked_trees::CheckedMemberIdentity::of_member(
                                variant.identity,
                                variant.name.clone(),
                            ),
                        )),
                        typed_trees::data::DataMember::Field(_) => None,
                    })
                    .collect::<Vec<_>>();
                if cases.is_empty() {
                    return None;
                }
                domains.push(cases);
            }
            TupleSubject::Boolean(_) => {
                domains.push(vec![TupleValue::Boolean(false), TupleValue::Boolean(true)]);
            }
        }
    }
    // Exhaustive tuple-matrix checks stay small by construction; a
    // product beyond the cap is out of scope for this pass.
    let extent: usize = domains.iter().map(Vec::len).product();
    if extent == 0 || extent > 4096 {
        return None;
    }
    let mut indexes = vec![0usize; domains.len()];
    loop {
        let covered = rows.iter().any(|row| {
            !row.residual
                && row.columns.iter().all(|(subject, allowed)| {
                    subjects
                        .iter()
                        .position(|candidate| *candidate == subject)
                        .is_some_and(|position| {
                            allowed.contains(&domains[position][indexes[position]])
                        })
                })
        });
        if !covered {
            return None;
        }
        let mut axis = domains.len();
        loop {
            if axis == 0 {
                return Some(());
            }
            axis -= 1;
            indexes[axis] += 1;
            if indexes[axis] < domains[axis].len() {
                break;
            }
            indexes[axis] = 0;
        }
    }
}

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
    let mut case_columns: Vec<(CaseSubject, Vec<symbols::SymbolHandle>)> = Vec::new();
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
        let destination = checked_branch_destination(program, machine, ordinal, transition, false);
        let destination = destination?;
        match transition.guard {
            TransitionGuardNode::Always if !arms.is_empty() => fallback = Some(destination),
            TransitionGuardNode::When(guard) => {
                let role = checked_trees::CheckedScalarExpressionRole::Guard;
                let mut selected = expressions.expressions.iter().filter(|expression| {
                    expression.state == state.symbol
                        && expression.statement_ordinal == ordinal
                        && expression.role == role
                });
                let mut row = None;
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
                        let (_, case) = crate::proof::exact_outcome_case_test(program, guard)?;
                        Some((CaseSubject::Expression(subject.clone()), case))
                    } else {
                        None
                    }
                } else {
                    let root = computations
                        .root_at(state.symbol, ordinal, role)
                        .filter(|root| root.machine == machine.symbol)?;
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
                if let Some((subject, case)) = case_evidence {
                    if let Some((_, cases)) = case_columns
                        .iter_mut()
                        .find(|(candidate, _)| *candidate == subject)
                    {
                        crate::collections::push_unique(cases, case);
                    } else {
                        case_columns.push((subject, vec![case]));
                    }
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
    if fallback.is_none()
        && !complementary_pair
        && !case_columns
            .iter()
            .any(|(_, cases)| complete_case_column(program, cases))
    {
        // Independent finite columns may establish coverage together even
        // when no single discriminator covers every admitted value.
        tuple_rows_cover(program, state, &rows)?;
    }
    Some((arms, fallback))
}

/// Pure case guards on one exact completed subject can cover the carrier even
/// with extra guarded/effectful alternatives interleaved. Those alternatives
/// supply no coverage, and keep their original ordered executable outcomes.
fn complete_case_column(
    program: &dyn validation::CheckingInput,
    cases: &[symbols::SymbolHandle],
) -> bool {
    let Some(first) = cases.first().copied() else {
        return false;
    };
    let owner = program.symbols.get(first).parent;
    let Some(data) = program.data_definition_by_symbol(owner) else {
        return false;
    };
    let expected = program
        .data_members(data)
        .iter()
        .filter_map(typed_trees::data::DataMember::as_variant)
        .map(|case| case.symbol)
        .collect::<Vec<_>>();
    !expected.is_empty()
        && cases.iter().all(|case| expected.contains(case))
        && expected.iter().all(|case| cases.contains(case))
}
