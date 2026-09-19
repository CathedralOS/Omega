//! Written linear targets and the claim identities and provenance of
//! permission expressions.

use crate::checks::multiplicity::linear_obligations::{LinearPlace, WrittenLinearTarget};
use crate::checks::multiplicity::type_multiplicity::{
    data_field_name, expression_establishes_obligation, literal_variant,
};
use language_semantics::{Multiplicity, PermissionClaimIdentity, PermissionProvenance};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;

pub(crate) fn written_linear_targets(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    places: &[LinearPlace],
) -> Vec<WrittenLinearTarget> {
    let (target, value) = match statement {
        StatementNode::LocalData(local) => {
            if !local.initial_value.is_valid() {
                return Vec::new();
            }
            (
                crate::flow::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(local.symbol),
                    segments: Vec::new(),
                },
                local.initial_value,
            )
        }
        StatementNode::Assignment(assignment) => {
            let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                assignment.target,
            ) else {
                return Vec::new();
            };
            (place, assignment.value)
        }
        _ => return Vec::new(),
    };
    let facts::PlaceRoot::Symbol(symbol) = target.root else {
        return Vec::new();
    };

    places
        .iter()
        .enumerate()
        .filter_map(|(place_index, tracked)| {
            if tracked.symbol != symbol || !tracked.path.starts_with(target.segments.as_slice()) {
                return None;
            }
            let relative_path = &tracked.path[target.segments.len()..];
            Some(WrittenLinearTarget {
                root: target.root,
                destination_path: target.segments.clone(),
                place_index,
                obligation_live: tracked.multiplicity != Multiplicity::Affine
                    && expression_establishes_obligation(
                        program,
                        state_symbol,
                        statement_index,
                        value,
                        relative_path,
                        places,
                    ),
                claim_identity: expression_permission_claim_identity_for_claim(
                    program,
                    state_symbol,
                    statement_index,
                    value,
                    relative_path,
                    places,
                ),
                provenance: expression_permission_provenance_for_claim(
                    program,
                    state_symbol,
                    statement_index,
                    value,
                    relative_path,
                    places,
                ),
            })
        })
        .collect()
}

fn expression_permission_claim_identity_for_claim(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    relative_path: &[facts::PlaceSegment],
    places: &[LinearPlace],
) -> Option<PermissionClaimIdentity> {
    if relative_path.is_empty() {
        match program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Call(call) => {
                // A checked result is a distinct occurrence until its exact
                // return map proves forwarding. One owned argument does not
                // prove that a conditional callee returns that argument.
                if crate::semantic_calls::find_state(program, call.target_symbol).is_some() {
                    return None;
                }
                let mut candidates = Vec::new();
                if call.receiver.is_valid() {
                    candidates.push(call.receiver);
                }
                candidates
                    .extend_from_slice(program.expression_table.expression_handles(call.arguments));
                return common_permission_claim_identity(candidates.into_iter().filter_map(
                    |candidate| {
                        expression_permission_claim_identity_for_claim(
                            program,
                            state_symbol,
                            statement_index,
                            candidate,
                            &[],
                            places,
                        )
                    },
                ));
            }
            typed_trees::expression::ExpressionNode::StructLiteral(literal) => {
                return common_permission_claim_identity(
                    program
                        .expression_table
                        .struct_fields(literal.fields)
                        .iter()
                        .filter_map(|field| {
                            expression_permission_claim_identity_for_claim(
                                program,
                                state_symbol,
                                statement_index,
                                field.value,
                                &[],
                                places,
                            )
                        }),
                );
            }
            typed_trees::expression::ExpressionNode::ArrayLiteral(values) => {
                return common_permission_claim_identity(
                    program
                        .expression_table
                        .expression_handles(*values)
                        .iter()
                        .filter_map(|value| {
                            expression_permission_claim_identity_for_claim(
                                program,
                                state_symbol,
                                statement_index,
                                *value,
                                &[],
                                places,
                            )
                        }),
                );
            }
            _ => {}
        }
    }

    if let typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
        && let Some(facts::PlaceSegment::Case { variant }) = relative_path.first()
    {
        if literal_variant(program, literal).map(|candidate| candidate.symbol) != Some(*variant) {
            return None;
        }
        return expression_permission_claim_identity_for_claim(
            program,
            state_symbol,
            statement_index,
            expression,
            &relative_path[1..],
            places,
        );
    }

    if let typed_trees::expression::ExpressionNode::ArrayLiteral(values) =
        program.expression_table.expression(expression)
        && let Some(facts::PlaceSegment::FixedIndex { index }) = relative_path.first()
    {
        let value = *program
            .expression_table
            .expression_handles(*values)
            .get(*index)?;
        return expression_permission_claim_identity_for_claim(
            program,
            state_symbol,
            statement_index,
            value,
            &relative_path[1..],
            places,
        );
    }

    if let typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
        && let Some(facts::PlaceSegment::Field { symbol }) = relative_path.first()
    {
        let field_name = data_field_name(program, *symbol)?;
        let field = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .find(|field| field.name.as_str() == field_name)?;
        return expression_permission_claim_identity_for_claim(
            program,
            state_symbol,
            statement_index,
            field.value,
            &relative_path[1..],
            places,
        );
    }

    if !relative_path.is_empty()
        && matches!(
            program.expression_table.expression(expression),
            typed_trees::expression::ExpressionNode::Call(_)
        )
    {
        return None;
    }

    let source = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    )?;
    let facts::PlaceRoot::Symbol(symbol) = source.root else {
        return None;
    };
    let mut source_path = source.segments;
    source_path.extend_from_slice(relative_path);
    let matches = places
        .iter()
        .filter(|place| {
            place.symbol == symbol
                && place.live
                && (place.path == source_path
                    || (place.conditional && source_path.starts_with(place.path.as_slice())))
        })
        .filter_map(|place| place.claim_identity);
    common_permission_claim_identity(matches)
}

