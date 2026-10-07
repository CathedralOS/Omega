//! Guard retention and parameter forwarding checks a state plans before its
//! statement sequence.
//!
//! `retained_guard` + `guard_is_boolean` read the authored guard coordinate;
//! `retire_guarded_reference_completion` and `forwards_qualified_parameter`
//! admit a guarded return arm's result binding and forwarded parameters. The
//! terminator module shares these with `build_traced` through `super::`.
use super::super::{
    CheckFacts, CheckedByteSequenceCarrier, CheckedComposedUnitControlStatePlan,
    CheckedScalarExpressionRole, CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan, Multiplicity, PrimitiveType,
};
use crate::execution::terminal_unit::types::ShapeCollector;

/// A sequence is shared by ordinary and composed planning, so it retains the
/// syntactic tail's reference completion. Guarded composed control owns every
/// return arm instead. Remove only the exact sequence result binding when its
/// source coordinate and ingress also occur in the guarded return roster;
/// ordinary and unconditional completions keep their producer unchanged.
pub(in crate::execution::terminal_unit) fn retire_guarded_reference_completion(
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    returned: Option<&checked_trees::CheckedUnitStructuralReturnPlan>,
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
) -> Option<()> {
    let Some(checked_trees::CheckedUnitStructuralReturnPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal },
        reference_sources,
        ..
    }) = returned
    else {
        return Some(());
    };
    let mut tails = facts
        .flow
        .terminal_scalar_graphs
        .guarded_tails
        .iter()
        .filter(|tail| tail.state == state.symbol);
    let Some(tail) = tails.next() else {
        return Some(());
    };
    if tails.next().is_some() {
        return None;
    }
    let arms = facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(tail.arms)?;
    // Each value-return arm owns its own completion. A sibling invocation
    // or state edge does not produce this sequence's reference binding and
    // therefore neither contributes a value coordinate nor prevents retiring
    // the duplicate producer for the exact arm below.
    let mut returned_statements = arms
        .iter()
        .map(|arm| &arm.destination)
        .chain(tail.fallback.iter())
        .filter_map(|destination| match destination {
            checked_trees::CheckedScalarBranchDestination::Return {
                statement_ordinal, ..
            } => Some(*statement_ordinal),
            _ => None,
        })
        .collect::<Vec<_>>();
    returned_statements.sort_unstable();
    returned_statements.dedup();
    let mut matches = operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            let CheckedUnitEffectOperationPlan::EstablishReference { result, source } = operation
            else {
                return None;
            };
            (result.binding_ordinal == *binding_ordinal
                && returned_statements.contains(&result.statement_index)
                && reference_sources
                    .iter()
                    .any(|candidate| candidate.path.is_empty() && candidate.source == *source))
            .then_some(index)
        });
    let Some(index) = matches.next() else {
        return Some(());
    };
    if matches.next().is_some() {
        return None;
    }
    operations.remove(index);
    Some(())
}

