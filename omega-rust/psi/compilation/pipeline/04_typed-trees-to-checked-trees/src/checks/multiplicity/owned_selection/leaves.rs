//! leaves slice of owned_selection.

use super::operands::*;
use super::{owned_source, source_multiplicity, stored_contents, unsupported};
use crate::checks::multiplicity::linear_obligations::type_reference_is_reference;
use arena::Handle;
use diagnostics::Diagnostic;
use language_semantics::Multiplicity;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern, TableMatchArm};
use typed_trees::types::TypeReferenceHandle;

pub(super) fn has_owned_leaf(
    program: &dyn ::validation::CheckingInput,
    places: &[super::super::linear_obligations::LinearPlace],
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            reachable_arms(program, dispatch.arms)
                .iter()
                .any(|(_, arm)| {
                    has_owned_leaf(program, places, machine, state, statement_index, arm.value)
                })
        }
        ExpressionNode::Name(_) => {
            validation::expression_result_type_reference(program, machine, state, expression)
                .is_some_and(|reference| {
                    owned_source(program, places, expression, reference).is_some()
                })
        }
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            projected_leaf_source(program, machine, state, statement_index, expression).is_some()
        }
        // A record arm moving existing children carries its leaves inside the
        // constructor: a projected field is the same moved-child leaf the
        // arm-level rule admits, so the roster has to engage for it too.
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| {
                has_owned_leaf(
                    program,
                    places,
                    machine,
                    state,
                    statement_index,
                    field.value,
                )
            }),
        _ => false,
    }
}

/// The exact canonical projection for one `member`/`indexed` leaf, when the
/// leaf selects a plain affine child of a live plain affine owner. A borrowed
/// prefix would escape the root's custody, so any reference at or below the
/// root rejects the projection instead of guessing referent ownership. The
/// root may be a whole local or a call's structural product; the product is
/// its own once-evaluated owner and carries no roster source.
pub(super) fn projected_leaf_source(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<(facts::PlaceRoot, Vec<facts::PlaceSegment>)> {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
    ) {
        return None;
    }
    let place = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        expression,
    )?;
    if !place.has_only_positional_segments() {
        return None;
    }
    match place.root {
        facts::PlaceRoot::Symbol(symbol) => {
            if symbol == machine.symbol
                || !matches!(
                    program.symbols.get(symbol).kind,
                    symbols::SymbolKind::Local | symbols::SymbolKind::Parameter
                )
            {
                return None;
            }
        }
        facts::PlaceRoot::Expression(root)
            if matches!(
                program.expression_table.expression(root),
                ExpressionNode::Call(_)
            ) => {}
        _ => return None,
    }
    // The leaf admits either an affine plain-owned child (the original rule)
    // or a linear child under an affine root: the moved claim then keeps its
    // exact path while the root's residual frontier stays live on every edge.
    // A linear root would put the claim at the whole place, which a projected
    // path cannot split, so the root itself must be affine.
    let leaf_reference =
        validation::canonical_place_type_reference(program, state.symbol, statement_index, &place)?;
    let leaf_multiplicity = source_multiplicity(program, leaf_reference);
    if !matches!(
        leaf_multiplicity,
        Multiplicity::Affine | Multiplicity::Linear
    ) {
        return None;
    }
    let linear_leaf = leaf_multiplicity == Multiplicity::Linear;
    // A temporary root (a call's structural product) can drop an affine
    // residual sibling, but a linear residual has no surviving owner place;
    // a projected linear child may only leave a named local or parameter.
    if linear_leaf && !matches!(place.root, facts::PlaceRoot::Symbol(_)) {
        return None;
    }
    let contents_ok = |type_reference| stored_contents(program, type_reference);
    let place_type_ok = |segments: &[facts::PlaceSegment], expected: Multiplicity| {
        validation::canonical_place_type_reference(
            program,
            state.symbol,
            statement_index,
            &validation::CanonicalPlace {
                root: place.root,
                segments: segments.to_vec(),
            },
        )
        .is_some_and(|type_reference| {
            validation::reference_result_custody::binding_multiplicity(program, type_reference)
                == expected
                && contents_ok(type_reference)
        })
    };
    if !place_type_ok(&place.segments, leaf_multiplicity)
        || !place_type_ok(&[], Multiplicity::Affine)
    {
        return None;
    }
    // Root and every proper prefix must be owned, never a reference: moving a
    // projected child through a borrow would imply referent custody.
    if (0..place.segments.len()).any(|length| {
        validation::canonical_place_type_reference(
            program,
            state.symbol,
            statement_index,
            &validation::CanonicalPlace {
                root: place.root,
                segments: place.segments[..length].to_vec(),
            },
        )
        .is_some_and(|reference| type_reference_is_reference(program, reference))
    }) {
        return None;
    }
    Some((place.root, place.segments))
}

