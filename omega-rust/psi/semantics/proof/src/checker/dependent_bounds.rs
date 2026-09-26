//! Bounds that depend on sibling fields, dependent call fields and lengths.

use crate::checker::diagnostics::expression_display_name;
use crate::obligations::{
    BoundedCallArgumentObligation, BoundedTransitionArgumentObligation, ProofConstraint, ProofPlan,
};
use language_core::{receiver_place_field, receiver_place_label};
use language_semantics::declaration_selection::CollectionMeasure;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::name::Identifier;
use typed_trees::statement::TransitionGuardNode;

/// The single dependent-maximum atom of a constraint set (R1a mints at most
/// one: the declared range's own bound).
pub(crate) fn symbolic_max_from_constraints(
    constraints: &[ProofConstraint],
) -> Option<(i64, &Identifier, i64)> {
    constraints.iter().find_map(|constraint| match constraint {
        ProofConstraint::IntegerRangeSymbolicMax {
            minimum,
            max_field,
            max_offset,
        } => Some((*minimum, max_field, *max_offset)),
        _ => None,
    })
}

/// Route (a): the arm's guard conjunction contains a compare relating the
/// ARGUMENT (matched by display spelling -- the co-located guard names the
/// same expression the argument position does) to `self.<max_field> + k`,
/// tight enough for the declared offset: `arg < f + k` gives `arg <= f+k-1`
/// (needs `k-1 <= offset`); `arg <= f + k` needs `k <= offset`. Flipped
/// spellings (`f + k > arg`) normalize to the same two forms.
pub(crate) fn guard_proves_dependent_upper(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    max_field: &Identifier,
    max_offset: i64,
) -> bool {
    let TransitionGuardNode::When(guard) = obligation.guard else {
        return false;
    };
    let argument_label = expression_display_name(proof_plan, obligation.argument);
    guard_conjunct_proves_dependent_upper(proof_plan, guard, &argument_label, max_field, max_offset)
}

fn guard_conjunct_proves_dependent_upper(
    proof_plan: &ProofPlan,
    guard: ExpressionHandle,
    argument_label: &str,
    max_field: &Identifier,
    max_offset: i64,
) -> bool {
    let ExpressionNode::Binary(binary) = proof_plan.program.expression_table.expression(guard)
    else {
        return false;
    };
    let recurse = |handle: ExpressionHandle| {
        guard_conjunct_proves_dependent_upper(
            proof_plan,
            handle,
            argument_label,
            max_field,
            max_offset,
        )
    };
    // `arg REL bound` normalized to (argument side, bound side, inclusive?).
    let normalized = match binary.operator {
        BinaryOperator::And => return recurse(binary.left) || recurse(binary.right),
        // The multi-arm desugar nests the spelled compare inside
        // `(subject) == true` (same shape the D14 literal gate walks);
        // look through it.
        BinaryOperator::Equal
            if matches!(
                proof_plan.program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            return recurse(binary.left);
        }
        BinaryOperator::Less => Some((binary.left, binary.right, false)),
        BinaryOperator::LessOrEqual => Some((binary.left, binary.right, true)),
        BinaryOperator::Greater => Some((binary.right, binary.left, false)),
        BinaryOperator::GreaterOrEqual => Some((binary.right, binary.left, true)),
        _ => None,
    };
    let Some((argument_side, bound_side, inclusive)) = normalized else {
        return false;
    };
    if expression_display_name(proof_plan, argument_side) != argument_label {
        return false;
    }
    let Some(bound) = typed_trees::dependent_ranges::symbolic_max_bound(
        &proof_plan.program.expression_table,
        bound_side,
    ) else {
        return false;
    };
    if bound.field.as_str() != max_field.as_str() {
        return false;
    }
    let implied_offset = if inclusive {
        Some(bound.offset)
    } else {
        bound.offset.checked_sub(1)
    };
    implied_offset.is_some_and(|implied| implied <= max_offset)
}

