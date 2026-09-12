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
            crate::operators::landed_integer_literal_type_reference(program, expression)?
        }
        ExpressionNode::Name(path) => super::named_value_type_reference(program, path)?,
        ExpressionNode::Call(call) => crate::calls::resolved_call_result_type(program, call)
            .or_else(|| {
                typed_trees::operator::resolve_named_expression_call(program, call)
                    .map(|operator| operator.return_type)
            })?,
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
    crate::places::unwrapped_type_reference(program, reference)
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
        .and_then(|reference| crate::places::unwrapped_type_reference(program, reference))
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
            crate::arithmetic_domains::validate_anonymous_integer_range(
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
/// Fresh/existing mixtures and borrowed, projected, or parameter owners remain
/// outside this whole-local contract.
fn plain_local_owner_selection(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    if crate::scalar_case_constructor(program, expression).is_some() {
        // Construction remains type-compatible. Multiplicity checking rejects
        // a reachable fresh/existing mixture until residual counts can join.
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
        ExpressionNode::Name(path) => {
            let Some(reference) = declared_value_type(program, machine, state, expression) else {
                return false;
            };
            program.type_multiplicity(reference) == language_semantics::Multiplicity::Affine
                && crate::scalar_case_value_source(program, expression, reference) == Some(path.symbol)
                && crate::has_plain_owned_contents_with_numeric_constraints(program, reference)
                && program.statement_table.statements(state.statement_nodes).iter().any(|statement| {
                    matches!(statement, typed_trees::statement::StatementNode::LocalData(local)
                        if local.symbol == path.symbol && !local.is_mutable && local.initial_value.is_valid())
                })
        }
        _ => false,
    }
}

fn result_needs_custody_join(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    value: ExpressionHandle,
) -> bool {
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
    // Thus an affine constructor needs no predecessor-owned input join, while
    // selecting an existing affine place or an owned call result still does.
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
            .any(|field| result_needs_custody_join(program, machine, state, field.value)),
        ExpressionNode::ArrayLiteral(elements) => program
            .expression_table
            .expression_handles(*elements)
            .iter()
            .any(|element| result_needs_custody_join(program, machine, state, *element)),
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
        return crate::places::unwrapped_type_reference(program, reference)
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
                let parameters = crate::calls::machine_state_by_symbol(program, call.target_symbol)
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
        crate::literals::expression_children::children(program, node, |child| pending.push(child));
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
        .map(|reference| crate::places::assignment_value_type(program, reference));
    let value_type = declared_value_type(program, machine, state, value)
        .map(|reference| crate::places::assignment_value_type(program, reference));
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
        crate::literals::validate_suffix_landing(program, anonymous, reference, diagnostics);
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