pub(super) fn collect_leaves(
    program: &dyn ::validation::CheckingInput,
    places: &[super::super::linear_obligations::LinearPlace],
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    type_reference: TypeReferenceHandle,
    source_arm: Handle<TableMatchArm>,
    leaves: &mut Vec<(
        ExpressionHandle,
        Handle<TableMatchArm>,
        facts::PlaceRoot,
        Vec<facts::PlaceSegment>,
        TypeReferenceHandle,
    )>,
) -> Result<(), Diagnostic> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) if source_arm.is_valid() => {
            match owned_source(program, places, expression, type_reference) {
                Some(symbol) => {
                    leaves.push((
                        expression,
                        source_arm,
                        facts::PlaceRoot::Symbol(symbol),
                        Vec::new(),
                        type_reference,
                    ));
                    Ok(())
                }
                None => fresh_leaf(program, machine, state, expression, type_reference),
            }
        }
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) if source_arm.is_valid() => {
            match projected_leaf_source(program, machine, state, statement_index, expression) {
                Some((root, path)) => {
                    let leaf_type = validation::canonical_place_type_reference(
                        program,
                        state.symbol,
                        statement_index,
                        &validation::CanonicalPlace {
                            root,
                            segments: path.clone(),
                        },
                    )
                    .ok_or_else(unsupported)?;
                    if program.normalized_type_identity(leaf_type)
                        != program.normalized_type_identity(type_reference)
                    {
                        return Err(unsupported());
                    }
                    leaves.push((expression, source_arm, root, path, leaf_type));
                    Ok(())
                }
                _ => Err(unsupported()),
            }
        }
        ExpressionNode::StructLiteral(_) if source_arm.is_valid() => {
            // A record arm is fresh construction whose fields may each move
            // an exact owned child leaf. Moved fields join the roster so the
            // edge residual schedule names the precise complement; every
            // other field keeps the unrestricted-operand rule.
            // Capturing borrowed ingress forms a descriptor; it does not consume an
            // owned source slot. Reuse ordinary exact ingress/loan correspondence.
            if validation::reference_shell(program, type_reference).is_some()
                && validation::reference_result_custody::initializer_source(
                    program,
                    state,
                    expression,
                    type_reference,
                )
                .is_some()
            {
                return Ok(());
            }
            let fields = fresh_leaf_shape(program, machine, state, expression, type_reference)?;
            for field in fields {
                collect_field_leaves(
                    program,
                    places,
                    machine,
                    state,
                    statement_index,
                    field,
                    source_arm,
                    leaves,
                )?;
            }
            Ok(())
        }
        ExpressionNode::Call(call) if source_arm.is_valid() => {
            // A call's structural product is a fresh arm value: it carries no
            // roster leaf and no transfer, but its argument operands obey the
            // same hidden-ownership rules as fresh construction fields, and a
            // receiver or non-unrestricted parameter would move custody the
            // receipt cannot name.
            if call.receiver.is_valid() || !call_moves_no_ownership(program, call) {
                return Err(unsupported());
            }
            let operands = program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .flat_map(|argument| expression_nodes(program, *argument))
                .collect::<Vec<_>>();
            hidden_ownership_operands(
                program,
                machine,
                state,
                &operands,
                "owned match call arguments require unrestricted operands without hidden ownership transfers",
            )
        }
        ExpressionNode::Match(dispatch) => {
            // Selection predicates cannot introduce ownership effects hidden
            // outside the selected value graph. Scalar calls need their own
            // captured effect/loan sequence before joining this route, so a
            // predicate call stays rejected even when its arguments move no
            // ownership.
            let mut predicate_roots = vec![dispatch.subject];
            for (_, arm) in reachable_arms(program, dispatch.arms) {
                if let MatchPattern::Value(pattern) = arm.pattern {
                    predicate_roots.push(pattern);
                }
            }
            let operands = predicate_roots
                .iter()
                .flat_map(|root| expression_nodes(program, *root))
                .collect::<Vec<_>>();
            if operands.iter().any(|operand| {
                matches!(
                    program.expression_table.expression(*operand),
                    ExpressionNode::Call(_) | ExpressionNode::Borrow(_) | ExpressionNode::Atomic(_)
                )
            }) {
                return Err(unsupported());
            }
            let tag_operands = case_tag_operands(program, machine, state, &operands);
            if predicate_roots
                .iter()
                .flat_map(|root| predicate_value_nodes(program, machine, state, *root))
                .any(|operand| {
                    matches!(
                        program.expression_table.expression(operand),
                        ExpressionNode::Name(_)
                    ) && !tag_operands.contains(&operand)
                        && validation::expression_result_type_reference(
                            program, machine, state, operand,
                        )
                        .is_none_or(|reference| {
                            program.type_multiplicity(reference) != Multiplicity::Unrestricted
                        })
                })
            {
                return Err(Diagnostic::error(
                    "owned match predicates require unrestricted operands without hidden ownership transfers",
                ));
            }
            for (source_arm, arm) in reachable_arms(program, dispatch.arms) {
                collect_leaves(
                    program,
                    places,
                    machine,
                    state,
                    statement_index,
                    arm.value,
                    type_reference,
                    source_arm,
                    leaves,
                )?;
            }
            Ok(())
        }
        ExpressionNode::Borrow(_) if source_arm.is_valid() => {
            fresh_leaf(program, machine, state, expression, type_reference)
        }
        _ => Err(unsupported()),
    }
}

