//! Ordered value-pattern compatibility and coverage. Destination checks still
//! visit every result arm, including arms whose runtime execution is excluded.

use super::expression_result_type_reference as declared_value_type;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, MatchPattern, TableMatchExpression,
};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

#[cfg(test)]
mod tests;

/// Structural readers visit every source edge, not just executable arms.
pub(crate) fn match_children(
    program: &TypedTrees,
    dispatch: TableMatchExpression,
) -> impl Iterator<Item = ExpressionHandle> + '_ {
    std::iter::once(dispatch.subject).chain(
        program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .flat_map(|arm| {
                let pattern = match arm.pattern {
                    MatchPattern::Value(pattern) => Some(pattern),
                    MatchPattern::Wildcard => None,
                };
                pattern.into_iter().chain(std::iter::once(arm.value))
            }),
    )
}

/// Find a retained subject carrier or an already-typed value-pattern peer.
/// Anonymous patterns never choose a default width.
pub fn match_subject_primitive_type(
    program: &TypedTrees,
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

fn retained_primitive(program: &TypedTrees, expression: ExpressionHandle) -> Option<PrimitiveType> {
    let reference = match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(_) => return Some(PrimitiveType::Bool),
        ExpressionNode::Integer(_) => {
            crate::declarations::operators::landed_integer_literal_type_reference(
                program, expression,
            )?
        }
        ExpressionNode::Name(path) => super::named_value_type_reference(program, path)?,
        ExpressionNode::Call(call) => {
            crate::machine_calls::calls::resolved_call_result_type(program, call).or_else(|| {
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
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    dispatch: &TableMatchExpression,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let arms = program.expression_table.match_arms(dispatch.arms);
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
    let selected_local_owners = plain_local_owner_selection(program, machine, state, expression);
    for arm in arms {
        match arm.pattern {
            MatchPattern::Wildcard => wildcard = true,
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
                if let ExpressionNode::Boolean(value) = program.expression_table.expression(pattern)
                {
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
    if !wildcard && !(subject_boolean && boolean_values == [true, true]) {
        diagnostics.push(Diagnostic::error(
            "match does not cover its subject; add a wildcard arm or both Boolean value alternatives",
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
        if selected_expression_transfers_owned(program, machine, state, arm.value)
            || matches!(arm.pattern, MatchPattern::Value(pattern)
                if selected_expression_transfers_owned(program, machine, state, pattern))
        {
            diagnostics.push(Diagnostic::error(
                "match selected pattern or arm transfers owned input custody; branch-local transfer and cleanup joins are not supported yet",
            ).with_source_span(arm.source_span));
        }
        if !selected_local_owners && result_needs_custody_join(program, machine, state, arm.value) {
            diagnostics.push(Diagnostic::error(
                "match result requires a reference or non-plain-owned branch custody join, which is not supported yet",
            ).with_source_span(arm.source_span));
        }
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

/// Type admission only. Mandatory multiplicity checking subsequently establishes
/// selected transfer receipts, continuation availability, and residual custody.
/// Fresh plain construction can join whole locals. Borrowed, projected, and
/// parameter owners still require their own transfer and cleanup evidence.
fn plain_local_owner_selection(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    if crate::scalar_case_constructor(program, expression).is_some() {
        // Construction remains type-compatible; multiplicity checking owns
        // the displaced source and residual counts for fresh/existing mixtures.
        return true;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            let arms = program.expression_table.match_arms(dispatch.arms);
            !arms.is_empty()
                && arms
                    .iter()
                    .all(|arm| plain_local_owner_selection(program, machine, state, arm.value))
        }
        ExpressionNode::StructLiteral(literal) => {
            let Some(reference) = declared_value_type(program, machine, state, expression) else {
                return false;
            };
            if crate::has_plain_owned_contents(program, reference) {
                return !result_needs_custody_join(program, machine, state, expression);
            }
            // A fresh linear construction establishes its own claim at the
            // destination on that edge only; its fields must carry no custody
            // join of their own.
            program.type_multiplicity(reference) == language_semantics::Multiplicity::Linear
                && crate::has_linear_owned_contents(program, reference)
                && program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .all(|field| !result_needs_custody_join(program, machine, state, field.value))
        }
        ExpressionNode::Name(path) => {
            let Some(reference) = declared_value_type(program, machine, state, expression) else {
                return false;
            };
            // A whole affine or linear local or parameter joins the same way a
            // plain affine source does; the multiplicity pass, not this
            // predicate, enforces that the identical place moves on every
            // reachable arm. `affine_owned_value_source` keeps the numeric-owned
            // walls while tolerating `[linear]` members: the root still moves
            // whole, and the checker names each linear child's exact claim on
            // the transfer instead of pretending the root is claim-free.
            let admitted_source = match program.type_multiplicity(reference) {
                language_semantics::Multiplicity::Affine => {
                    crate::affine_owned_value_source(program, expression, reference)
                        == Some(path.symbol)
                }
                language_semantics::Multiplicity::Linear => {
                    crate::linear_owned_value_source(program, expression, reference)
                        == Some(path.symbol)
                }
                _ => false,
            };
            admitted_source
                && (program.statement_table.statements(state.statement_nodes).iter().any(|statement| {
                    matches!(statement, typed_trees::statement::StatementNode::LocalData(local)
                        if local.symbol == path.symbol && !local.is_mutable && local.initial_value.is_valid())
                }) || program.state_parameters(state).iter().any(|parameter| {
                    // An immutable owned parameter is a live whole source
                    // established at state entry rather than by a statement.
                    parameter.symbol == path.symbol
                        && !parameter.is_self
                        && !parameter.is_const
                        && !parameter.is_mutable
                }))
        }
        // An owned child selected by exact field/fixed-index path is the same
        // plain-local transfer at a projected boundary: the root carries the
        // ownership event while the untouched residual siblings die on the
        // selected edge. Multiplicity checking replays the exact root, path,
        // and type from the owned-selection transfer evidence. A linear child
        // follows the same route except its residual claims stay live; the
        // uniform-consumption rule keeps the join's frontier identical on
        // every edge.
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            projected_owned_leaf(program, machine, state, expression)
        }
        // A call's structural product is a fresh independently-owned arm
        // value: its declared return type carries the plain-affine custody the
        // selection join consumes at the destination. Any owned input the
        // call would move stays rejected by the branch-local transfer check
        // above and by owned-selection receipt construction. A linear product
        // is equally fresh at the result boundary.
        ExpressionNode::Call(_) => declared_value_type(program, machine, state, expression)
            .is_some_and(|reference| {
                (program.type_multiplicity(reference) == language_semantics::Multiplicity::Affine
                    && crate::has_plain_owned_contents_with_numeric_constraints(program, reference))
                    || (program.type_multiplicity(reference)
                        == language_semantics::Multiplicity::Linear
                        && crate::has_linear_owned_contents(program, reference))
            }),
        _ => false,
    }
}

/// An exact projection selecting a moved owned child of an immutable local or
/// parameter, wherever a selection leaf is admitted: an arm value or a record
/// field inside one. The projection guard lives here: a bare name satisfies
/// the root checks below but names the whole place, whose custody joins
/// through the arm-level and custody-join rules instead.
fn projected_owned_leaf(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
    ) {
        return false;
    }
    projected_plain_owned_source(program, machine, state, expression)
        || projected_linear_owned_source(program, machine, state, expression)
}

/// Walk an exact projection chain to its whole local root when the leaf is a
/// linear claim. The root must stay affine: a linear root keeps its claim at
/// the whole place, which a projected path cannot split. Residual linear
/// siblings stay live on every edge, so their contents still satisfy the
/// linear-tolerant carrier rule rather than plain-owned storage.
fn projected_linear_owned_source(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    let Some(reference) = declared_value_type(program, machine, state, expression) else {
        return false;
    };
    if program.type_multiplicity(reference) != language_semantics::Multiplicity::Linear
        || !crate::has_linear_owned_contents(program, reference)
    {
        return false;
    }
    let mut cursor = expression;
    loop {
        match program.expression_table.expression(cursor) {
            ExpressionNode::Member(member) => cursor = member.receiver,
            ExpressionNode::Indexed(indexed) => {
                if !matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Integer(_)
                ) {
                    return false;
                }
                cursor = indexed.collection;
            }
            ExpressionNode::Name(path) => {
                let Some(root_reference) = declared_value_type(program, machine, state, cursor)
                else {
                    return false;
                };
                return program.type_multiplicity(root_reference)
                    == language_semantics::Multiplicity::Affine
                    && crate::has_linear_owned_contents(program, root_reference)
                    && crate::value_custody::owned_value_source::whole_owned_value_source(
                        program,
                        cursor,
                        root_reference,
                    ) == Some(path.symbol)
                    && (program.statement_table.statements(state.statement_nodes).iter().any(
                        |statement| {
                            matches!(statement, typed_trees::statement::StatementNode::LocalData(local)
                                if local.symbol == path.symbol && !local.is_mutable && local.initial_value.is_valid())
                        },
                    ) || program.state_parameters(state).iter().any(|parameter| {
                        parameter.symbol == path.symbol
                            && !parameter.is_self
                            && !parameter.is_const
                            && !parameter.is_mutable
                    }));
            }
            // A call's structural product is a temporary: an affine residual
            // may die with it, but a linear residual sibling has no named
            // place to preserve its claim, so no projected linear child may
            // leave a temporary root.
            _ => return false,
        }
    }
}

/// Walk an exact projection chain to its whole local root. Only record fields
/// and literal fixed indexes keep exact path identity; a borrowed, dynamic, or
/// otherwise opaque receiver rejects admission instead of guessing custody.
fn projected_plain_owned_source(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    let Some(reference) = declared_value_type(program, machine, state, expression) else {
        return false;
    };
    if program.type_multiplicity(reference) != language_semantics::Multiplicity::Affine
        || !crate::has_plain_owned_contents_with_numeric_constraints(program, reference)
    {
        return false;
    }
    let mut cursor = expression;
    loop {
        match program.expression_table.expression(cursor) {
            ExpressionNode::Member(member) => cursor = member.receiver,
            ExpressionNode::Indexed(indexed) => {
                if !matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Integer(_)
                ) {
                    return false;
                }
                cursor = indexed.collection;
            }
            ExpressionNode::Name(path) => {
                let Some(root_reference) = declared_value_type(program, machine, state, cursor)
                else {
                    return false;
                };
                return program.type_multiplicity(root_reference)
                    == language_semantics::Multiplicity::Affine
                    && crate::plain_owned_value_source(program, cursor, root_reference)
                        == Some(path.symbol)
                    && crate::has_plain_owned_contents_with_numeric_constraints(
                        program,
                        root_reference,
                    )
                    && (program.statement_table.statements(state.statement_nodes).iter().any(
                        |statement| {
                            matches!(statement, typed_trees::statement::StatementNode::LocalData(local)
                                if local.symbol == path.symbol && !local.is_mutable && local.initial_value.is_valid())
                        },
                    ) || program.state_parameters(state).iter().any(|parameter| {
                        // An immutable owned parameter is a live root
                        // established at state entry: the moved child keeps
                        // its exact path on the transfer while the residual
                        // complement dies on the selected edge, exactly as a
                        // local root does.
                        parameter.symbol == path.symbol
                            && !parameter.is_self
                            && !parameter.is_const
                            && !parameter.is_mutable
                    }));
            }
            // A call's structural product is its own once-evaluated owned
            // root: the declared return type carries the affine custody whose
            // projected child moves on the selected edge while the untouched
            // complement dies there.
            ExpressionNode::Call(_) => {
                let Some(root_reference) = declared_value_type(program, machine, state, cursor)
                else {
                    return false;
                };
                return program.type_multiplicity(root_reference)
                    == language_semantics::Multiplicity::Affine
                    && crate::has_plain_owned_contents_with_numeric_constraints(
                        program,
                        root_reference,
                    );
            }
            _ => return false,
        }
    }
}

/// A shared borrow of an exact place joins borrowed custody at the selected
/// destination: the referent's owner is never transferred, so there is no
/// owned input custody to merge. The typed-to-checked stage replays the
/// authored target's canonical root and path, so admission here is limited to
/// an exact place path -- record fields and literal fixed indexes over a named
/// root -- whose declared referent is a record the structural pipeline can
/// carry.
///
/// That carrier rule is linear-tolerant. Owned transfer, claims, and referent
/// cleanup cannot cross the borrowed boundary, so a linear referent's
/// exactly-once claim stays with its own owner on every edge and never becomes
/// an obligation this join has to partition or discharge. Exclusive carriers
/// and case-bearing referents keep their separate rejections: the former has
/// affine custody of its own, and the latter is outside the record-shaped
/// frontier the structural pipeline carries.
///
/// A primitive referent is admitted on the same terms. `&u64` denotes the
/// original storage exactly as `&Payload` does -- "a small referent or register
/// ABI does not authorize passing a snapshot" -- so the join observes a place
/// whose width happens to be scalar, and there is still no owned custody to
/// merge. It has no data declaration for the record rule to resolve, which is a
/// fact about where its shape is declared, not about what this gate checks.
fn selected_shared_borrow_place(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    value: ExpressionHandle,
) -> bool {
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(value) else {
        return false;
    };
    if borrow.access != language_semantics::ReferenceAccess::Shared
        || !shared_borrow_target_is_exact_place(program, borrow.target)
    {
        return false;
    }
    // `expression_result_type_reference` deliberately returns no result type
    // for a Borrow node, so the referent is read off the exact target place:
    // `&a.first` borrows the declared `first` field's own type. That referent
    // must be a named record whose contents the structural pipeline carries.
    // A `[linear]` declaration is such a carrier even though it is not plain
    // owned storage: the borrow observes the place and moves nothing, so this
    // join neither adds a claim to an edge nor discharges one. Loans, nominal
    // cleanup, and recursive storage stay excluded by the same carrier rule.
    let Some(referent) = declared_value_type(program, machine, state, borrow.target) else {
        return false;
    };
    if !crate::has_linear_owned_contents(program, referent) {
        return false;
    }
    // A primitive referent carries no declaration to resolve. The structural
    // pipeline builds its carrier from the primitive itself, so requiring a
    // data definition here would reject the place for the wrong reason.
    if program.primitive_type_reference(referent).is_some() {
        return true;
    }
    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(referent)
    else {
        return false;
    };
    let Some(record) = program
        .data_definitions()
        .iter()
        .find(|record| record.symbol == *symbol)
    else {
        return false;
    };
    !program
        .data_members(record)
        .iter()
        .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
}

/// Whether a shared-borrow arm target is an exact place path the checked arm
/// planner can canonicalize into a `SharedBorrow` source: record fields and
/// literal fixed indexes over a named root. A dynamic index or range segment
/// has no statically checkable ordinal, and a computed root has no place to
/// re-derive at replay, so neither joins borrowed custody here.
fn shared_borrow_target_is_exact_place(program: &TypedTrees, mut target: ExpressionHandle) -> bool {
    loop {
        match program.expression_table.expression(target) {
            ExpressionNode::Name(_) => return true,
            ExpressionNode::Member(member) => target = member.receiver,
            ExpressionNode::Indexed(indexed) => {
                if program
                    .expression_table
                    .constant_integer_value(indexed.index)
                    .and_then(|index| usize::try_from(index).ok())
                    .is_none()
                {
                    return false;
                }
                target = indexed.collection;
            }
            ExpressionNode::Borrow(inner) => target = inner.target,
            _ => return false,
        }
    }
}

fn result_needs_custody_join(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    value: ExpressionHandle,
) -> bool {
    if selected_shared_borrow_place(program, machine, state, value) {
        return false;
    }
    let reference = declared_value_type(program, machine, state, value);
    if reference.is_some_and(|reference| {
        !crate::has_plain_owned_contents_with_numeric_constraints(program, reference)
            && !has_unrouted_scalar_contents(program, reference)
    }) {
        return true;
    }
    // A selected constructor establishes its result once at the destination.
    // Plain owned contents exclude loans, linear debt and nominal cleanup; each
    // child still has to supply fresh construction or an unrestricted value.
    // Thus an affine constructor needs no predecessor-owned input join, and a
    // call's structural product is equally fresh at the result boundary, while
    // selecting an existing affine place still joins predecessor custody.
    match program.expression_table.expression(value) {
        ExpressionNode::Borrow(_) => true,
        ExpressionNode::Match(dispatch) => program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .any(|arm| result_needs_custody_join(program, machine, state, arm.value)),
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| {
                // A projected owned field is a selection leaf: its custody
                // joins through the transfer roster's exact moved path, not
                // through a branch custody join of the record itself.
                !projected_owned_leaf(program, machine, state, field.value)
                    && result_needs_custody_join(program, machine, state, field.value)
            }),
        ExpressionNode::ArrayLiteral(elements) => program
            .expression_table
            .expression_handles(*elements)
            .iter()
            .any(|element| result_needs_custody_join(program, machine, state, *element)),
        // A call product is a fresh owned result, never a join of
        // predecessor custody: any owned input it moves is the call's own
        // transfer obligation, which the branch-local transfer check still
        // rejects independently.
        ExpressionNode::Call(_) => false,
        _ => reference.is_some_and(|reference| {
            program.type_multiplicity(reference) != language_semantics::Multiplicity::Unrestricted
                && fresh_payloadless_case(program, value, reference).is_none()
        }),
    }
}

/// A fresh claim-free payloadless constructor has no input ownership to merge.
/// Resolve its actual declaration and case; an owned local with the same type
/// is a transfer and must not inherit this construction permission.
pub fn fresh_payloadless_case(
    program: &TypedTrees,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<(symbols::SymbolHandle, symbols::SymbolHandle)> {
    use typed_trees::data::DataMember;
    use typed_trees::types::TypeReferenceNode;
    if !crate::has_plain_owned_contents_with_numeric_constraints(program, reference) {
        return None;
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *symbol)?;
    let members = program.data_members(data);
    if members.is_empty()
        || members.iter().any(|member| match member {
            DataMember::Variant(variant) => !program.data_payload_fields(variant).is_empty(),
            DataMember::Field(_) => true,
        })
    {
        return None;
    }
    let selected = match program.expression_table.expression(expression) {
        ExpressionNode::Name(path)
            if path.head_symbol == *symbol
                && program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    == 2 =>
        {
            path.symbol
        }
        ExpressionNode::StructLiteral(literal)
            if literal.type_symbol == *symbol
                && program
                    .expression_table
                    .struct_fields(literal.fields)
                    .is_empty() =>
        {
            literal.case_symbol?
        }
        _ => return None,
    };
    members
        .iter()
        .any(|member| matches!(member, DataMember::Variant(variant) if variant.symbol == selected))
        .then_some((*symbol, selected))
}

pub fn is_fresh_payloadless_structural_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
) -> bool {
    if fresh_payloadless_case(program, expression, reference).is_some() {
        return true;
    }
    let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression) else {
        return false;
    };
    let arms = program.expression_table.match_arms(dispatch.arms);
    !arms.is_empty()
        && arms
            .iter()
            .all(|arm| is_fresh_payloadless_structural_value(program, arm.value, reference))
}

/// Static scalar theories do not add storage to a result join. Their exact
/// meaning is retained by compatibility above, not erased to admit the value.
/// Routed provenance and references still need their separate branch custody;
/// do not widen the plain-storage classifier used by representation-erasing
/// construction and return paths elsewhere in the compiler.
fn has_unrouted_scalar_contents(program: &TypedTrees, mut reference: TypeReferenceHandle) -> bool {
    use typed_trees::types::{DomainConstraintSubject, TypeConstraintNode, TypeReferenceNode};
    let mut visited = Vec::new();
    while program
        .type_reference_table
        .contains_type_reference(reference)
        && !visited.contains(&reference)
    {
        visited.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let Some(constraints) = program.type_reference_table.constraint_span(*constraints)
                else {
                    return false;
                };
                if !constraints.iter().all(|constraint| match constraint {
                    TypeConstraintNode::Range { .. } | TypeConstraintNode::ArithmeticDomain(_) => {
                        true
                    }
                    TypeConstraintNode::Domain(domain) => {
                        domain.subject == DomainConstraintSubject::Declared
                            && domain.symbol.is_valid()
                            && domain.establishment_routes.is_empty()
                    }
                    TypeConstraintNode::Named(_) => false,
                }) {
                    return false;
                }
                reference = *base_type;
            }
            TypeReferenceNode::Named { .. } => {
                return program.primitive_type_reference(reference).is_some();
            }
            _ => return false,
        }
    }
    false
}

fn is_scalar_value(
    program: &TypedTrees,
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

fn selected_expression_transfers_owned(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    root: ExpressionHandle,
) -> bool {
    let requires_transfer = |reference: TypeReferenceHandle| {
        reference.is_valid()
            && program.type_multiplicity(reference)
                != language_semantics::Multiplicity::Unrestricted
    };
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !expression.is_valid() || visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        let node = program.expression_table.expression(expression);
        match node {
            ExpressionNode::Call(call) => {
                let parameters = crate::machine_calls::calls::machine_state_by_symbol(
                    program,
                    call.target_symbol,
                )
                .map(|(_, state)| program.state_parameters(state))
                .or_else(|| {
                    typed_trees::operator::resolve_named_expression_call(program, call)
                        .map(|operator| program.operator_parameters(operator))
                });
                if parameters.is_some_and(|parameters| {
                    parameters
                        .iter()
                        .any(|parameter| requires_transfer(parameter.type_reference))
                }) {
                    return true;
                }
            }
            ExpressionNode::Binary(binary) => {
                if [binary.left, binary.right].iter().any(|operand| {
                    declared_value_type(program, machine, state, *operand)
                        .is_some_and(requires_transfer)
                }) {
                    return true;
                }
            }
            ExpressionNode::Unary(unary)
                if declared_value_type(program, machine, state, unary.operand)
                    .is_some_and(requires_transfer) =>
            {
                return true;
            }
            _ => {}
        }
        crate::value_custody::literals::expression_children::children(program, node, |child| {
            pending.push(child)
        });
    }
    false
}

fn check_compatible_values(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    peer: ExpressionHandle,
    value: ExpressionHandle,
    message: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let ExpressionNode::Match(dispatch) = program.expression_table.expression(value) {
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
    if let ExpressionNode::Match(dispatch) = program.expression_table.expression(peer) {
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
