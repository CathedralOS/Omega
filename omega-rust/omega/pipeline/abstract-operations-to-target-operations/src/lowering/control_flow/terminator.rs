//! Result-bearing exits and exact successor telescopes.
use super::LiveDefinitions;
use super::{observations, structural_case};
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractParameter,
};
use semantic_vocabulary::{PlaceId, ScalarType, StructuralTypeId};
use std::collections::BTreeSet;
use target_operations::{TargetBooleanExpression, TargetScalarExpression, TerminalPsiProvenance};
use target_operations::{TargetControlSuccessor, TargetControlTerminator};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralPathSegment, TerminalAffineCleanupAction,
};

// An unobserved owned arrival has no live storage home, so its discard keys
// on the arrival declaration — the same owned affine contract a realized home
// would report.
fn owned_arrival(function: &AbstractFunction, place: PlaceId) -> bool {
    function
        .structural_parameters
        .iter()
        .chain(
            function
                .block_entries
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .any(|parameter| {
            parameter.place == place
                && parameter.access == StructuralAccess::Owned
                && parameter.multiplicity == StructuralMultiplicity::Affine
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
                && function.entry_claims.is_empty()
        })
}

fn owned_root_type(
    live: &LiveDefinitions,
    function: &AbstractFunction,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    if let Some(home) = live.structural_homes.get(&place) {
        return (home.multiplicity() == StructuralMultiplicity::Affine
            && !home.has_claims()
            && home.qualifications().is_empty()
            && home.projected_qualifications().is_empty())
        .then_some(home.structural_type());
    }
    if owned_arrival(function, place) {
        return function
            .structural_parameters
            .iter()
            .chain(
                function
                    .block_entries
                    .iter()
                    .flat_map(|block| &block.structural_parameters),
            )
            .find(|parameter| parameter.place == place)
            .map(|parameter| parameter.structural_type);
    }
    live.references.get(&(place, Vec::new())).and_then(|leaf| {
        (leaf.result.multiplicity == StructuralMultiplicity::Affine)
            .then_some(leaf.result.structural_type)
    })
}

fn live_home_or_arrival(
    live: &LiveDefinitions,
    function: &AbstractFunction,
    place: PlaceId,
) -> bool {
    owned_root_type(live, function, place).is_some()
}

fn residual_cleanup(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    structural_types: &StructuralTypeLookup<'_>,
    discard: &terminal_psi::StructuralAffineDiscard,
    discharged_roots: &BTreeSet<PlaceId>,
    discharged_residuals: &BTreeSet<(PlaceId, Vec<StructuralPathSegment>)>,
) -> bool {
    // A residual is a strictly projected subtree of a live root; the verifier
    // names maximal complements of moved paths, never a whole root.
    if discard.path.is_empty()
        || discharged_roots.contains(&discard.place)
        || discharged_residuals.iter().any(|(place, path)| {
            *place == discard.place
                && (path.starts_with(&discard.path) || discard.path.starts_with(path))
        })
    {
        return false;
    }
    // Only a plain subtree needs no executable disposal; a loan anchored
    // inside or above the discarded boundary has no custody to move to. The
    // retained path must also resolve to the declared residual type.
    if super::references::contains_reference(structural_types, discard.structural_type)
        || live.references.keys().any(|(carrier, leaf_path)| {
            *carrier == discard.place
                && (leaf_path.starts_with(&discard.path) || discard.path.starts_with(leaf_path))
        })
    {
        return false;
    }
    owned_root_type(live, function, discard.place).is_some_and(|root| {
        structural_types.subtree(root, &discard.path) == Some(discard.structural_type)
    }) && !super::references::is_suspended_root(live, discard.place)
}

pub(super) fn plain_home_cleanup(
    function: &AbstractFunction,
    live: &mut LiveDefinitions,
    structural_types: &StructuralTypeLookup<'_>,
    actions: &[TerminalAffineCleanupAction],
) -> Result<bool, LoweringError> {
    let mut discarded_roots = BTreeSet::new();
    let mut discarded_residuals = BTreeSet::new();
    for action in actions {
        match action {
            TerminalAffineCleanupAction::DiscardRoot(place) => {
                if !discarded_roots.insert(*place) {
                    return Ok(false);
                }
                if !live_home_or_arrival(live, function, *place)
                    || super::references::is_suspended_root(live, *place)
                {
                    return Ok(false);
                }
            }
            TerminalAffineCleanupAction::DiscardResidual(discard) => {
                if !residual_cleanup(
                    function,
                    live,
                    structural_types,
                    discard,
                    &discarded_roots,
                    &discarded_residuals,
                ) {
                    return Ok(false);
                }
                discarded_residuals.insert((discard.place, discard.path.clone()));
            }
            TerminalAffineCleanupAction::InvokeNominal(_) => return Ok(false),
        }
    }
    // Admission is staged before custody moves: a discarded owner also ends
    // every reference leaf it carries, in the verified reverse-declaration
    // order. This removes loan custody, never referent storage. Residual
    // discards carry plain subtrees only, so they move no loan custody.
    for place in discarded_roots {
        super::references::discard_owned(function, structural_types, live, place)?;
    }
    Ok(true)
}

pub(super) fn lower_terminator(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &mut LiveDefinitions,
    structural_types: &StructuralTypeLookup<'_>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<TargetControlTerminator, LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    if live.nonreturning
        && !matches!(operation, AbstractOperation::ReturnUnit { cleanup_actions, .. } if cleanup_actions.is_empty())
    {
        return Err(LoweringError::InvalidHostedExitProcessShape(
            function.machine,
        ));
    }
    // Plain aggregate homes need no executable destructor, but their exact
    // edge-local disposition remains part of the independently replayed plan.
    // A constructor temporary can die before a join, not only at function exit.
    // Owned parameters owe the same no-code disposition after observation;
    // current ownership validation, not their physical home kind, establishes
    // availability and exact disposal order.
    let cleanup = |live: &mut LiveDefinitions, places: &[PlaceId]| {
        let actions = places
            .iter()
            .copied()
            .map(TerminalAffineCleanupAction::DiscardRoot)
            .collect::<Vec<_>>();
        if !plain_home_cleanup(function, live, structural_types, &actions)? {
            return Err(invalid());
        }
        Ok(actions)
    };
    let successor = |live: &mut LiveDefinitions,
                     edge: &abstract_operations::AbstractSuccessor|
     -> Result<TargetControlSuccessor, LoweringError> {
        Ok(TargetControlSuccessor {
            psi_edge: edge.psi_edge,
            target: edge.target,
            bindings: edge.bindings.clone(),
            structural_bindings: edge.structural_bindings.clone(),
            cleanup_actions: cleanup(live, &edge.trivial_affine_discards)?,
        })
    };
    match operation {
        AbstractOperation::Crash {
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => {
            // Crashing abandons this execution domain: there is no result,
            // successor transport, or implicit owner cleanup to realize.
            provenance.edges.push(*psi_edge);
            Ok(TargetControlTerminator::Crash {
                psi_edge: *psi_edge,
                cause: *cause,
                site_guard: site_guard.clone(),
                frontier_lower_bound: frontier_lower_bound.clone(),
            })
        }
        AbstractOperation::ReturnStructural {
            psi_edge,
            source,
            returned_claims,
            trivial_affine_locals,
            trivial_affine_discards,
        } => {
            let result = function.result.structural().ok_or_else(invalid)?;
            let cleanup_actions = cleanup(live, trivial_affine_discards)?;
            let reference_bearing =
                super::references::contains_reference(structural_types, result.structural_type);
            if let Some(parameter) = function
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == *source)
            {
                let actual = prepared
                    .parameters
                    .iter()
                    .find(|parameter| parameter.place == *source)
                    .ok_or_else(invalid)?;
                let placement = prepared.call_plan.result.as_ref().ok_or_else(invalid)?;
                if parameter.access != StructuralAccess::Owned
                    || parameter.multiplicity == StructuralMultiplicity::Linear
                    || parameter.is_self
                    || parameter.structural_type != result.structural_type
                    || parameter.multiplicity != result.multiplicity
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                    || !returned_claims.is_empty()
                    || !trivial_affine_locals.is_empty()
                    || actual.shape != placement.shape
                {
                    return Err(invalid());
                }
                // The complete prepared call plan already fixes both placements.
                // Returning the same value does not require identical input and
                // result registers, nor a direct-register ABI on either side.
                // Selection realizes the transfer from captured input storage.
                if reference_bearing {
                    super::references::return_custody(
                        function,
                        live,
                        structural_types,
                        *source,
                        result,
                    )?;
                }
                provenance.edges.push(*psi_edge);
                return Ok(TargetControlTerminator::ReturnStructural {
                    psi_edge: *psi_edge,
                    source: target_operations::TargetStructuralReturnSource::Parameter(
                        actual.clone(),
                    ),
                    cleanup_actions,
                });
            }
            // A bare reference carrier returns through its canonical zero-byte
            // custody home; the referent is never read or transported.
            if reference_bearing && let Some(leaf) = live.references.get(&(*source, Vec::new())) {
                if !returned_claims.is_empty()
                    || !trivial_affine_locals.is_empty()
                    || result.multiplicity != leaf.result.multiplicity
                {
                    return Err(invalid());
                }
                super::references::return_custody(
                    function,
                    live,
                    structural_types,
                    *source,
                    result,
                )?;
                let operation = leaf.operation.ok_or_else(invalid)?;
                let home =
                    super::aggregate_results::home(operation, &leaf.result, structural_types)?;
                provenance.edges.push(*psi_edge);
                return Ok(TargetControlTerminator::ReturnStructural {
                    psi_edge: *psi_edge,
                    source: target_operations::TargetStructuralReturnSource::Home(home),
                    cleanup_actions,
                });
            }
            let home = live.structural_homes.get(source).ok_or_else(invalid)?;
            if result.structural_type != home.structural_type()
                || result.multiplicity != home.multiplicity()
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !returned_claims.is_empty()
                || !trivial_affine_locals.is_empty()
            {
                return Err(invalid());
            }
            if reference_bearing {
                super::references::return_custody(
                    function,
                    live,
                    structural_types,
                    *source,
                    result,
                )?;
            }
            provenance.edges.push(*psi_edge);
            Ok(TargetControlTerminator::ReturnStructural {
                psi_edge: *psi_edge,
                source: target_operations::TargetStructuralReturnSource::Home(home.clone()),
                cleanup_actions,
            })
        }
        AbstractOperation::Return {
            psi_edge,
            result,
            value,
            scalar_type,
            cleanup_actions,
        } => {
            let expected = function.result.scalar().ok_or_else(invalid)?;
            if *result != expected.value
                || *scalar_type != expected.scalar_type
                || (!cleanup_actions.is_empty()
                    && !plain_home_cleanup(function, live, structural_types, cleanup_actions)?)
            {
                return Err(invalid());
            }
            let expression = if matches!(scalar_type, ScalarType::IeeeFloat(_)) {
                TargetScalarExpression::IeeeFloat(super::scalar_sources::source(
                    *value, function, live,
                )?)
            } else {
                observations::scalar_values(live, &prepared.scalar_parameters)?
                    .remove(value)
                    .ok_or(LoweringError::UnknownValue(*value))?
                    .into_expression(*value)?
            };
            if expression.scalar_type() != *scalar_type {
                return Err(LoweringError::ValueTypeMismatch(*value));
            }
            provenance.edges.push(*psi_edge);
            Ok(TargetControlTerminator::ReturnScalar {
                psi_edge: *psi_edge,
                source_value: *value,
                expression,
                cleanup_actions: cleanup_actions.clone(),
            })
        }
        AbstractOperation::StructuralCase { .. } => structural_case::lower(
            operation,
            function,
            prepared,
            live,
            structural_types,
            provenance,
        ),
        AbstractOperation::ReturnUnit {
            psi_edge,
            cleanup_actions,
        } => {
            if function.result != AbstractFunctionResult::Unit {
                return Err(invalid());
            }
            // Current ownership validation owns the exact live frontier and
            // discard order. Native admission only proves each retained action
            // is a no-code discard of an available boundary result home.
            if !plain_home_cleanup(function, live, structural_types, cleanup_actions)? {
                return Err(invalid());
            }
            provenance.edges.push(*psi_edge);
            Ok(TargetControlTerminator::Return {
                psi_edge: *psi_edge,
                cleanup_actions: cleanup_actions.clone(),
            })
        }
        AbstractOperation::Jump {
            psi_edge,
            target,
            bindings,
            structural_bindings,
            trivial_affine_discards,
            residual_affine_discards,
        } => {
            // A residual boundary overlapping a transferred subtree would
            // dispose of moved storage; residuals live strictly beside the
            // edge's projected arguments.
            if residual_affine_discards.iter().any(|discard| {
                structural_bindings.iter().any(|binding| {
                    binding.argument.place == discard.place
                        && (binding.argument.path.starts_with(&discard.path)
                            || discard.path.starts_with(&binding.argument.path))
                })
            }) {
                return Err(invalid());
            }
            let mut cleanup_actions = trivial_affine_discards
                .iter()
                .copied()
                .map(TerminalAffineCleanupAction::DiscardRoot)
                .collect::<Vec<_>>();
            cleanup_actions.extend(
                residual_affine_discards
                    .iter()
                    .cloned()
                    .map(TerminalAffineCleanupAction::DiscardResidual),
            );
            if !plain_home_cleanup(function, live, structural_types, &cleanup_actions)? {
                return Err(invalid());
            }
            provenance.edges.push(*psi_edge);
            Ok(TargetControlTerminator::Jump {
                successor: TargetControlSuccessor {
                    psi_edge: *psi_edge,
                    target: *target,
                    bindings: bindings.clone(),
                    structural_bindings: structural_bindings.clone(),
                    cleanup_actions,
                },
            })
        }
        AbstractOperation::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            let expression = if let Some((position, parameter)) = prepared
                .scalar_parameters
                .iter()
                .enumerate()
                .find(|(_, parameter)| {
                    parameter.value == *condition && parameter.scalar_type == ScalarType::Boolean
                }) {
                TargetBooleanExpression::Parameter {
                    source_value: *condition,
                    parameter_index: position,
                    location: crate::lowering::scalar::scalar_parameter_location(
                        &AbstractParameter {
                            value: *condition,
                            scalar_type: ScalarType::Boolean,
                        },
                        &parameter.placement,
                    )?,
                }
            } else if let Some((_, value)) = live.booleans.get(condition) {
                TargetBooleanExpression::Immediate {
                    source_value: *condition,
                    value: *value,
                }
            } else if let Some(home) = live.scalar_homes.get(condition)
                && home.scalar_type == ScalarType::Boolean
            {
                TargetBooleanExpression::ScalarHome(*home)
            } else if let Some(parameter) = live.scalar_block_parameters.get(condition)
                && parameter.scalar_type == ScalarType::Boolean
            {
                TargetBooleanExpression::BlockParameter(*parameter)
            } else {
                return Err(invalid());
            };
            provenance
                .edges
                .extend([when_true.psi_edge, when_false.psi_edge]);
            Ok(TargetControlTerminator::Conditional {
                condition_source: *condition,
                condition: expression,
                when_true: successor(live, when_true)?,
                when_false: successor(live, when_false)?,
            })
        }
        _ => Err(invalid()),
    }
}