/// Route (d) for CALL arguments: the call statement has no co-located guard,
/// but its state can still carry a dominating relational fact. A `requires`
/// contract on the state is an arrival contract -- every incoming transition
/// establishes it -- and the machine's `requires` likewise holds on the entry
/// state. Failing those, EVERY incoming edge must carry a `when` guard that
/// proves the bound, so the fact holds however the state was reached. The
/// bound field and the argument's place must survive the whole state: a write
/// between arrival and the call would invalidate the carried fact.
///
/// The route speaks only of `self.<field>` arguments -- the guard's bound
/// names `self.<max_field>`, and the preservation scan is a `self.` path
/// comparison; a local argument's writes would not be caught by it.
pub(crate) fn incoming_guard_proves_dependent_call_upper(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    max_field: &Identifier,
    max_offset: i64,
) -> bool {
    use typed_trees::signature::SignatureContractKind;
    use typed_trees::statement::StatementNode;
    let program = &proof_plan.program;
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == obligation.machine_symbol)
    else {
        return false;
    };
    let states = program.machine_states(machine);
    let Some((state_index, state)) = states
        .iter()
        .enumerate()
        .find(|(_, state)| state.symbol == obligation.state_symbol)
    else {
        return false;
    };
    let argument_label = expression_display_name(proof_plan, obligation.argument);
    let Some(argument_field) = receiver_place_field(&argument_label).filter(|field| {
        !field.is_empty()
            && field
                .chars()
                .all(|character| character.is_alphanumeric() || character == '_')
    }) else {
        return false;
    };
    // Only statements BEFORE the call can invalidate the arrival fact; a
    // later write (for example the terminal `exit_process` service call) is
    // outside the fact's window of relevance.
    let statements = program.statement_table.statements(state.statement_nodes);
    let call_index = statements
        .iter()
        .position(|statement| {
            matches!(statement, StatementNode::Call(call)
                if call.target_symbol == obligation.target_symbol
                    && program
                        .statement_table
                        .expression_handles(call.arguments)
                        .contains(&obligation.argument))
        })
        .unwrap_or(statements.len());
    if !place_preserved_in_statements(
        proof_plan,
        machine,
        &receiver_place_label(argument_field),
        &statements[..call_index],
    ) || !place_preserved_in_statements(
        proof_plan,
        machine,
        &receiver_place_label(max_field.as_str()),
        &statements[..call_index],
    ) {
        return false;
    }
    let proves = |condition: ExpressionHandle| {
        guard_conjunct_proves_dependent_upper(
            proof_plan,
            condition,
            &argument_label,
            max_field,
            max_offset,
        )
    };
    let requires_proves = |contracts: &[typed_trees::signature::SignatureContract]| {
        contracts
            .iter()
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts).iter())
            .any(|fact| {
                matches!(fact, typed_trees::domain::ProofFact::Expression(condition)
                    if proves(*condition))
            })
    };
    if requires_proves(program.state_contracts(state)) {
        return true;
    }
    let mut incoming_edges = 0usize;
    for source in states {
        for statement in program.statement_table.statements(source.statement_nodes) {
            let StatementNode::Transition(row) = statement else {
                continue;
            };
            // A value-dispatch edge could reach any state at runtime; the
            // route cannot prove it does not reach this one.
            if (row.target.is_valid()
                && matches!(
                    program.statement_table.transition_target(row.target),
                    typed_trees::statement::TransitionTargetNode::Value(_)
                ))
                || (row.continuation.is_valid()
                    && matches!(
                        program.statement_table.transition_target(row.continuation),
                        typed_trees::statement::TransitionTargetNode::Value(_)
                    ))
            {
                return false;
            }
            // The `target` arm arrives under the row's guard; the
            // `continuation` arm is the else path and carries no guard of its
            // own, so it can never witness this route.
            if target_enters_state(proof_plan, row.target, source, state) {
                let TransitionGuardNode::When(condition) = row.guard else {
                    return false;
                };
                if !proves(condition) {
                    return false;
                }
                incoming_edges += 1;
            }
            if row.continuation.is_valid()
                && target_enters_state(proof_plan, row.continuation, source, state)
            {
                return false;
            }
        }
    }
    if state_index == 0 {
        // The entry state's machine `requires` holds on the initial entry
        // edge only; a backedge would re-enter without re-discharging it, so
        // any incoming edge retires this leg.
        return incoming_edges == 0 && requires_proves(program.machine_contracts(machine));
    }
    incoming_edges > 0
}