/// A returning exit disposes whole roots only: a partial affine drop or a
/// residual discard at the return has no Terminal return-cleanup row here.
#[allow(clippy::too_many_arguments)]
/// A shared loan freezes its referent. A mutable bounded byte owner instead
/// preserves its primitive byte predicates through independently checked stores.
/// Neither rule grants transport for arbitrary mutable qualified values.
/// An immutable owned byte parameter likewise forwards its established value
/// whole, with exactly the same primitive predicate roster on every arrival.
/// A successor's qualified parameter is admitted only when every live
/// edge into its state forwards a source parameter with exactly the same
/// carrier, access, multiplicity and qualifications, or, for a shared byte
/// view, establishes an authored literal under those qualifications: each
/// is then a byte-predicate domain without establishment routes, which
/// Terminal verification decides from the literal's octets at the edge.
///
/// A re-entered entry answers to this same rule, and the walk below is
/// complete for it. An entry has two arrival kinds: its invocation, whose
/// qualifications the caller establishes at the call, and its reentry edges.
/// The caller's rule above exempts the invocation-only entry outright, so a
/// state reaching here with index 0 is re-entered and the invocation
/// arrival is already licensed; what remains unproven is exactly the edge
/// transport this walk requires, because the walk visits every live edge
/// whose target is that state, backedges into the entry included. Refusing
/// index 0 outright instead re-decided the caller's rule by hand and
/// refused a machine that recurses in its entry -- the only recursion shape
/// `Nat::BoundedDistance` admits -- however exactly its edges forward the
/// qualified parameter.
pub(in crate::execution::terminal_unit) fn forwards_qualified_parameter(
    shapes: &ShapeCollector<'_>,
    planned: &[CheckedComposedUnitControlStatePlan],
    live: &[bool],
    target_index: usize,
    parameter_index: usize,
) -> bool {
    let Some(target) = planned.get(target_index) else {
        return false;
    };
    let Some(parameter) = target.structural_parameters.get(parameter_index) else {
        return false;
    };
    if !(parameter.access == CheckedStructuralAccess::SharedBorrow
        || (matches!(
            parameter.access,
            CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::Owned
        ) && parameter.projected_qualifications.is_empty()
            && shapes
                .types
                .get(&parameter.type_identity)
                .is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(
                            CheckedByteSequenceCarrier::BoundedOwned { .. }
                        )
                    )
                })
            && parameter.qualifications.iter().all(|qualification| {
                shapes.domains.iter().any(|domain| {
                    domain.domain == *qualification
                        && domain.carrier_type_identity == parameter.type_identity
                        && domain.byte_predicate.is_some()
                        && domain.establishment_routes.is_empty()
                })
            })))
        || parameter.multiplicity != Multiplicity::Unrestricted
    {
        return false;
    }
    let literal_establishes_qualifications = parameter.access
        == CheckedStructuralAccess::SharedBorrow
        && parameter.projected_qualifications.is_empty()
        && shapes
            .types
            .get(&parameter.type_identity)
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(
                        CheckedByteSequenceCarrier::BorrowedView { .. }
                    )
                )
            })
        && parameter.qualifications.iter().all(|qualification| {
            shapes.domains.iter().any(|domain| {
                domain.domain == *qualification
                    && domain.carrier_type_identity == parameter.type_identity
                    && domain.byte_predicate.is_some()
                    && domain.establishment_routes.is_empty()
            })
        });
    planned
        .iter()
        .zip(live)
        .filter(|(_, live)| **live)
        .all(|(source, _)| {
            source
                .successors()
                .into_iter()
                .filter(|successor| successor.target_state == target.state)
                .all(|successor| {
                    successor.transfers.iter().any(|transfer| {
                        transfer.target_parameter_index as usize == parameter_index
                            && match &transfer.source {
                                checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceLiteral { .. } => {
                                    literal_establishes_qualifications
                                }
                                checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index } => {
                                    source.structural_parameters.get(*index as usize).is_some_and(|forwarded| {
                                        forwarded.type_identity == parameter.type_identity
                                            && forwarded.access == parameter.access
                                            && forwarded.multiplicity == parameter.multiplicity
                                            && forwarded.qualifications == parameter.qualifications
                                            && forwarded.projected_qualifications
                                                == parameter.projected_qualifications
                                    })
                                }
                                // A projected loan or copy forwards custody of
                                // storage declared under the same base identity
                                // and access: the subject's admission already
                                // proved the declared type match, and byte-
                                // predicate qualifications re-verify on the
                                // storage at the edge either way.
                                checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan { subject, .. }
                                | checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedValue { subject } => {
                                    subject.type_identity == parameter.type_identity
                                        && subject.access == parameter.access
                                }
                                _ => false,
                            }
                    })
                })
        })
}

/// The exact Boolean value one authored guard retains at its `Guard`
/// coordinate: the pure expression, or the unique computation root this
/// machine owns there (a selected comparison, a call, or any other checked
/// computation). The computation producer records a root only where no pure
/// expression exists, so the coordinate names at most one of the two forms.
pub(in crate::execution::terminal_unit) fn retained_guard(
    facts: &CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_ordinal: u32,
) -> Option<checked_trees::CheckedCallScalarArgument> {
    let role = CheckedScalarExpressionRole::Guard;
    if let Some(expression) =
        facts
            .values
            .scalar_expressions
            .expression_at(state, statement_ordinal, role)
    {
        return Some(checked_trees::CheckedCallScalarArgument::Pure(
            expression.clone(),
        ));
    }
    let computations = &facts.values.scalar_computations;
    let root = computations.root_at(state, statement_ordinal, role)?;
    (root.machine == machine && computations.nodes.is_valid(root.root)).then_some(
        checked_trees::CheckedCallScalarArgument::Computation(root.root),
    )
}

pub(in crate::execution::terminal_unit) fn guard_is_boolean(
    facts: &CheckFacts,
    guard: &checked_trees::CheckedCallScalarArgument,
) -> bool {
    match guard {
        checked_trees::CheckedCallScalarArgument::Pure(expression) => {
            expression.primitive_type() == Some(PrimitiveType::Bool)
        }
        checked_trees::CheckedCallScalarArgument::Computation(root) => {
            facts
                .values
                .scalar_computations
                .nodes
                .get(*root)
                .primitive_type
                == PrimitiveType::Bool
        }
    }
}
