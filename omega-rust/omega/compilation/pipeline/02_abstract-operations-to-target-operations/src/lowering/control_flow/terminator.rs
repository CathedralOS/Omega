//! Result-bearing exits and exact successor telescopes.
use super::LiveDefinitions;
use super::{observations, structural_case};
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractParameter,
};
use semantic_vocabulary::{PlaceId, ScalarType, StructuralTypeId};
use target_operations::{TargetBooleanExpression, TargetScalarExpression, TerminalPsiProvenance};
use target_operations::{TargetControlSuccessor, TargetControlTerminator};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, TerminalAffineCleanupAction};

/// A materialized descriptor owns its carrier, not the borrowed backing.
/// A live descriptor comes from an operation or a block arrival. Rejoin its
/// exact establishment contract before accepting a no-code discard; a block
/// binding is not an owned backing home and has no operation-result producer.
/// Copyability permits independent descriptors; it does not keep a replaced
/// descriptor alive or authorize disposal of a linear carrier.
fn disposable_view_carrier(
    live: &LiveDefinitions,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    place: PlaceId,
) -> Option<StructuralMultiplicity> {
    let (structural_type, multiplicity) = if live.views.contains_key(&place) {
        let result = live.view_result(function, place)?;
        if !matches!(
            result.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        ) || !result.claims.is_empty()
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
        {
            return None;
        }
        (result.structural_type, result.multiplicity)
    } else {
        if !live.block_views.contains(&place) {
            return None;
        }
        let parameter = function.arrival_parameter(place)?;
        if !parameter.is_unqualified_root() {
            return None;
        }
        (parameter.structural_type, parameter.multiplicity)
    };
    types
        .get(&structural_type)
        .is_some_and(|declaration| {
            matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::ElementView { .. }
            ) || declaration.shape.is_borrowed_byte_view()
        })
        .then_some(multiplicity)
}

/// Rejoin the selected cleanup action to existing owned storage. The checked
/// action stream settles claims and orders nominal hooks before descendant
/// disposal. A linear root may therefore contain a plain residual after its
/// hook runs; root multiplicity cannot decide that subtree's disposition.
fn owned_root_type(
    live: &LiveDefinitions,
    function: &AbstractFunction,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    // Empty affine locals have an establishment but deliberately no storage
    // home. They still owe the same exact, edge-local root disposition.
    if let Some((_, structural_type)) = live.trivial_affine_locals.get(&place) {
        return Some(*structural_type);
    }
    if let Some(home) = live.structural_homes.get(&place) {
        return (home.access() == StructuralAccess::Owned).then_some(home.structural_type());
    }
    // Incoming values can remain in ABI fragments without a materialized
    // home. Their exact arrival declaration still names the original storage.
    if let Some(parameter) = function
        .arrival_parameter(place)
        .filter(|parameter| parameter.access == StructuralAccess::Owned)
    {
        return Some(parameter.structural_type);
    }
    None
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
) -> bool {
    // Region disposal does not retire its home. A selected no-code alternative
    // can dispose the whole value while a later RetireHome ends the binding.
    // Type and backing checks locate the selected storage, not permission to
    // dispose it. No executable destructor is needed for this plain subtree.
    owned_root_type(live, function, discard.place).is_some_and(|root| {
        structural_types.subtree(root, &discard.path) == Some(discard.structural_type)
    })
}