/// Whether `target` names `state`: a `Named` target by its resolved symbol,
/// or a `SelfTarget` on an edge written inside the state itself.
fn target_enters_state(
    proof_plan: &ProofPlan,
    target: typed_trees::statement::TransitionTargetHandle,
    source: &typed_trees::state::State,
    state: &typed_trees::state::State,
) -> bool {
    if !target.is_valid() {
        return false;
    }
    match proof_plan.program.statement_table.transition_target(target) {
        typed_trees::statement::TransitionTargetNode::Named { path, .. } => {
            path.symbol == state.symbol
        }
        typed_trees::statement::TransitionTargetNode::SelfTarget => source.symbol == state.symbol,
        _ => false,
    }
}

/// Route (b): the named field's OWN enforced literal range minimum -- an
/// argument bounded by `min(field) + offset` satisfies `field + offset` for
/// EVERY runtime value the field's store-enforced range admits. `None` when
/// the field is unranged or carries a non-Exact domain (whose range is
/// deliberately permissive and must never discharge a bound).
pub(crate) fn dependent_field_floor(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    max_field: &Identifier,
) -> Option<i64> {
    let program = &proof_plan.program;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == obligation.machine.as_str())?;
    let attached = machine.attached_data.as_ref()?;
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == attached.as_str())?;
    let field_type =
        crate::obligations::data_field_type_by_name(program, data, max_field.as_str())?;
    enforced_literal_range_minimum(program, field_type)
}

/// The literal Range minimum of a type reference's Constrained shells, ONLY
/// under an Exact arithmetic domain (a non-Exact range is deliberately
/// permissive -- probed live on the store side -- and must never discharge a
/// bound). Mirrors the checker crate's `enforced_range_of_type_reference`.
fn enforced_literal_range_minimum(
    program: &typed_trees::TypedTrees,
    handle: typed_trees::types::TypeReferenceHandle,
) -> Option<i64> {
    use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            enforced_literal_range_minimum(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = program.type_reference_table.constraints(*constraints);
            if constraints.iter().any(|constraint| {
                matches!(
                    constraint,
                    TypeConstraintNode::ArithmeticDomain(domain)
                        if *domain != numerics::arithmetic::ArithmeticDomain::Exact
                )
            }) {
                return None;
            }
            constraints
                .iter()
                .find_map(|constraint| match constraint {
                    TypeConstraintNode::Range { minimum, .. } => {
                        validation::closed_integer_range_bound(program, *minimum)
                            .and_then(|value| value.to_i64())
                    }
                    _ => None,
                })
                .or_else(|| enforced_literal_range_minimum(program, *base_type))
        }
        _ => None,
    }
}

/// Route (b) for CALL arguments: same floor, machine resolved from the
/// obligation's (self-receiver) caller.
pub(crate) fn dependent_call_field_floor(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    max_field: &Identifier,
) -> Option<i64> {
    let program = &proof_plan.program;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == obligation.machine.as_str())?;
    let attached = machine.attached_data.as_ref()?;
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == attached.as_str())?;
    let field_type =
        crate::obligations::data_field_type_by_name(program, data, max_field.as_str())?;
    enforced_literal_range_minimum(program, field_type)
}

