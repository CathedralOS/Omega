//! Written linear targets and the claim identities and provenance of
//! permission expressions.

use crate::checks::multiplicity::linear_obligations::{LinearPlace, WrittenLinearTarget};
use crate::checks::multiplicity::type_multiplicity::{
    data_field_name, expression_establishes_obligation, literal_variant,
};
use language_semantics::{Multiplicity, PermissionClaimIdentity, PermissionProvenance};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;

/// Value expressions complete an owner whether spelled as a statement result
/// or a selected transition target. Keep the target's evaluation identity so
/// sibling arms cannot establish or transfer each other's custody.
pub(crate) fn completed_statement_values(
    program: &dyn ::validation::CheckingInput,
    statement: &StatementNode,
) -> Vec<(
    typed_trees::expression::ExpressionHandle,
    checked_trees::FlowPermissionEvaluation,
)> {
    match statement {
        StatementNode::Expression(expression) => vec![(
            *expression,
            checked_trees::FlowPermissionEvaluation::Statement,
        )],
        StatementNode::Transition(transition) => [transition.target, transition.continuation]
            .into_iter()
            .filter(|target| target.is_valid())
            .filter_map(
                |target| match program.statement_table.transition_target(target) {
                    typed_trees::statement::TransitionTargetNode::Value(expression) => Some((
                        *expression,
                        checked_trees::FlowPermissionEvaluation::TransitionTarget(target),
                    )),
                    _ => None,
                },
            )
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn written_linear_targets(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    evaluation: checked_trees::FlowPermissionEvaluation,
    places: &[LinearPlace],
) -> Vec<WrittenLinearTarget> {
    let (target, value) = match statement {
        StatementNode::LocalData(local) => {
            if !local.initial_value.is_valid() {
                // Zero-filled storage establishes a value only if the type's
                // validity admits zero. Gated storage keeps its construction
                // frontier without a whole-value permission occurrence.
                if validation::type_requires_establishment(program, local.type_reference) {
                    return Vec::new();
                }
                // An affine or linear local's zero image is a debt-free value
                // its first assignment replaces; the write below records no
                // obligation and, for a linear place, no establishment.
                return places
                    .iter()
                    .enumerate()
                    .filter(|(_, tracked)| {
                        tracked.root == facts::PlaceRoot::Symbol(local.symbol)
                            && tracked.multiplicity != Multiplicity::Unrestricted
                            && tracked.path.is_empty()
                    })
                    .map(|(place_index, _)| WrittenLinearTarget {
                        root: facts::PlaceRoot::Symbol(local.symbol),
                        destination_path: Vec::new(),
                        place_index,
                        obligation_live: false,
                        claim_identity: None,
                        provenance: None,
                    })
                    .collect();
            }
            (
                validation::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(local.symbol),
                    segments: Vec::new(),
                },
                local.initial_value,
            )
        }
        StatementNode::Assignment(assignment) => {
            let Some(place) = validation::canonical_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                assignment.target,
            ) else {
                return Vec::new();
            };
            (place, assignment.value)
        }
        _ => {
            let Some((expression, _)) = completed_statement_values(program, statement)
                .into_iter()
                .find(|(_, source)| *source == evaluation)
            else {
                return Vec::new();
            };
            let Some(place) = validation::canonical_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                expression,
            ) else {
                return Vec::new();
            };
            if place.root != facts::PlaceRoot::Expression(expression) {
                return Vec::new();
            }
            (place, expression)
        }
    };
    written_owner_targets(
        program,
        state_symbol,
        statement_index,
        &target,
        value,
        places,
    )
}

/// Establishment follows the completed value, not whether its destination has
/// a source name. Both local writes and anonymous owners retain the same input
/// identities, provenance and conditional claim frontier.
pub(crate) fn written_owner_targets(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    target: &validation::CanonicalPlace,
    value: typed_trees::expression::ExpressionHandle,
    places: &[LinearPlace],
) -> Vec<WrittenLinearTarget> {
    places
        .iter()
        .enumerate()
        .filter_map(|(place_index, tracked)| {
            if tracked.root != target.root || !tracked.path.starts_with(target.segments.as_slice())
            {
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

/// The descent-and-match core both permission provenance walks share: peel a
/// `Case`/`FixedIndex`/`Field` segment into the literal that produced it, then
/// resolve the remaining expression to its canonical place and take the live
/// places its path covers. `descend` is the caller's own recursive step,
/// `claim` the per-walk field, `common` the unanimous-match fold.
#[allow(clippy::too_many_arguments)]
fn expression_permission_claim_scan<X>(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    relative_path: &[facts::PlaceSegment],
    places: &[LinearPlace],
    descend: &dyn Fn(
        &dyn ::validation::CheckingInput,
        SymbolHandle,
        usize,
        typed_trees::expression::ExpressionHandle,
        &[facts::PlaceSegment],
        &[LinearPlace],
    ) -> Option<X>,
    claim: fn(&LinearPlace) -> Option<X>,
    common: &dyn Fn(Vec<X>) -> Option<X>,
) -> Option<X> {
    if let typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
        && let Some(facts::PlaceSegment::Case { variant }) = relative_path.first()
    {
        if literal_variant(program, literal).map(|candidate| candidate.symbol) != Some(*variant) {
            return None;
        }
        return descend(
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
        return descend(
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
        return descend(
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

    let source = validation::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    )?;
    let mut source_path = source.segments;
    source_path.extend_from_slice(relative_path);
    let matches = places
        .iter()
        .filter(|place| {
            place.root == source.root
                && place.live
                && (place.path == source_path
                    || (place.conditional && source_path.starts_with(place.path.as_slice())))
        })
        .filter_map(claim)
        .collect();
    common(matches)
}

fn expression_permission_claim_identity_for_claim(
    program: &dyn ::validation::CheckingInput,
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
                if program.state_by_symbol(call.target_symbol).is_some() {
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

    expression_permission_claim_scan(
        program,
        state_symbol,
        statement_index,
        expression,
        relative_path,
        places,
        &expression_permission_claim_identity_for_claim,
        |place| place.claim_identity,
        &|matches: Vec<_>| common_permission_claim_identity(matches.into_iter()),
    )
}

fn expression_permission_provenance_for_claim(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    relative_path: &[facts::PlaceSegment],
    places: &[LinearPlace],
) -> Option<PermissionProvenance> {
    if let typed_trees::expression::ExpressionNode::Call(call) =
        program.expression_table.expression(expression)
        && program.state_by_symbol(call.target_symbol).is_some()
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

    expression_permission_claim_scan(
        program,
        state_symbol,
        statement_index,
        expression,
        relative_path,
        places,
        &expression_permission_provenance_for_claim,
        |place| place.provenance,
        &|matches: Vec<_>| common_permission_provenance(matches.into_iter()),
    )
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