/// One record field contributes the same owned operand as an arm-level value.
/// Resolve its own type before selecting that source: the surrounding record's
/// type is not the moved field's carrier. Whole and projected sources retain
/// their exact current path; only genuinely fresh values use construction rules.
pub(super) fn collect_field_leaves(
    program: &dyn ::validation::CheckingInput,
    places: &[super::super::linear_obligations::LinearPlace],
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    field: ExpressionHandle,
    source_arm: Handle<TableMatchArm>,
    leaves: &mut Vec<(
        ExpressionHandle,
        Handle<TableMatchArm>,
        facts::PlaceRoot,
        Vec<facts::PlaceSegment>,
        TypeReferenceHandle,
    )>,
) -> Result<(), Diagnostic> {
    if let Some(reference) =
        validation::expression_result_type_reference(program, machine, state, field)
        && let Some(symbol) = owned_source(program, places, field, reference)
    {
        leaves.push((
            field,
            source_arm,
            facts::PlaceRoot::Symbol(symbol),
            Vec::new(),
            reference,
        ));
        return Ok(());
    }
    let fresh = |field| {
        hidden_ownership_operands(
            program,
            machine,
            state,
            &expression_nodes(program, field),
            "owned match record field operands require unrestricted operands without hidden ownership transfers",
        )
    };
    match program.expression_table.expression(field) {
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            match projected_leaf_source(program, machine, state, statement_index, field) {
                Some((root, path)) => {
                    let leaf_type = validation::canonical_place_type_reference(
                        program,
                        state.symbol,
                        statement_index,
                        &validation::CanonicalPlace {
                            root,
                            segments: path.clone(),
                        },
                    )
                    .ok_or_else(unsupported)?;
                    if Some(leaf_type)
                        != validation::expression_result_type_reference(
                            program, machine, state, field,
                        )
                    {
                        return Err(unsupported());
                    }
                    leaves.push((field, source_arm, root, path, leaf_type));
                    Ok(())
                }
                None => fresh(field),
            }
        }
        ExpressionNode::StructLiteral(literal) if literal.case_symbol.is_none() => {
            let reference =
                validation::expression_result_type_reference(program, machine, state, field)
                    .ok_or_else(unsupported)?;
            let fields = fresh_leaf_shape(program, machine, state, field, reference)?;
            for nested in fields {
                collect_field_leaves(
                    program,
                    places,
                    machine,
                    state,
                    statement_index,
                    nested,
                    source_arm,
                    leaves,
                )?;
            }
            Ok(())
        }
        _ => fresh(field),
    }
}

/// A fresh arm constructs the exact result type and contributes no source and
/// no transfer; its field operands obey the same no-hidden-ownership rules as
/// selection predicates.
pub(super) fn fresh_leaf(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    type_reference: TypeReferenceHandle,
) -> Result<(), Diagnostic> {
    // Capturing borrowed ingress forms a descriptor; it does not consume an
    // owned source slot. Reuse ordinary exact ingress/loan correspondence.
    if validation::reference_shell(program, type_reference).is_some()
        && validation::reference_result_custody::initializer_source(
            program,
            state,
            expression,
            type_reference,
        )
        .is_some()
    {
        return Ok(());
    }
    let fields = fresh_leaf_shape(program, machine, state, expression, type_reference)?;
    let operands = fields
        .into_iter()
        .flat_map(|value| expression_nodes(program, value))
        .collect::<Vec<_>>();
    hidden_ownership_operands(
        program,
        machine,
        state,
        &operands,
        "owned match fresh fields require unrestricted operands without hidden ownership transfers",
    )
}

/// The fresh-construction shape every literal or case arm must keep: the
/// authored expression declares the exact result type, its contents satisfy
/// the multiplicity's owned-storage walls, and the field value list is handed
/// back for the caller's per-field disposition.
pub(super) fn fresh_leaf_shape(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    type_reference: TypeReferenceHandle,
) -> Result<Vec<ExpressionHandle>, Diagnostic> {
    let (reference, fields) =
        if let Some(constructor) = validation::scalar_case_constructor(program, expression) {
            (
                constructor.type_reference,
                constructor
                    .fields
                    .into_iter()
                    .map(|(_, value, _)| value)
                    .collect::<Vec<_>>(),
            )
        } else if let ExpressionNode::StructLiteral(literal) =
            program.expression_table.expression(expression)
            && literal.case_symbol.is_none()
        {
            let reference =
                validation::expression_result_type_reference(program, machine, state, expression)
                    .ok_or_else(unsupported)?;
            (
                reference,
                program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value)
                    .collect(),
            )
        } else {
            return Err(unsupported());
        };
    let contents_ok = stored_contents(program, reference);
    if !contents_ok
        || program.normalized_type_identity(reference)
            != program.normalized_type_identity(type_reference)
    {
        return Err(unsupported());
    }
    Ok(fields)
}