/// Route (c)'s soundness fence: the argument's own dependent atom speaks of
/// the field AT THIS STATE'S ENTRY, while the new obligation speaks of the
/// NEXT entry -- valid only if the field cannot have changed in between.
/// Conservative whole-state scan: assignments to the field and resolved
/// statement/value calls whose shared R5 frame overlaps it defeat the route.
/// Opaque calls remain fail-closed; the other discharge routes (guard / floor)
/// remain.
pub(crate) fn state_preserves_field(
    proof_plan: &ProofPlan,
    machine_name: &str,
    state_name: &str,
    field: &Identifier,
) -> bool {
    let program = &proof_plan.program;
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
    else {
        return false;
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == state_name)
    else {
        return false;
    };
    state_preserves_place_path(
        proof_plan,
        machine,
        state,
        &receiver_place_label(field.as_str()),
    )
}

/// The `state_preserves_field` scan over a concrete `self.<...>` place path:
/// assignments to the place and resolved statement/value calls whose shared
/// R5 frame overlaps it defeat the route. Opaque calls remain fail-closed.
fn state_preserves_place_path(
    proof_plan: &ProofPlan,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    field_path: &str,
) -> bool {
    place_preserved_in_statements(
        proof_plan,
        machine,
        field_path,
        proof_plan
            .program
            .statement_table
            .statements(state.statement_nodes),
    )
}