pub(super) fn plain_home_cleanup(
    function: &AbstractFunction,
    live: &mut LiveDefinitions,
    structural_types: &StructuralTypeLookup<'_>,
    actions: &[TerminalAffineCleanupAction],
) -> Result<(), LoweringError> {
    // The checked action stream owns semantic disposition and ordering. A
    // nominal hook retires its own obligation before descendant hooks run;
    // invoking it is not destruction of every field in its receiver region.
    // Resolve each selected receiver against live storage instead of imposing
    // a disjoint-region admission rule on this already checked sequence.
    for action in actions {
        match action {
            TerminalAffineCleanupAction::RetireHome(place) => {
                // The checked partition has already disposed the live regions.
                // Dominating geometry remains available to sibling edges; this
                // marker neither disposes reserved fields nor prunes backing
                // captured by another carrier.
                if owned_root_type(live, function, *place).is_none()
                    && !live.retired_reference_roots.contains(place)
                    && disposable_view_carrier(live, function, structural_types, *place).is_none()
                {
                    return Err(LoweringError::InvalidStructuralValueInput {
                        machine: function.machine,
                        place: *place,
                        reason: "home retirement has no owned home or materialized view carrier",
                    });
                }
                // Retire the carrier's remaining metadata, not its pointees.
                // A signature roster includes inactive alternatives after a
                // selected payload has moved; only the checked edge decides
                // which residents owed disposition. Moved leaves and copied
                // descriptors retain their metadata at their own homes.
                live.references.retain(|(carrier, path), _| {
                    carrier != place
                        || path.contains(&terminal_psi::StructuralPathSegment::Referent)
                });
            }
            TerminalAffineCleanupAction::DiscardRoot(place) => {
                if live.views.contains_key(place) || live.block_views.contains(place) {
                    if disposable_view_carrier(live, function, structural_types, *place)
                        != Some(StructuralMultiplicity::Affine)
                    {
                        return Err(LoweringError::InvalidStructuralValueInput {
                            machine: function.machine,
                            place: *place,
                            reason: "view discard has no affine descriptor carrier",
                        });
                    }
                    // Keep the dominating definition available for sibling-edge
                    // reconstruction. The target cleanup action retires this
                    // path's carrier; no owned backing home is created or moved.
                    continue;
                }
                // Psi supplies the disposal order. Retire metadata from the
                // selected carrier storage without replaying loan permission.
                if live
                    .references
                    .keys()
                    .any(|(carrier, _)| *carrier == *place)
                    || live.retired_reference_roots.contains(place)
                {
                    super::references::discard_owned(function, structural_types, live, *place)?;
                    continue;
                }
                if !live_home_or_arrival(live, function, *place) {
                    return Err(LoweringError::InvalidStructuralValueInput {
                        machine: function.machine,
                        place: *place,
                        reason: "root discard has no live home or arrival",
                    });
                }
                super::references::discard_owned(function, structural_types, live, *place)?;
            }
            TerminalAffineCleanupAction::DiscardResidual(discard) => {
                if !residual_cleanup(function, live, structural_types, discard) {
                    return Err(LoweringError::InvalidStructuralValueInput {
                        machine: function.machine,
                        place: discard.place,
                        reason: "residual discard does not match the owned storage subtree",
                    });
                }
                super::references::discard_subtree(
                    function,
                    structural_types,
                    live,
                    discard.place,
                    &discard.path,
                )?;
            }
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                // The selected owner remains intact when the hook borrows it;
                // moving a disjoint sibling does not change its storage.
                if owned_root_type(live, function, cleanup.place)
                    .and_then(|root| structural_types.subtree(root, &cleanup.path))
                    != Some(cleanup.structural_type)
                    || !cleanup.requirement_obligations.is_empty()
                {
                    return Err(LoweringError::InvalidStructuralValueInput {
                        machine: function.machine,
                        place: cleanup.place,
                        reason: "nominal cleanup receiver or requirement obligations are not realized",
                    });
                }
                // The ordinary borrowed call above already transports runtime
                // selectors. Cleanup retains that captured receiver rather
                // than requiring a second, static-only address route.
                // Normal hook completion retires the same owner's embedded
                // loans before the next action or successor. Physical storage
                // stays live through the edge-owned call selected above.
                super::references::discard_subtree(
                    function,
                    structural_types,
                    live,
                    cleanup.place,
                    &cleanup.path,
                )?;
            }
        }
    }
    Ok(())
}

