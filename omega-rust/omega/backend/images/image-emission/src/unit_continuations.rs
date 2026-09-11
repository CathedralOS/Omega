//! Replay continuation cleanup at its real call boundary, separately from return.

use machine_code::{
    InternalUnitCallRecord, UnitAffineCleanupRecord, UnitContinuationRecord,
    UnitParameterHomeRecord,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralPathSegment, TerminalAffineCleanupAction,
};

struct LiveRoot<'calls> {
    place: PlaceId,
    structural_type: StructuralTypeId,
    source: RootSource<'calls>,
    moved: Vec<(&'calls [StructuralPathSegment], StructuralTypeId)>,
}

#[derive(Clone, Copy)]
enum RootSource<'calls> {
    Parameter(&'calls UnitParameterHomeRecord),
    Result(&'calls machine_code::InternalStructuralCallResult),
}

/// Validate the complete bounded call/continuation sequence. Returned places
/// are precisely the owners discharged before the final return.
pub(crate) fn completed_roots(
    parameters: &[UnitParameterHomeRecord],
    calls: &[InternalUnitCallRecord],
    continuations: &[UnitContinuationRecord],
    returned: Option<&UnitAffineCleanupRecord>,
) -> Option<Vec<PlaceId>> {
    if continuations.is_empty() {
        return Some(Vec::new());
    }
    let returned = returned?;
    if !returned.locals.is_empty()
        || parameters.iter().any(|parameter| {
            parameter.access != StructuralAccess::Owned
                || parameter.multiplicity != StructuralMultiplicity::Affine
        })
        || continuations.windows(2).any(|pair| {
            pair[0].operation_ordinal >= pair[1].operation_ordinal
                || pair[0].target_block != pair[1].source_block
        })
        || calls
            .windows(2)
            .any(|pair| pair[0].operation_ordinal >= pair[1].operation_ordinal)
    {
        return None;
    }
    let mut roots = parameters
        .iter()
        .map(|parameter| LiveRoot {
            place: parameter.place,
            structural_type: parameter.structural_type,
            source: RootSource::Parameter(parameter),
            moved: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut identities = roots.iter().map(|root| root.place).collect::<Vec<_>>();
    if identities
        .iter()
        .enumerate()
        .any(|(position, place)| identities[..position].contains(place))
    {
        return None;
    }
    let mut disposed = Vec::new();
    let mut blocks = vec![continuations.first()?.source_block];
    let mut edges = vec![returned.psi_edge];
    let mut owners = Vec::new();
    let mut call_position = 0;
    let mut continuation_position = 0;
    let mut previous_end = None;
    let operation_count = calls.len().checked_add(continuations.len())?;
    for operation_ordinal in 0..operation_count {
        if let Some(continuation) = continuations
            .get(continuation_position)
            .filter(|continuation| continuation.operation_ordinal == operation_ordinal)
        {
            if calls
                .get(call_position)
                .is_some_and(|call| call.operation_ordinal == operation_ordinal)
                || continuation.successor_operation_ordinal != operation_ordinal.checked_add(1)?
                || blocks.contains(&continuation.target_block)
                || edges.contains(&continuation.cleanup.psi_edge)
                || continuation.cleanup.byte_count != 0
                || !continuation.cleanup.locals.is_empty()
                || continuation.cleanup.structural_types != returned.structural_types
                || previous_end != Some(continuation.cleanup.code_offset)
            {
                return None;
            }
            blocks.push(continuation.target_block);
            edges.push(continuation.cleanup.psi_edge);
            let residuals = continuation
                .cleanup
                .actions
                .iter()
                .map(|action| match action {
                    TerminalAffineCleanupAction::DiscardResidual(residual) => Some(residual),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            let mut partial = roots
                .iter()
                .enumerate()
                .filter(|(_, root)| !root.moved.is_empty());
            let selected = partial.next();
            if partial.next().is_some() {
                return None;
            }
            if let Some((root_position, root)) = selected {
                if residuals
                    .iter()
                    .any(|residual| residual.place != root.place)
                    || !crate::exact_partial_cleanup_partition(
                        &returned.structural_types,
                        root.structural_type,
                        &root.moved,
                        &residuals,
                    )
                {
                    return None;
                }
                disposed.push(root.place);
                roots.remove(root_position);
            } else if !residuals.is_empty() {
                return None;
            }
            previous_end = Some(continuation.cleanup.code_offset);
            continuation_position += 1;
            continue;
        }
        let call = calls.get(call_position)?;
        if call.operation_ordinal != operation_ordinal
            || !matches!(call.owner, target_operations::CallSiteOwner::Operation(_))
            || owners.contains(&call.owner)
            || call.result.is_some()
            || call.semantic_result.is_some()
            || (!call.scalar_arguments.is_empty()
                && (!call.arguments.is_empty() || call.structural_result.is_some()))
            || !call.claim_transfers.is_empty()
            || previous_end.is_some_and(|end| end != call.code_offset)
        {
            return None;
        }
        owners.push(call.owner);
        previous_end = Some(call.code_offset.checked_add(call.byte_count)?);
        if let Some(result) = &call.structural_result {
            let [input] = call.arguments.as_slice() else {
                return None;
            };
            let source_position = roots.iter().position(|root| root.place == input.place)?;
            let source = &roots[source_position];
            let RootSource::Parameter(parameter) = source.source else {
                return None;
            };
            let home = result.result_home.as_ref()?;
            let (defining_operation, retained_result) = home.requirement.operation_result()?;
            if !source.moved.is_empty()
                || !input.path.is_empty()
                || input.access != StructuralAccess::Owned
                || input.root_structural_type != source.structural_type
                || input.structural_type != source.structural_type
                || input.shape != parameter.shape
                || result.operation_result.structural_type != source.structural_type
                || result.operation_result.multiplicity != StructuralMultiplicity::Affine
                || !result.operation_result.qualifications.is_empty()
                || !result.operation_result.projected_qualifications.is_empty()
                || !result.operation_result.claims.is_empty()
                || !result.returned_claim_transfers.is_empty()
                || !result.returned_claims.is_empty()
                || identities.contains(&result.operation_result.place)
                || retained_result != &result.operation_result
                || call.owner != target_operations::CallSiteOwner::Operation(defining_operation)
                || home.requirement.layout.shape() != parameter.shape
                || !matches!(
                    home.requirement.layout,
                    target_operations::TargetStructuralHomeLayout::Aggregate(_)
                )
                || home.code_offset < call.code_offset
                || home.code_offset.checked_add(home.byte_count)? > previous_end?
            {
                return None;
            }
            roots.remove(source_position);
            identities.push(result.operation_result.place);
            roots.push(LiveRoot {
                place: result.operation_result.place,
                structural_type: result.operation_result.structural_type,
                source: RootSource::Result(result),
                moved: Vec::new(),
            });
        } else if !call.arguments.is_empty() {
            let [argument] = call.arguments.as_slice() else {
                return None;
            };
            let root = roots.iter_mut().find(|root| root.place == argument.place)?;
            if argument.path.is_empty()
                || argument.access != StructuralAccess::Owned
                || root.moved.iter().any(|(earlier, _)| {
                    earlier.starts_with(&argument.path) || argument.path.starts_with(earlier)
                })
            {
                return None;
            }
            let exact = match root.source {
                RootSource::Parameter(parameter) => {
                    crate::affine_projected_calls::exact_owned_projection(
                        argument,
                        parameter,
                        &returned.structural_types,
                    )
                }
                RootSource::Result(result) => {
                    result.result_home.as_ref().is_some_and(|home| {
                        home.code_offset
                            .checked_add(home.byte_count)
                            .is_some_and(|end| end <= argument.code_offset)
                    }) && crate::affine_projected_calls::exact_owned_result_projection(
                        argument,
                        result,
                        &returned.structural_types,
                    )
                }
            };
            if !exact {
                return None;
            }
            root.moved.push((&argument.path, argument.structural_type));
        }
        call_position += 1;
    }
    if call_position != calls.len()
        || continuation_position != continuations.len()
        || previous_end != Some(returned.code_offset)
        || roots.iter().any(|root| !root.moved.is_empty())
    {
        return None;
    }
    Some(disposed)
}

pub(crate) fn result_for_call<'calls>(
    calls: &'calls [InternalUnitCallRecord],
    disposed: &[PlaceId],
    call: &InternalUnitCallRecord,
) -> Option<&'calls machine_code::InternalStructuralCallResult> {
    let place = call
        .structural_result
        .as_ref()
        .map(|result| result.operation_result.place)
        .or(match call.arguments.as_slice() {
            [argument] => Some(argument.place),
            _ => None,
        })?;
    if !disposed.contains(&place) {
        return None;
    }
    calls
        .iter()
        .filter(|producer| producer.operation_ordinal <= call.operation_ordinal)
        .filter_map(|producer| producer.structural_result.as_ref())
        .find(|result| result.operation_result.place == place)
}

pub(crate) fn cleanup_for_call(
    continuations: &[UnitContinuationRecord],
    operation_ordinal: usize,
) -> Option<&UnitAffineCleanupRecord> {
    continuations
        .iter()
        .find(|continuation| continuation.operation_ordinal > operation_ordinal)
        .map(|continuation| &continuation.cleanup)
}

pub(crate) fn exact_attribution(
    continuations: &[UnitContinuationRecord],
    returned: Option<&UnitAffineCleanupRecord>,
    call_count: usize,
    attribution: &[machine_code::SemanticCodeAttribution],
) -> bool {
    let Some(returned) = returned else {
        return continuations.is_empty();
    };
    if continuations.is_empty() {
        let other_edges = attribution
            .iter()
            .filter(|row| {
                matches!(row.site,
            machine_code::SemanticCodeSite::Edge(edge) if edge != returned.psi_edge)
            })
            .collect::<Vec<_>>();
        // Ordinary conditional dispatch owns nonzero branch-edge evidence;
        // only an otherwise complete call-only fallthrough transaction has
        // unaccounted zero-byte edges after its continuation rows are removed.
        return other_edges.is_empty()
            || other_edges.iter().any(|row| row.byte_count != 0)
            || attribution
                .iter()
                .filter(|row| matches!(row.site, machine_code::SemanticCodeSite::Operation(_)))
                .count()
                != call_count;
    }
    attribution.len() == call_count + continuations.len() + 1
        && continuations.iter().all(|continuation| {
            attribution
                .iter()
                .filter(|row| {
                    row.site == machine_code::SemanticCodeSite::Edge(continuation.cleanup.psi_edge)
                        && row.operation_ordinal == continuation.operation_ordinal
                        && row.code_offset == continuation.cleanup.code_offset
                        && row.byte_count == 0
                })
                .count()
                == 1
        })
        && attribution.last().is_some_and(|row| {
            row.site == machine_code::SemanticCodeSite::Edge(returned.psi_edge)
                && row.operation_ordinal == call_count + continuations.len()
                && row.code_offset == returned.code_offset
                && row.byte_count == returned.byte_count
        })
}

pub(crate) fn exact_projected_callee(
    call: &InternalUnitCallRecord,
    parameters: &[machine_code::UnitParameterRecord],
    cleanup: Option<&UnitAffineCleanupRecord>,
    caller_cleanup: &UnitAffineCleanupRecord,
    attribution: &[machine_code::SemanticCodeAttribution],
) -> bool {
    let [argument] = call.arguments.as_slice() else {
        return false;
    };
    let [parameter] = parameters else {
        return false;
    };
    let Some(cleanup) = cleanup else {
        return false;
    };
    argument.structural_type == parameter.structural_type
        && argument.shape == parameter.shape
        && argument.access == StructuralAccess::Owned
        && parameter.access == StructuralAccess::Owned
        && parameter.multiplicity == StructuralMultiplicity::Affine
        && cleanup.locals.is_empty()
        && cleanup.actions.as_slice() == [TerminalAffineCleanupAction::DiscardRoot(parameter.place)]
        && matches!(attribution, [row] if row.site == machine_code::SemanticCodeSite::Edge(cleanup.psi_edge)
            && row.operation_ordinal == 0 && row.code_offset == cleanup.code_offset
            && row.byte_count == cleanup.byte_count)
        && cleanup
            .structural_types
            .iter()
            .filter(|declaration| {
                caller_cleanup
                    .structural_types
                    .iter()
                    .any(|source| source.id == declaration.id)
            })
            .all(|declaration| caller_cleanup.structural_types.contains(declaration))
        && cleanup
            .structural_types
            .iter()
            .any(|declaration| declaration.id == argument.structural_type)
}

pub(crate) fn validate_function(
    function: &machine_code::MachineCodeFunction,
) -> Result<Vec<PlaceId>, crate::ObjectError> {
    let invalid = || crate::ObjectError::InvalidUnitAffineCleanupEvidence(function.machine);
    if function.unit_continuations.is_empty() {
        if function
            .parameter_abi
            .as_ref()
            .is_some_and(|abi| !abi.entry_register_spills.is_empty())
        {
            return Err(invalid());
        }
        // An unrecorded zero-byte edge may not be mistaken for a return.
        if function.unit_stack.is_some()
            && !exact_attribution(
                &[],
                function.unit_affine_cleanup.as_ref(),
                function.internal_unit_calls.len(),
                &function.semantic_code_attribution,
            )
        {
            return Err(invalid());
        }
        return Ok(Vec::new());
    }
    if function.unit_stack.is_none()
        || function.scalar_stack.is_some()
        || function.scalar_abi.is_some()
        || function.structural_return.is_some()
        || function.scalar_affine_cleanup.is_some()
        || !function.scalar_control_affine_cleanups.is_empty()
        || function.structural_call_scalar_return.is_some()
        || !exact_scalar_bindings(
            function.parameter_abi.as_ref(),
            &function.unit_continuations,
        )
        || !function.boundary_settlements.is_empty()
        || !function.foreign_calls.is_empty()
        || !function.unit_scalar_homes.is_empty()
        || !function.unit_integer_constants.is_empty()
        || !function.unit_affine_scalar_records.is_empty()
        || !function.unit_structural_scalar_field_stores.is_empty()
        || !function.unit_write_only_primitive_stores.is_empty()
        || !function.internal_unit_scalar_calls.is_empty()
        || !function.installed_provider_unit_scalar_calls.is_empty()
        || !function.dynamic_calls.is_empty()
        || !function.stored_dynamic_calls.is_empty()
        || !function.dynamic_parameter_calls.is_empty()
        || !function.forwarded_dynamic_parameter_calls.is_empty()
        || !function.forwarded_dynamic_descriptor_calls.is_empty()
    {
        return Err(invalid());
    }
    let returned = function.unit_affine_cleanup.as_ref().ok_or_else(invalid)?;
    let completed = completed_roots(
        &function.unit_parameter_homes,
        &function.internal_unit_calls,
        &function.unit_continuations,
        Some(returned),
    )
    .ok_or_else(invalid)?;
    if function.provenance.edges.len() != function.unit_continuations.len() + 1
        || function.provenance.edges.last() != Some(&returned.psi_edge)
        || function.semantic_code_attribution.len()
            != function.internal_unit_calls.len() + function.unit_continuations.len() + 1
    {
        return Err(invalid());
    }
    for (position, continuation) in function.unit_continuations.iter().enumerate() {
        if function.provenance.edges.get(position) != Some(&continuation.cleanup.psi_edge)
            || function
                .semantic_code_attribution
                .iter()
                .filter(|attribution| {
                    attribution.site
                        == machine_code::SemanticCodeSite::Edge(continuation.cleanup.psi_edge)
                        && attribution.operation_ordinal == continuation.operation_ordinal
                        && attribution.code_offset == continuation.cleanup.code_offset
                        && attribution.byte_count == 0
                })
                .count()
                != 1
        {
            return Err(invalid());
        }
    }
    let return_ordinal = function.internal_unit_calls.len() + function.unit_continuations.len();
    if function
        .semantic_code_attribution
        .last()
        .is_none_or(|attribution| {
            attribution.site != machine_code::SemanticCodeSite::Edge(returned.psi_edge)
                || attribution.operation_ordinal != return_ordinal
                || attribution.code_offset != returned.code_offset
                || attribution.byte_count != returned.byte_count
        })
    {
        return Err(invalid());
    }
    Ok(completed)
}

/// Source-to-target validation owns the authored alias use. This source-free
/// boundary independently checks retained definitions and simultaneous types;
/// canonical call sources still rejoin the original ABI parameter separately.
pub(crate) fn exact_scalar_bindings(
    abi: Option<&machine_code::ParameterFunctionAbiRecord>,
    continuations: &[UnitContinuationRecord],
) -> bool {
    let mut values = std::collections::BTreeMap::new();
    if let Some(abi) = abi {
        for parameter in &abi.parameters {
            if !matches!(parameter.scalar_type, semantic_vocabulary::ScalarType::Integer(integer)
                if crate::unit_scalar_call_custody::integer_shape(integer).is_some())
                || values
                    .insert(parameter.value, parameter.scalar_type)
                    .is_some()
            {
                return false;
            }
        }
    }
    for continuation in continuations {
        let mut destinations = std::collections::BTreeSet::new();
        if continuation.bindings.iter().any(|binding| {
            values.get(&binding.argument) != Some(&binding.scalar_type)
                || values.contains_key(&binding.parameter)
                || !destinations.insert(binding.parameter)
        }) {
            return false;
        }
        for binding in &continuation.bindings {
            values.insert(binding.parameter, binding.scalar_type);
        }
    }
    true
}