/// The preservation scan over an explicit statement slice -- the incoming-
/// guard route passes the prefix before the call so later writes cannot
/// invalidate a fact that only needs to describe the call's argument.
fn place_preserved_in_statements(
    proof_plan: &ProofPlan,
    machine: &typed_trees::machine::Machine,
    field_path: &str,
    statements: &[typed_trees::statement::StatementNode],
) -> bool {
    use typed_trees::statement::StatementNode;
    let program = &proof_plan.program;
    let call_frames = validation::CallFrameResolver::new(program);
    let field = &Identifier::generated(
        receiver_place_field(field_path)
            .unwrap_or(field_path)
            .to_owned(),
    );
    let field_path = field_path.to_owned();
    for statement in statements {
        let Some(value_written) = call_frames
            .as_ref()
            .and_then(|frames| frames.statement_value_may_write_paths(machine, statement))
        else {
            return false;
        };
        if value_written
            .iter()
            .any(|written| validation::frame_paths_overlap(&field_path, written))
        {
            return false;
        }
        match statement {
            StatementNode::Assignment(assignment) => {
                if expression_mentions_field(proof_plan, assignment.target, field) {
                    return false;
                }
            }
            StatementNode::Call(call) => {
                let Some(written) = call_frames
                    .as_ref()
                    .and_then(|frames| frames.may_write_paths(machine, call))
                else {
                    return false;
                };
                if written
                    .iter()
                    .any(|written| validation::frame_paths_overlap(&field_path, written))
                {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

fn expression_mentions_field(
    proof_plan: &ProofPlan,
    expression: ExpressionHandle,
    field: &Identifier,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match proof_plan.program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            member.member.as_str() == field.as_str()
                || expression_mentions_field(proof_plan, member.receiver, field)
        }
        ExpressionNode::Borrow(inner) => expression_mentions_field(proof_plan, inner.target, field),
        ExpressionNode::Indexed(indexed) => {
            expression_mentions_field(proof_plan, indexed.collection, field)
        }
        _ => false,
    }
}

pub(crate) fn sibling_len_from_constraints(constraints: &[ProofConstraint]) -> Option<(i64, i64)> {
    constraints.iter().find_map(|constraint| match constraint {
        ProofConstraint::IntegerRangeSiblingLenMax {
            minimum,
            max_offset,
            ..
        } => Some((*minimum, *max_offset)),
        _ => None,
    })
}

/// Guard route for the sibling-length upper half: a conjunct (through the
/// `== true` desugar) relating the ARGUMENT to `<sibling-arg>.len + k`,
/// tight enough for the declared offset (`arg < s.len` implies
/// `arg <= s.len - 1`, so strict needs `k-1 <= offset`). The receiver is
/// matched by display spelling against the (Mutable-stripped) sibling
/// argument.
pub(crate) fn guard_proves_sibling_len_upper(
    proof_plan: &ProofPlan,
    argument: ExpressionHandle,
    guard: &TransitionGuardNode,
    sibling_argument: ExpressionHandle,
    max_offset: i64,
) -> bool {
    let TransitionGuardNode::When(guard) = guard else {
        return false;
    };
    let argument_label = expression_display_name(proof_plan, argument);
    let sibling_label = expression_display_name(
        proof_plan,
        strip_mutable_handle(proof_plan, sibling_argument),
    );
    sibling_conjunct_proves(
        proof_plan,
        *guard,
        &argument_label,
        &sibling_label,
        max_offset,
    )
}

fn sibling_conjunct_proves(
    proof_plan: &ProofPlan,
    guard: ExpressionHandle,
    argument_label: &str,
    sibling_label: &str,
    max_offset: i64,
) -> bool {
    let ExpressionNode::Binary(binary) = proof_plan.program.expression_table.expression(guard)
    else {
        return false;
    };
    let recurse = |handle: ExpressionHandle| {
        sibling_conjunct_proves(
            proof_plan,
            handle,
            argument_label,
            sibling_label,
            max_offset,
        )
    };
    let normalized = match binary.operator {
        BinaryOperator::And => return recurse(binary.left) || recurse(binary.right),
        BinaryOperator::Equal
            if matches!(
                proof_plan.program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            return recurse(binary.left);
        }
        BinaryOperator::Less => Some((binary.left, binary.right, false)),
        BinaryOperator::LessOrEqual => Some((binary.left, binary.right, true)),
        BinaryOperator::Greater => Some((binary.right, binary.left, false)),
        BinaryOperator::GreaterOrEqual => Some((binary.right, binary.left, true)),
        _ => None,
    };
    let Some((argument_side, bound_side, inclusive)) = normalized else {
        return false;
    };
    if expression_display_name(proof_plan, argument_side) != argument_label {
        return false;
    }
    let Some(bound) = typed_trees::dependent_ranges::sibling_len_bound(
        &proof_plan.program.expression_table,
        bound_side,
    )
    .map(|bound| (bound.sibling, bound.offset))
    .or_else(|| {
        // The guard names the CALLER's expression (`self.buf.len`), not the
        // callee's param -- recognize `<expr>.len + k` with the RECEIVER
        // display-matched below instead of the bare-name rule.
        len_of_expression_bound(proof_plan, bound_side)
    }) else {
        return false;
    };
    let (receiver_label, k) = (bound.0, bound.1);
    if receiver_label.as_str() != sibling_label {
        return false;
    }
    let implied = if inclusive { Some(k) } else { k.checked_sub(1) };
    implied.is_some_and(|implied| implied <= max_offset)
}

/// `<receiver-expr>.len [+/- k]` with the receiver rendered by display name
/// (an Identifier carrying the display spelling for comparison).
fn len_of_expression_bound(
    proof_plan: &ProofPlan,
    bound: ExpressionHandle,
) -> Option<(Identifier, i64)> {
    let table = &proof_plan.program.expression_table;
    let (len_expr, offset) = match table.expression(bound) {
        ExpressionNode::Binary(binary) => {
            let ExpressionNode::Integer(literal) = table.expression(binary.right) else {
                return None;
            };
            let magnitude = literal.value_i64()?;
            let offset = match binary.operator {
                BinaryOperator::Add => magnitude,
                BinaryOperator::Subtract => magnitude.checked_neg()?,
                _ => return None,
            };
            (binary.left, offset)
        }
        _ => (bound, 0),
    };
    let ExpressionNode::Member(member) = table.expression(len_expr) else {
        return None;
    };
    if CollectionMeasure::from_authored_spelling(member.member.as_str())
        != Some(CollectionMeasure::Length)
    {
        return None;
    }
    let receiver_label = expression_display_name(proof_plan, member.receiver);
    Some((Identifier::generated(receiver_label), offset))
}

fn strip_mutable_handle(proof_plan: &ProofPlan, expression: ExpressionHandle) -> ExpressionHandle {
    match proof_plan.program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => strip_mutable_handle(proof_plan, inner.target),
        _ => expression,
    }
}
