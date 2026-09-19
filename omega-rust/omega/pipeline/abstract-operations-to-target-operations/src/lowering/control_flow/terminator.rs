//! Result-bearing exits and exact successor telescopes.
use super::LiveDefinitions;
use super::{observations, structural_case};
use crate::lowering::shared::*;
use target_operations::{TargetControlSuccessor, TargetControlTerminator};

fn plain_home_cleanup(
    function: &AbstractFunction,
    live: &mut LiveDefinitions,
    structural_types: &StructuralTypeLookup<'_>,
    actions: &[TerminalAffineCleanupAction],
) -> Result<bool, LoweringError> {
    let mut discarded = BTreeSet::new();
    for action in actions {
        let TerminalAffineCleanupAction::DiscardRoot(place) = action else {
            return Ok(false);
        };
        if !discarded.insert(*place) {
            return Ok(false);
        }
        // An unobserved owned arrival has no live storage home, so its discard
        // keys on the arrival declaration — the same owned affine contract a
        // realized home would report.
        let arrival = function
            .structural_parameters
            .iter()
            .chain(
                function
                    .block_entries
                    .iter()
                    .flat_map(|block| &block.structural_parameters),
            )
            .any(|parameter| {
                parameter.place == *place
                    && parameter.access == StructuralAccess::Owned
                    && parameter.multiplicity == StructuralMultiplicity::Affine
                    && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
                    && function.entry_claims.is_empty()
            });
        let admitted = live.structural_homes.get(place).is_some_and(|home| {
            home.multiplicity() == StructuralMultiplicity::Affine
                && !home.has_claims()
                && home.qualifications().is_empty()
                && home.projected_qualifications().is_empty()
        }) || arrival
            || live
                .references
                .get(&(*place, Vec::new()))
                .is_some_and(|leaf| leaf.result.multiplicity == StructuralMultiplicity::Affine);
        // A suspended referent root cannot be discarded while its loan lives.
        if !admitted || super::references::is_suspended_root(live, *place) {
            return Ok(false);
        }
    }
    // Admission is staged before custody moves: a discarded owner also ends
    // every reference leaf it carries, in the verified reverse-declaration
    // order. This removes loan custody, never referent storage.
    for place in discarded {
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
        AbstractOperation::StructuralCase { .. } => {
            structural_case::lower(operation, function, live, structural_types, provenance)
        }
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
            if !residual_affine_discards.is_empty() {
                return Err(invalid());
            }
            provenance.edges.push(*psi_edge);
            Ok(TargetControlTerminator::Jump {
                successor: TargetControlSuccessor {
                    psi_edge: *psi_edge,
                    target: *target,
                    bindings: bindings.clone(),
                    structural_bindings: structural_bindings.clone(),
                    cleanup_actions: cleanup(live, trivial_affine_discards)?,
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