fn expression_permission_provenance_for_claim(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    relative_path: &[facts::PlaceSegment],
    places: &[LinearPlace],
) -> Option<PermissionProvenance> {
    if let typed_trees::expression::ExpressionNode::Call(call) =
        program.expression_table.expression(expression)
        && crate::semantic_calls::find_state(program, call.target_symbol).is_some()
    {
        return None;
    }
    if relative_path.is_empty()
        && matches!(
            program.expression_table.expression(expression),
            typed_trees::expression::ExpressionNode::Call(_)
                | typed_trees::expression::ExpressionNode::StructLiteral(_)
                | typed_trees::expression::ExpressionNode::ArrayLiteral(_)
        )
    {
        return validation::expression_permission_provenance(
            program,
            expression,
            &mut |candidate| {
                Ok(expression_permission_provenance_for_claim(
                    program,
                    state_symbol,
                    statement_index,
                    candidate,
                    &[],
                    places,
                ))
            },
        )
        .ok()
        .flatten();
    }

    if let typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
        && let Some(facts::PlaceSegment::Case { variant }) = relative_path.first()
    {
        if literal_variant(program, literal).map(|candidate| candidate.symbol) != Some(*variant) {
            return None;
        }
        return expression_permission_provenance_for_claim(
            program,
            state_symbol,
            statement_index,
            expression,
            &relative_path[1..],
            places,
        );
    }

    if let typed_trees::expression::ExpressionNode::ArrayLiteral(values) =
        program.expression_table.expression(expression)
        && let Some(facts::PlaceSegment::FixedIndex { index }) = relative_path.first()
    {
        let value = *program
            .expression_table
            .expression_handles(*values)
            .get(*index)?;
        return expression_permission_provenance_for_claim(
            program,
            state_symbol,
            statement_index,
            value,
            &relative_path[1..],
            places,
        );
    }

    if let typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
        && let Some(facts::PlaceSegment::Field { symbol }) = relative_path.first()
    {
        let field_name = data_field_name(program, *symbol)?;
        let field = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .find(|field| field.name.as_str() == field_name)?;
        return expression_permission_provenance_for_claim(
            program,
            state_symbol,
            statement_index,
            field.value,
            &relative_path[1..],
            places,
        );
    }

    if !relative_path.is_empty()
        && matches!(
            program.expression_table.expression(expression),
            typed_trees::expression::ExpressionNode::Call(_)
        )
    {
        // Multi-output call mappings need the explicit P1c outcome map. Do
        // not guess a field origin from argument order.
        return None;
    }

    let source = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    )?;
    let facts::PlaceRoot::Symbol(symbol) = source.root else {
        return None;
    };
    let mut source_path = source.segments;
    source_path.extend_from_slice(relative_path);
    let matches = places
        .iter()
        .filter(|place| {
            place.symbol == symbol
                && place.live
                && (place.path == source_path
                    || (place.conditional && source_path.starts_with(place.path.as_slice())))
        })
        .filter_map(|place| place.provenance);
    common_permission_provenance(matches)
}

fn common_permission_provenance(
    mut origins: impl Iterator<Item = PermissionProvenance>,
) -> Option<PermissionProvenance> {
    let first = origins.next()?;
    origins.all(|origin| origin == first).then_some(first)
}

fn common_permission_claim_identity(
    mut identities: impl Iterator<Item = PermissionClaimIdentity>,
) -> Option<PermissionClaimIdentity> {
    let first = identities.next()?;
    identities
        .all(|identity| identity == first)
        .then_some(first)
}