pub(super) fn lower_terminator(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &mut LiveDefinitions,
    structural_types: &StructuralTypeLookup<'_>,
    target: target::NativeTarget,
    boundary_machines: &std::collections::BTreeMap<
        semantic_vocabulary::BoundaryMachineId,
        &terminal_psi::BoundaryMachineDeclaration,
    >,
    settlements: &std::collections::BTreeMap<
        semantic_vocabulary::BoundaryMachineId,
        target_operations::BoundarySettlementBinding,
    >,
    provenance: &mut TerminalPsiProvenance,
) -> Result<TargetControlTerminator, LoweringError> {
    // Plain aggregate homes need no executable destructor, but their exact
    // edge-local disposition remains in the source-selected action stream.
    // A constructor temporary can die before a join, not only at function exit.
    // Owned parameters owe the same no-code disposition after observation;
    // current ownership validation, not their physical home kind, establishes
    // availability and exact disposal order.
    let cleanup = |live: &mut LiveDefinitions,
                   actions: &[TerminalAffineCleanupAction]|
     -> Result<Vec<TerminalAffineCleanupAction>, LoweringError> {
        plain_home_cleanup(function, live, structural_types, actions)?;
        Ok(actions.to_vec())
    };
    let successor = |live: &mut LiveDefinitions,
                     edge: &abstract_operations::AbstractSuccessor|
     -> Result<TargetControlSuccessor, LoweringError> {
        let destination = function
            .block_entry(edge.target)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        super::transfers::re_home_bound_references(
            function,
            structural_types,
            &edge.structural_bindings,
            &destination.structural_parameters,
            live,
        )?;
        Ok(TargetControlSuccessor {
            psi_edge: edge.psi_edge,
            target: edge.target,
            bindings: edge.bindings.clone(),
            structural_bindings: edge.structural_bindings.clone(),
            cleanup_actions: cleanup(live, &edge.cleanup_actions)?,
        })
    };
    match operation {
        AbstractOperation::ExternalCompletion {
            psi_edge,
            boundary,
            kind,
            arguments,
            structural_arguments,
            requirement_obligations,
            frontier_lower_bound,
        } => {
            let declaration = boundary_machines
                .get(boundary)
                .copied()
                .ok_or(LoweringError::UnknownBoundarySettlement(*boundary))?;
            let binding = settlements
                .get(boundary)
                .cloned()
                .ok_or(LoweringError::MissingBoundarySettlement(*boundary))?;
            if declaration.completion != semantic_vocabulary::BoundaryCompletion::External(*kind)
                || !declaration.result.is_unit()
                || declaration.scalar_parameters.len() != arguments.len()
                || declaration.structural_parameters.len() != structural_arguments.len()
            {
                return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
            }
            let shapes = declaration
                .scalar_parameters
                .iter()
                .map(|scalar_type| {
                    crate::lowering::scalar_abi::fixed_native_scalar_shape(*scalar_type)
                        .ok_or(LoweringError::BoundaryRealizationMismatch(*boundary))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let signature = calling_conventions::CallSignature {
                parameters: shapes,
                result: None,
            };
            // An external completion still uses its selected counterparty's
            // exact ABI. Native defaults cannot replace the admitted entry plan
            // (notably supervisor entries and explicitly selected foreign ABIs).
            let call_plan = match &binding.realization {
                target_operations::BoundarySettlementRealization::NormalizedForeignCall(
                    foreign,
                ) => calling_conventions::validate_boundary_entry_plan(
                    foreign.boundary_entry_plan.clone(),
                    &signature,
                )
                .map_err(LoweringError::AbiPlan)?
                .plan()
                .call
                .clone(),
                _ => calling_conventions::evaluate_call_plan(
                    calling_conventions::CallingPolicy::native_for_target(target),
                    &signature,
                )
                .map_err(LoweringError::AbiPlan)?,
            };
            let arguments = arguments
                .iter()
                .zip(&call_plan.parameters)
                .zip(&declaration.scalar_parameters)
                .enumerate()
                .map(|(position, ((source, placement), expected))| {
                    let source = super::scalar_sources::source(*source, function, live)?;
                    if source.scalar_type() != *expected {
                        return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                    }
                    Ok(target_operations::TargetUnitScalarCallArgument {
                        parameter_index: u32::try_from(position)
                            .map_err(|_| LoweringError::BoundaryRealizationMismatch(*boundary))?,
                        source,
                        placement: placement.clone(),
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            provenance.edges.push(*psi_edge);
            Ok(TargetControlTerminator::ExternalCompletion {
                psi_edge: *psi_edge,
                boundary: *boundary,
                kind: *kind,
                binding,
                arguments,
                structural_arguments: structural_arguments.clone(),
                requirement_obligations: requirement_obligations.clone(),
                frontier_lower_bound: frontier_lower_bound.clone(),
            })
        }
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
            cleanup_actions,
        } => {
            let result = function
                .result
                .structural()
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            plain_home_cleanup(function, live, structural_types, cleanup_actions)?;
            let cleanup_actions = cleanup_actions.clone();
            if terminal_semantics::descriptor_results::is_descriptor(
                structural_types.values().copied(),
                result.structural_type,
            ) {
                // Descriptor storage is not backing ownership. Every established
                // view uses the same value-result ABI, including a literal whose
                // octets live in the image rather than an operation-result home.
                let source_type = live
                    .structural_homes
                    .get(source)
                    .filter(|home| {
                        home.multiplicity() == result.multiplicity
                            && home.qualifications() == result.qualifications
                            && home.projected_qualifications().is_empty()
                    })
                    .map(|home| home.structural_type())
                    .or_else(|| {
                        live.views
                            .get(source)
                            .filter(|_| {
                                result.multiplicity == StructuralMultiplicity::Unrestricted
                                    || disposable_view_carrier(
                                        live,
                                        function,
                                        structural_types,
                                        *source,
                                    ) == Some(StructuralMultiplicity::Affine)
                            })
                            .map(|(_, identity)| *identity)
                    })
                    .or_else(|| {
                        function
                            .arrival_parameter(*source)
                            .filter(|parameter| {
                                (function
                                    .structural_parameters
                                    .iter()
                                    .any(|entry| entry.place == *source)
                                    || live.block_views.contains(source))
                                    && matches!(
                                        parameter.access,
                                        StructuralAccess::Owned
                                            | StructuralAccess::SharedBorrow
                                            | StructuralAccess::MutableBorrow
                                    )
                                    && (result.multiplicity != StructuralMultiplicity::Affine
                                        || parameter.access != StructuralAccess::SharedBorrow)
                                    && (parameter.multiplicity == result.multiplicity
                                        || result.multiplicity == StructuralMultiplicity::Affine
                                            && parameter.access == StructuralAccess::MutableBorrow
                                            && parameter.multiplicity
                                                == StructuralMultiplicity::Unrestricted)
                                    && parameter.qualifications == result.qualifications
                                    && parameter.projected_qualifications.is_empty()
                            })
                            .map(|parameter| parameter.structural_type)
                    });
                if source_type != Some(result.structural_type)
                    || !matches!(
                        result.multiplicity,
                        terminal_psi::nonlinear_multiplicity_pair_pattern!()
                    )
                    || !result.projected_qualifications.is_empty()
                    || !returned_claims.is_empty()
                    || !trivial_affine_locals.is_empty()
                    || prepared.call_plan.result.as_ref().is_none_or(|placement| {
                        placement.shape != calling_conventions::ValueShape::integer(16, 8)
                    })
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                provenance.edges.push(*psi_edge);
                return Ok(TargetControlTerminator::ReturnStructural {
                    psi_edge: *psi_edge,
                    source: target_operations::TargetStructuralReturnSource::SharedDescriptor {
                        place: *source,
                    },
                    cleanup_actions,
                });
            }
            if let Some(parameter) = function.structural_parameter(*source) {
                let actual = prepared
                    .parameter(*source)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                let placement = prepared
                    .call_plan
                    .result
                    .as_ref()
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                // Returned claims leave with the value; the verified
                // ownership replay already matched them to the source.
                if parameter.access != StructuralAccess::Owned
                    || parameter.structural_type != result.structural_type
                    || parameter.multiplicity != result.multiplicity
                    || !trivial_affine_locals.is_empty()
                    || actual.shape != placement.shape
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                // The complete prepared call plan already fixes both placements.
                // Returning the same value does not require identical input and
                // result registers, nor a direct-register ABI on either side.
                // Selection realizes the transfer from captured input storage.
                provenance.edges.push(*psi_edge);
                return Ok(TargetControlTerminator::ReturnStructural {
                    psi_edge: *psi_edge,
                    source: target_operations::TargetStructuralReturnSource::Parameter(
                        actual.clone(),
                    ),
                    cleanup_actions,
                });
            }
            // Return the current value's storage home, including reference
            // descriptors. Loan identity can predate this carrier (an ingress
            // leaf or a moved record field); it is not the return ABI's home.
            let home = live
                .structural_homes
                .get(source)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            // Psi proves the declared result domains at this return. The
            // source home names the physical value, not that proof's roster:
            // a field snapshot can establish the result through its field
            // predicate without carrying a root qualification itself. Return
            // preparation rejoins type and ABI extent, irrespective
            // of whether a parameter, call or field copy supplied the value.
            // The descriptor bytes travel with this home. Psi already admitted
            // returned loans; replaying their origins cannot choose storage or
            // emit code and belongs only to the independent corpus audit.
            if result.structural_type != home.structural_type()
                || result.multiplicity != home.multiplicity()
                || prepared
                    .call_plan
                    .result
                    .as_ref()
                    .is_none_or(|placement| placement.shape != home.layout.shape())
                || !trivial_affine_locals.is_empty()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
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
            let expected = function
                .result
                .scalar()
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            if *result != expected.value || *scalar_type != expected.scalar_type {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            plain_home_cleanup(function, live, structural_types, cleanup_actions)?;
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
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            // Current ownership validation owns the exact live frontier and
            // action order. Native admission retains each original owner home;
            // executable actions use the independently reconstructed call ABI.
            plain_home_cleanup(function, live, structural_types, cleanup_actions)?;
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
            cleanup_actions,
        } => {
            // A residual boundary overlapping a transferred subtree would
            // dispose of moved storage; residuals live strictly beside the
            // edge's projected arguments.
            if cleanup_actions.iter().any(|action| {
                let TerminalAffineCleanupAction::DiscardResidual(discard) = action else {
                    return false;
                };
                structural_bindings.iter().any(|binding| {
                    binding.argument.place == discard.place
                        && (binding.argument.path.starts_with(&discard.path)
                            || discard.path.starts_with(&binding.argument.path))
                })
            }) {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let destination = function
                .block_entry(*target)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            super::transfers::re_home_bound_references(
                function,
                structural_types,
                structural_bindings,
                &destination.structural_parameters,
                live,
            )?;
            let cleanup_actions = cleanup(live, cleanup_actions)?;
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
                return Err(LoweringError::unsupported_control_flow(function.machine));
            };
            provenance
                .edges
                .extend([when_true.psi_edge, when_false.psi_edge]);
            let mut true_exit = live.clone();
            let mut false_exit = live.clone();
            let lowered_true = successor(&mut true_exit, when_true)?;
            let lowered_false = successor(&mut false_exit, when_false)?;
            live.finish_reference_edges(vec![
                true_exit.reference_edge_exit(when_true.target),
                false_exit.reference_edge_exit(when_false.target),
            ]);
            Ok(TargetControlTerminator::Conditional {
                condition_source: *condition,
                condition: expression,
                when_true: lowered_true,
                when_false: lowered_false,
            })
        }
        _ => Err(LoweringError::unsupported_control_flow(function.machine)),
    }
}
