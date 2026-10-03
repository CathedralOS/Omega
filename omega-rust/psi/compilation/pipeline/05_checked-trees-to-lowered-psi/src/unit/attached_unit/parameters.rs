//! Parameter, transfer, and structural-argument lowering for attached Unit
//! closures.

use super::super::{
    BTreeSet, ByteSequenceCarrier, CheckedUnitEntryClaimPlan, CheckedUnitStructuralPathSegment,
    ClaimId, PermissionClaimIdentity, ServiceId, ServiceReachId, ServiceReachInterface,
    ServiceReachPlan, StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
    StructuralTypeDeclaration,
};
use super::{
    CheckedBoundaryMachinePlan, CheckedTrees, ClaimTransfer, LoweringError, Multiplicity, PlaceId,
    ScalarType, ServiceReachSummary, StructuralMultiplicity, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeId, StructuralTypeShape, ValueDeclaration, allocate_dense,
    lookup_claim_id, lookup_service_id, lookup_type_id, place_id, require_valid_service_row,
    terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::{
    LoweredDirectExpression, emit_direct_expression,
};
use crate::expression_preparation::bindings::StructuralScalarFieldBinding;
use crate::expression_preparation::bindings::structural_paths::{
    lower_structural_path, lower_typed_argument_path, terminal_access,
};
use crate::terminal_identities::{StructuralDomainBinding, lookup_domain_id_for_carrier};
use semantic_vocabulary::ValueId;
#[cfg(test)]
mod borrowed_service_tests;
mod reference_sources;
pub(crate) use reference_sources::lower_reference_source;
mod service_forward;
pub(in crate::unit::attached_unit) use service_forward::validate_parameter as validate_service_parameter;
mod source_path;
pub(crate) use source_path::{
    expression_producer, source_path, source_place_path, statement_receiver_path,
};

/// Qualifications have their own retained row; carrier comparisons preserve
/// reference presentation rather than silently turning a view into ownership.
pub(crate) fn structural_carrier_type(
    checked: &CheckedTrees,
    mut reference: checked_trees::types::TypeReferenceHandle,
) -> Result<checked_trees::types::TypeReferenceHandle, LoweringError> {
    let mut visited = Vec::new();
    loop {
        if !reference.is_valid() || visited.contains(&reference) {
            return unsupported("structural carrier has an invalid type chain");
        }
        visited.push(reference);
        match checked.type_reference_table.type_reference(reference) {
            checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                reference = *base_type
            }
            _ => return Ok(reference),
        }
    }
}

/// Rejoin direct and composed Unit scalar parameters and routed-Service receipts to the
/// exact typed state signature before raw checked-to-Terminal lowering erases
/// their authored source partition. Owner-selected Fused provenance is
/// validated by the compiler's later custody gate; this layer independently
/// rejects missing, fabricated, or source-substituted checked custody.
pub(crate) fn validate_direct_unit_parameter_custody(
    checked: &CheckedTrees,
) -> Result<(), LoweringError> {
    let has_receipt = |parameters: &[checked_trees::CheckedUnitStructuralParameterPlan]| {
        parameters
            .iter()
            .any(|parameter| parameter.fused_service_erasure.is_some())
    };
    let flow = &checked.facts.flow;
    let unsupported_receipt = flow
        .terminal_unit_effects
        .boundary_machines
        .iter()
        .any(|plan| has_receipt(&plan.structural_parameters))
        || flow
            .terminal_unit_effects
            .composed_machines
            .iter()
            .filter(|machine| {
                !matches!(
                    machine.result,
                    checked_trees::CheckedControlResultPlan::Unit
                )
            })
            .flat_map(|machine| &machine.states)
            .any(|state| has_receipt(&state.structural_parameters))
        || flow
            .terminal_partial_affine_unit_cleanups
            .machines
            .iter()
            .any(|plan| has_receipt(&plan.machine.structural_parameters))
        || flow
            .terminal_nominal_affine_unit_cleanups
            .machines
            .iter()
            .any(|plan| has_receipt(&plan.machine.structural_parameters))
        || flow
            .terminal_structural_unit_controls
            .machines
            .iter()
            .flat_map(|machine| &machine.states)
            .any(|state| has_receipt(&state.structural_parameters))
        || flow
            .terminal_structural_scalar_returns
            .machines
            .iter()
            .any(|machine| has_receipt(&machine.structural_parameters))
        || flow
            .terminal_structural_scalar_returns
            .trait_operator_machines
            .iter()
            .any(|machine| has_receipt(&machine.structural_parameters))
        || flow
            .terminal_boundary_scalar_returns
            .boundary_machines
            .iter()
            .any(|machine| has_receipt(&machine.structural_parameters))
        || flow
            .terminal_boundary_scalar_returns
            .machines
            .iter()
            .any(|machine| has_receipt(&machine.structural_parameters))
        || flow
            .terminal_structural_returns
            .machines
            .iter()
            .any(|machine| has_receipt(&machine.structural_parameters));
    if unsupported_receipt {
        return unsupported(
            "fused Service parameter receipt appears outside supported Unit parameter custody",
        );
    }

    // The same authored state signature owns each parameter, whether execution
    // uses one block or a composed state graph. A loan keeps the full reference
    // identity in its receipt and the underlying Binding identity in its carrier.
    let direct = flow.terminal_unit_effects.machines.iter().map(|plan| {
        (
            true,
            plan.machine,
            plan.state,
            plan.scalar_parameters.as_slice(),
            plan.structural_parameters.as_slice(),
        )
    });
    let composed = flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .filter(|machine| {
            matches!(
                machine.result,
                checked_trees::CheckedControlResultPlan::Unit
            )
        })
        .flat_map(|machine| {
            machine.states.iter().map(move |state| {
                (
                    false,
                    machine.machine,
                    state.state,
                    state.scalar_parameters.as_slice(),
                    state.structural_parameters.as_slice(),
                )
            })
        });
    for (direct, machine_symbol, state_symbol, scalar_parameters, structural_parameters) in
        direct.chain(composed)
    {
        let carries_receipt = has_receipt(structural_parameters);
        let carries_parameter_custody = carries_receipt || !scalar_parameters.is_empty();
        let Some(machine) = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)
        else {
            if carries_parameter_custody {
                return unsupported("direct Unit parameter plan has no exact typed machine");
            }
            continue;
        };
        // A composed plan's fused states belong to the machines their
        // transition arms reach, not the dispatch machine — resolve the state
        // under its own owner when the plan machine's roster does not name it.
        let Some(state) = checked
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_symbol)
            .or_else(|| {
                checked.machines().iter().find_map(|owner| {
                    checked
                        .machine_states(owner)
                        .iter()
                        .find(|state| state.symbol == state_symbol)
                })
            })
        else {
            if carries_parameter_custody {
                return unsupported("direct Unit parameter plan has no exact typed state");
            }
            continue;
        };
        let source_parameters = checked.state_parameters(state);
        let source_carriers = source_parameters
            .iter()
            .map(|source| {
                checked
                    .bound_service_parameter_carrier(source.type_reference)
                    .map_err(|_| {
                        LoweringError::Unsupported(
                            "typed Unit parameter has an invalid routed Binding carrier",
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        // This new composed-state join concerns routed Binding custody only.
        // Source carriers still trigger replay when a producer drops receipts;
        // unrelated composed signatures retain their existing validators.
        if !direct && !carries_receipt && source_carriers.iter().all(Option::is_none) {
            continue;
        }
        if structural_parameters.iter().any(|parameter| {
            parameter.fused_service_erasure.is_some()
                && usize::try_from(parameter.position)
                    .ok()
                    .is_none_or(|position| position >= source_parameters.len())
        }) {
            return unsupported("fused Service receipt has no authored parameter position");
        }
        let expected_scalar_parameters = checked_scalar_source_parameters(checked, state)?;
        if scalar_parameters != expected_scalar_parameters {
            return unsupported(
                "direct Unit scalar parameters do not rejoin the exact typed source partition",
            );
        }
        for (position, (source, carrier)) in
            source_parameters.iter().zip(source_carriers).enumerate()
        {
            let matches = structural_parameters
                .iter()
                .filter(|parameter| usize::try_from(parameter.position).ok() == Some(position))
                .collect::<Vec<_>>();
            let checked_parameter = match matches.as_slice() {
                [parameter] => Some(*parameter),
                [] => None,
                _ => {
                    return unsupported(
                        "Unit structural parameters duplicate an authored source position",
                    );
                }
            };
            let Some(carrier) = carrier else {
                if checked_parameter
                    .is_some_and(|parameter| parameter.fused_service_erasure.is_some())
                {
                    return unsupported(
                        "ordinary Unit parameter fabricates a fused Service erasure receipt",
                    );
                }
                continue;
            };
            let parameter = checked_parameter.ok_or(LoweringError::Unsupported(
                "typed routed Service parameter has no checked structural parameter",
            ))?;
            let receipt =
                parameter
                    .fused_service_erasure
                    .as_ref()
                    .ok_or(LoweringError::Unsupported(
                        "typed routed Service parameter lost its Fused erasure receipt",
                    ))?;
            if source.is_self
                || source.is_const
                || source.is_mutable
                    != (carrier.access == checked_trees::CheckedStructuralAccess::MutableBorrow)
                || parameter.is_self
                || source.relevance.is_erased()
                || parameter.multiplicity != service_parameter_multiplicity(carrier.access)?
                || parameter.access != carrier.access
                || !parameter.projected_qualifications.is_empty()
                || receipt.source_parameter != source.symbol
                || receipt.requirement != carrier.requirement
                || receipt.carrier_type_identity != carrier.carrier_type_identity
            {
                return unsupported(
                    "fused Service parameter receipt does not rejoin its exact typed source custody",
                );
            }
            if checked.fused_service_erasure(carrier.requirement).is_none() {
                return unsupported(
                    "fused Service parameter lacks compiler-owned erasure authorization",
                );
            }
            let base_identity = carrier.base_type_identity;
            let mut qualifications = carrier.qualifications;
            qualifications.sort_by_key(|domain| domain.0);
            qualifications.dedup();
            if parameter.type_identity != base_identity
                || parameter.qualifications != qualifications
            {
                return unsupported(
                    "fused Service parameter substituted its base type or qualifications",
                );
            }
        }
    }
    service_forward::validate(checked)?;
    Ok(())
}

/// A borrowed Binding is an ordinary reference parameter, not an owned affine
/// binding. Erasure changes storage, never the source access or multiplicity.
fn service_parameter_multiplicity(
    access: checked_trees::CheckedStructuralAccess,
) -> Result<Multiplicity, LoweringError> {
    match access {
        checked_trees::CheckedStructuralAccess::Owned => Ok(Multiplicity::Affine),
        checked_trees::CheckedStructuralAccess::SharedBorrow
        | checked_trees::CheckedStructuralAccess::MutableBorrow => Ok(Multiplicity::Unrestricted),
        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow => {
            unsupported("fused Service parameter cannot use write-only access")
        }
    }
}

pub(crate) fn checked_scalar_source_parameters(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
) -> Result<Vec<checked_trees::CheckedStructuralScalarParameterPlan>, LoweringError> {
    checked
        .state_parameters(state)
        .iter()
        .enumerate()
        // An `[erased]` binding occurrence owns no scalar position
        // (contracts.md#explicit-erased-bindings); the retained ones keep
        // their authored `position`, decided from the typed relevance here
        // rather than from the plan being validated.
        .filter(|(_, source)| !source.relevance.is_erased())
        .filter_map(|(position, source)| {
            checked
                .primitive_type_reference(source.type_reference)
                .map(|primitive_type| (position, source, primitive_type))
        })
        .map(|(position, source, primitive_type)| {
            // A mutable scalar still arrives as a value of this exact type;
            // the state body separately establishes its current-value slot.
            if source.is_self || source.is_const {
                return unsupported("scalar parameter is not a direct value binding");
            }
            Ok(checked_trees::CheckedStructuralScalarParameterPlan {
                source_position: u32::try_from(position).map_err(|_| {
                    LoweringError::Unsupported("scalar source position exceeds u32")
                })?,
                primitive_type,
            })
        })
        .collect()
}

/// Merge retained formals without turning lane-local proof/custody coordinates
/// into ABI positions. Gaps may denote omitted receivers or erased formals.
pub(crate) fn lower_boundary_parameter_order(
    scalars: &[checked_trees::CheckedStructuralScalarParameterPlan],
    structural: &[checked_trees::CheckedUnitStructuralParameterPlan],
) -> Result<Vec<terminal_psi::BoundaryParameterKind>, LoweringError> {
    use terminal_psi::BoundaryParameterKind::{Scalar, Structural};
    if scalars
        .windows(2)
        .any(|pair| pair[0].source_position >= pair[1].source_position)
        || structural
            .windows(2)
            .any(|pair| pair[0].position >= pair[1].position)
    {
        return unsupported("boundary parameter lanes are not in strict source order");
    }
    let mut scalar = scalars.iter().peekable();
    let mut structure = structural.iter().peekable();
    let mut order = Vec::with_capacity(scalars.len() + structural.len());
    loop {
        let kind = match (scalar.peek(), structure.peek()) {
            (Some(left), Some(right)) => match left.source_position.cmp(&right.position) {
                std::cmp::Ordering::Less => Scalar,
                std::cmp::Ordering::Greater => Structural,
                std::cmp::Ordering::Equal => {
                    return unsupported("boundary parameter lanes share a source position");
                }
            },
            (Some(_), None) => Scalar,
            (None, Some(_)) => Structural,
            (None, None) => break,
        };
        match kind {
            Scalar => {
                scalar.next();
            }
            Structural => {
                structure.next();
            }
        }
        order.push(kind);
    }
    Ok(order)
}

pub(crate) fn lower_unit_scalar_parameter_types(
    parameters: &[checked_trees::CheckedStructuralScalarParameterPlan],
) -> Result<Vec<ScalarType>, LoweringError> {
    if parameters
        .windows(2)
        .any(|pair| pair[0].source_position >= pair[1].source_position)
    {
        return unsupported("Unit scalar parameters are not in strict source order");
    }
    parameters
        .iter()
        .map(|parameter| terminal_scalar_type(parameter.primitive_type))
        .collect()
}
pub(crate) fn lower_unit_parameters(
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    type_ids: &[(String, StructuralTypeId)],
    structural_types: &[StructuralTypeDeclaration],
    domain_ids: &[StructuralDomainBinding],
    next_place: &mut u64,
) -> Result<Vec<StructuralParameterDeclaration>, LoweringError> {
    let mut positions = BTreeSet::new();
    parameters
        .iter()
        .enumerate()
        .map(|(dense_position, parameter)| {
            if !positions.insert(parameter.position) {
                return Err(LoweringError::Unsupported(
                    "Unit structural parameters contain duplicate source positions",
                ));
            }
            let mut qualifications = parameter
                .qualifications
                .iter()
                .map(|domain| {
                    lookup_domain_id_for_carrier(
                        domain_ids,
                        *domain,
                        lookup_type_id(type_ids, &parameter.type_identity)?,
                    )
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            qualifications.sort();
            qualifications.dedup();
            if qualifications.len() != parameter.qualifications.len() {
                return Err(LoweringError::Unsupported(
                    "Unit structural parameter repeats a qualification",
                ));
            }
            if parameter.fused_service_erasure.is_some()
                && (parameter.is_self
                    || parameter.multiplicity != service_parameter_multiplicity(parameter.access)?
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty())
            {
                return Err(LoweringError::Unsupported(
                    "fused Service parameter has an invalid checked structural shape",
                ));
            }
            Ok(StructuralParameterDeclaration {
                place: place_id(allocate_dense(next_place)?),
                position: u32::try_from(dense_position).map_err(|_| {
                    LoweringError::Unsupported("Unit structural parameter count exceeds u32")
                })?,
                is_self: parameter.is_self,
                structural_type: lookup_type_id(type_ids, &parameter.type_identity)?,
                multiplicity: match parameter.multiplicity {
                    Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                    Multiplicity::Affine => StructuralMultiplicity::Affine,
                    Multiplicity::Linear => StructuralMultiplicity::Linear,
                },
                access: terminal_access(parameter.access),
                qualifications,
                projected_qualifications: lower_projected_qualifications(
                    &parameter.projected_qualifications,
                    domain_ids,
                    lookup_type_id(type_ids, &parameter.type_identity)?,
                    structural_types,
                )?,
            })
        })
        .collect()
}

pub(crate) fn lower_contract_service_ceiling(
    rows: &language_semantics::ServiceReachRowTable,
    contract: ServiceReachPlan,
    summary: ServiceReachSummary,
    service_ids: &[(ServiceReachId, ServiceId)],
) -> Result<Vec<ServiceId>, LoweringError> {
    if contract.checked_inferred != summary.transitive {
        return unsupported("Unit contract reach does not match checked transitive reach");
    }
    let source = match contract.interface {
        ServiceReachInterface::PublishedCeiling(row) => {
            require_valid_service_row(row)?;
            let ceiling = rows.services(row);
            if rows
                .services(summary.transitive)
                .iter()
                .any(|service| !ceiling.contains(service))
            {
                return unsupported("checked Unit service reach exceeds its published ceiling");
            }
            ceiling
        }
        ServiceReachInterface::InternalInferred => rows.services(summary.transitive),
    };
    let mut lowered = source
        .iter()
        .map(|service| lookup_service_id(service_ids, *service))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    lowered.sort();
    lowered.dedup();
    if lowered.len() != source.len() {
        return unsupported("Unit published service ceiling contains duplicates");
    }
    Ok(lowered)
}

/// Keep the authored concrete basis, not the flattened inferred ceiling. A
/// Console declaration and a Console installation upper bound may overlap;
/// subtracting the bound would erase the declaration. Terminal independently
/// unions this basis with retained calls to reconstruct the concrete root row.
pub(crate) fn lower_declared_service_reach(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    service_ids: &[(ServiceReachId, ServiceId)],
) -> Result<Vec<ServiceId>, LoweringError> {
    let mut sources = checked
        .typed
        .machines()
        .iter()
        .filter(|source| source.symbol == machine);
    let source = sources.next().ok_or(LoweringError::Unsupported(
        "service declaration lost its source machine",
    ))?;
    if sources.next().is_some() {
        return unsupported("service declaration has ambiguous source machines");
    }
    if source.service_reach_is_installation_bound {
        return Ok(Vec::new());
    }
    let mut declared = checked
        .typed
        .service_reach_rows
        .services(source.service_reach_row)
        .iter()
        .map(|service| lookup_service_id(service_ids, *service))
        .collect::<Result<Vec<_>, _>>()?;
    declared.sort();
    declared.dedup();
    Ok(declared)
}

pub(crate) fn lower_fixed_boundary_service_reach(
    checked: &CheckedTrees,
    boundary: &CheckedBoundaryMachinePlan,
    service_ids: &[(ServiceReachId, ServiceId)],
) -> Result<Vec<ServiceId>, LoweringError> {
    let source =
        validation::fixed_installation_boundary_service_reach(&checked.typed, boundary.state)
            .ok_or(LoweringError::Unsupported(
                "fixed boundary reach has no exact typed requirement",
            ))?;
    let mut fixed = source
        .iter()
        .map(|service| lookup_service_id(service_ids, *service))
        .collect::<Result<Vec<_>, _>>()?;
    fixed.sort();
    fixed.dedup();
    Ok(fixed)
}

pub(crate) fn lower_published_service_ceiling(
    rows: &language_semantics::ServiceReachRowTable,
    contract: ServiceReachPlan,
    summary: ServiceReachSummary,
    service_ids: &[(ServiceReachId, ServiceId)],
) -> Result<Vec<ServiceId>, LoweringError> {
    if matches!(contract.interface, ServiceReachInterface::InternalInferred) {
        return unsupported("public Unit contract has no published service ceiling");
    }
    lower_contract_service_ceiling(rows, contract, summary, service_ids)
}

pub(crate) fn lower_installation_machine_service_ceiling(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    contract: ServiceReachPlan,
    summary: ServiceReachSummary,
    service_ids: &[(ServiceReachId, ServiceId)],
) -> Result<Vec<ServiceId>, LoweringError> {
    if let Some(reach) = checked.facts.service_reaches.for_machine(machine)
        && matches!(contract.interface, ServiceReachInterface::InternalInferred)
        && !reach.unresolved_installation_reaches.is_empty()
    {
        if contract.checked_inferred != summary.transitive
            || reach.inferred_transitive != summary.transitive
        {
            return unsupported(
                "installation-bound machine reach disagrees with its checked transitive row",
            );
        }
        let source = checked
            .facts
            .service_reaches
            .rows
            .services(summary.transitive);
        let mut lowered = source
            .iter()
            .map(|service| lookup_service_id(service_ids, *service))
            .collect::<Result<Vec<_>, LoweringError>>()?;
        lowered.sort();
        lowered.dedup();
        if lowered.len() != source.len() {
            return unsupported("installation-bound service ceiling contains duplicates");
        }
        return Ok(lowered);
    }
    lower_contract_service_ceiling(
        &checked.facts.service_reaches.rows,
        contract,
        summary,
        service_ids,
    )
}

/// The operation-owned result frontier, in the current source state's binding
/// namespace. Declaration-only callers cannot admit claim-bearing results.
pub(crate) struct StructuralResultCustody<'a> {
    pub results: &'a [(
        u32,
        crate::emission::operation_emission::buffer::StructuralBinding,
    )],
    pub domains: &'a [StructuralDomainBinding],
    pub claims: &'a [(PermissionClaimIdentity, ClaimId)],
    pub target_entry_claims: &'a [CheckedUnitEntryClaimPlan],
}

pub(crate) fn validate_transfer_shape(
    arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
    caller_parameters: &[StructuralParameterDeclaration],
    caller_trivial_affine_locals: &[StructuralPlaceDeclaration],
    caller_structural_results: &[(StructuralPlaceDeclaration, bool)],
    target_parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    type_ids: &[(String, StructuralTypeId)],
    structural_types: &[StructuralTypeDeclaration],
    expected_claim_arguments: &[u32],
    primitive_locals: &[super::primitive_locals::PrimitiveLocal],
    result_custody: Option<StructuralResultCustody<'_>>,
) -> Result<(), LoweringError> {
    if arguments.len() != target_parameters.len() {
        return unsupported(
            "Unit call structural argument arity does not match its checked target",
        );
    }
    for (argument_index, (argument, target)) in arguments.iter().zip(target_parameters).enumerate()
    {
        if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } =
            argument.source
        {
            let local = super::primitive_locals::find(primitive_locals, symbol)?;
            let expected_type = lookup_type_id(type_ids, &argument.type_identity)?;
            if !argument.path.is_empty()
                || argument.type_identity != target.type_identity
                || !matches!(local.declaration.kind, StructuralPlaceKind::OperationResult { structural_type, .. }
                    if structural_type == expected_type)
                || argument.access == checked_trees::CheckedStructuralAccess::Owned
                || argument.access != target.access
                || target.multiplicity != Multiplicity::Unrestricted
                || !target.qualifications.is_empty()
                || target.fused_service_erasure.is_some()
                || !matches!(structural_types.iter().find(|declaration| declaration.id == expected_type).map(|declaration| &declaration.shape),
                    Some(StructuralTypeShape::PrimitiveScalar(scalar_type)) if *scalar_type == local.scalar_type)
            {
                return unsupported("primitive local borrow has invalid target custody");
            }
            continue;
        }
        let range = super::byte_subslices::range(argument);
        if argument.byte_sequence_literal().is_some() || range.is_some() {
            // A literal may construct a fresh bounded owner. A range instead
            // lends its exact backing with the requested borrowing access;
            // descriptor custody cannot manufacture owned storage. Source and
            // storage replay retain the formal and collection projection, and
            // Terminal independently checks access and qualifications.
            let owned_literal = argument.byte_sequence_literal().is_some()
                && argument.access == checked_trees::CheckedStructuralAccess::Owned;
            let access_allowed = if range.is_some() {
                matches!(
                    argument.access,
                    checked_trees::CheckedStructuralAccess::SharedBorrow
                        | checked_trees::CheckedStructuralAccess::MutableBorrow
                )
            } else {
                matches!(
                    argument.access,
                    checked_trees::CheckedStructuralAccess::SharedBorrow
                        | checked_trees::CheckedStructuralAccess::Owned
                )
            };
            let element_range = range.as_ref().is_some_and(|range| {
                range.family
                    == crate::emission::operation_emission::view_subslice::ViewFamily::Elements
            });
            if (!argument.path.is_empty() && range.is_none())
                || argument.type_identity != target.type_identity
                || !access_allowed
                || argument.access != target.access
                || target.multiplicity != Multiplicity::Unrestricted
            {
                return unsupported("byte-sequence argument has invalid checked custody");
            }
            let structural_type = lookup_type_id(type_ids, &argument.type_identity)?;
            if !structural_types.iter().any(|declaration| {
                declaration.id == structural_type
                    && match declaration.shape {
                        StructuralTypeShape::ElementView { .. } => element_range,
                        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                            ..
                        }) => !owned_literal && !element_range,
                        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                            capacity,
                        }) => {
                            owned_literal
                                && argument.byte_sequence_literal().is_some_and(|bytes| {
                                    u64::try_from(bytes.len())
                                        .is_ok_and(|length| length <= capacity)
                                })
                        }
                        _ => false,
                    }
            }) {
                return unsupported("byte-sequence argument access differs from its carrier");
            }
            continue;
        }
        if let Some(declaration_ordinal) = argument.source_local_declaration_ordinal() {
            let source = caller_trivial_affine_locals
                .get(usize::try_from(declaration_ordinal).map_err(|_| {
                    LoweringError::Unsupported("Unit local argument ordinal exceeds usize")
                })?)
                .ok_or(LoweringError::Unsupported(
                    "Unit local argument has an invalid declaration ordinal",
                ))?;
            let StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal: source_ordinal,
                structural_type,
                ..
            } = source.kind
            else {
                return unsupported("Unit local argument does not name a trivial affine local");
            };
            if source_ordinal != declaration_ordinal
                || !argument.path.is_empty()
                || argument.type_identity != target.type_identity
                || structural_type != lookup_type_id(type_ids, &argument.type_identity)?
                || argument.access != checked_trees::CheckedStructuralAccess::Owned
                || target.access != checked_trees::CheckedStructuralAccess::Owned
                || target.multiplicity != Multiplicity::Affine
                || !target.qualifications.is_empty()
            {
                return unsupported("Unit local argument has invalid checked custody");
            }
            continue;
        }
        if let Some(binding_ordinal) = argument.source_structural_result_binding_ordinal() {
            let source =
                structural_result_source(caller_structural_results, binding_ordinal, argument)?;
            if let StructuralPlaceKind::ByteSequenceLiteral {
                structural_type, ..
            } = source.kind
            {
                if argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow
                    || target.access != argument.access
                    || !argument.path.is_empty()
                    || target.multiplicity != Multiplicity::Unrestricted
                    || argument.type_identity != target.type_identity
                    || structural_type != lookup_type_id(type_ids, &argument.type_identity)?
                    || !target.qualifications.is_empty()
                    || !target.projected_qualifications.is_empty()
                    || target.fused_service_erasure.is_some()
                    || target.is_self
                    || !structural_types.iter().any(|declaration| {
                        declaration.id == structural_type
                            && matches!(
                                declaration.shape,
                                StructuralTypeShape::ByteSequence(
                                    ByteSequenceCarrier::BorrowedView { .. }
                                )
                            )
                    })
                    || transfers.iter().any(|transfer| {
                        arguments
                            .get(transfer.argument_index as usize)
                            .is_some_and(|argument| {
                                argument.source_structural_result_binding_ordinal()
                                    == Some(binding_ordinal)
                            })
                    })
                {
                    return unsupported("literal binding argument changed its shared byte custody");
                }
                continue;
            }
            let StructuralPlaceKind::OperationResult {
                structural_type, ..
            } = source.kind
            else {
                return unsupported("Unit structural result source has no producer operation");
            };
            if target.multiplicity == Multiplicity::Linear {
                let custody = result_custody.as_ref().ok_or(LoweringError::Unsupported(
                    "linear result argument has no completed operation custody",
                ))?;
                let mut results = custody
                    .results
                    .iter()
                    .filter(|(ordinal, _)| *ordinal == binding_ordinal);
                let (_, result) = results.next().ok_or(LoweringError::Unsupported(
                    "linear result argument has no exact completed binding",
                ))?;
                let result = result.operation_result()?;
                let qualifications = target
                    .qualifications
                    .iter()
                    .map(|domain| {
                        lookup_domain_id_for_carrier(custody.domains, *domain, structural_type)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let claims = transfers
                    .iter()
                    .filter(|transfer| transfer.argument_index as usize == argument_index)
                    .map(|transfer| lookup_claim_id(custody.claims, transfer.claim_identity))
                    .collect::<Result<Vec<_>, _>>()?;
                let expected = custody
                    .target_entry_claims
                    .iter()
                    .filter(|claim| claim.parameter_index as usize == argument_index)
                    .collect::<Vec<_>>();
                let projected_qualifications = lower_projected_qualifications(
                    &target.projected_qualifications,
                    custody.domains,
                    structural_type,
                    structural_types,
                )?;
                // A completed whole result carries the exact target claim set,
                // with source identities rebased but relative paths unchanged.
                // Counts alone cannot distinguish missing or swapped siblings.
                // A boundary target has no entry-claim roster, so its exact
                // set is the completed result's own claim frontier.
                // A claim-carrying `self` formal publishes no transfer row at
                // all: the result's own claim frontier is the custody this
                // call consumes. The minted caller binding is the evidence —
                // the producing call minted it only for a claim established
                // at the result's own binding statement, so the carried claim
                // must resolve through the grown table to a statement-
                // established identity, and the formal's entry roster must be
                // exactly the whole-value claim that consume joins. Anything
                // else is a claim-free move wearing receiver custody, not a
                // completed operation transfer.
                let claims_match = if target.is_self {
                    claims.is_empty()
                        && expected.len() == 1
                        && result.claims.len() == 1
                        && expected.iter().zip(&result.claims).all(|(expected, binding)| {
                            lower_structural_path(&expected.path).is_ok_and(|path| binding.path == path)
                                && custody.claims.iter().any(|(identity, claim)| {
                                    *claim == binding.claim
                                        && matches!(
                                            identity,
                                            PermissionClaimIdentity::Established {
                                                source:
                                                    language_semantics::PermissionEventSource::Statement {
                                                        ..
                                                    },
                                                ..
                                            }
                                        )
                                })
                        })
                } else {
                    !claims.is_empty()
                        && result.claims.len() == claims.len()
                        && if expected.is_empty() {
                            let mut transferred = claims.clone();
                            transferred.sort_unstable();
                            let mut completed = result
                                .claims
                                .iter()
                                .map(|binding| binding.claim)
                                .collect::<Vec<_>>();
                            completed.sort_unstable();
                            transferred == completed
                        } else {
                            claims.len() == expected.len()
                                && claims.iter().zip(&expected).all(|(claim, expected)| {
                                    result
                                        .claims
                                        .iter()
                                        .filter(|result| {
                                            result.claim == *claim
                                                && lower_structural_path(&expected.path)
                                                    .is_ok_and(|path| result.path == path)
                                        })
                                        .count()
                                        == 1
                                })
                        }
                };
                if results.next().is_some()
                    || result.place != source.id
                    || result.structural_type != structural_type
                    || structural_type != lookup_type_id(type_ids, &argument.type_identity)?
                    || argument.type_identity != target.type_identity
                    || result.multiplicity != terminal_psi::StructuralMultiplicity::Linear
                    || !argument.path.is_empty()
                    || argument.access != checked_trees::CheckedStructuralAccess::Owned
                    || target.access != argument.access
                    || target.fused_service_erasure.is_some()
                    || result.qualifications != qualifications
                    || result.projected_qualifications != projected_qualifications
                    || !claims_match
                    || arguments[..argument_index].iter().any(|earlier| {
                        earlier.source_structural_result_binding_ordinal() == Some(binding_ordinal)
                    })
                {
                    return unsupported(
                        "linear result argument differs from its completed claim frontier",
                    );
                }
                continue;
            }
            if argument.access != checked_trees::CheckedStructuralAccess::Owned {
                // Source replay already rejoins this live result to its producer
                // and captured loan. Its owned home supplies the same projection
                // operation as a parameter; this row grants no new loan or claim.
                let storage = StructuralParameterDeclaration {
                    place: source.id,
                    position: target.position,
                    is_self: false,
                    structural_type,
                    access: StructuralAccess::Owned,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                };
                validate_borrowed_argument(&storage, argument, target, type_ids, structural_types)?;
            } else {
                // Ownership transports a value, whereas a loan leaves its home
                // in the caller. Keep the whole-value copy/affine distinction
                // and the linear claim frontier above independent of projection.
                let unrestricted_whole = argument.path.is_empty()
                    && target.multiplicity == Multiplicity::Unrestricted
                    && structural_types.iter().any(|declaration| {
                        declaration.id == structural_type
                            && match declaration.shape {
                                StructuralTypeShape::FixedArray { .. }
                                | StructuralTypeShape::Record { .. }
                                | StructuralTypeShape::Sum { .. } => true,
                                StructuralTypeShape::ByteSequence(
                                    ByteSequenceCarrier::BoundedOwned { .. },
                                ) => !target.is_self && target.projected_qualifications.is_empty(),
                                _ => false,
                            }
                    });
                if argument.type_identity != target.type_identity
                    || (argument.path.is_empty()
                        && structural_type != lookup_type_id(type_ids, &argument.type_identity)?)
                    || target.access != argument.access
                    || target.multiplicity
                        != if unrestricted_whole {
                            Multiplicity::Unrestricted
                        } else {
                            Multiplicity::Affine
                        }
                    || (target.is_self && !argument.path.is_empty())
                    || target.fused_service_erasure.is_some()
                {
                    return unsupported("owned result argument has invalid checked custody");
                }
            }
            if transfers.iter().any(|transfer| {
                arguments
                    .get(transfer.argument_index as usize)
                    .is_some_and(|argument| {
                        argument.source_structural_result_binding_ordinal() == Some(binding_ordinal)
                    })
            }) {
                return unsupported("borrowed or affine result argument cannot transfer a claim");
            }
            continue;
        }
        let source_parameter_index =
            argument
                .source_parameter_index()
                .ok_or(LoweringError::Unsupported(
                    "Unit structural argument is neither a parameter nor a supported literal",
                ))?;
        let source = caller_parameters
            .get(usize::try_from(source_parameter_index).map_err(|_| {
                LoweringError::Unsupported("Unit structural argument index exceeds usize")
            })?)
            .ok_or(LoweringError::Unsupported(
                "Unit structural argument has an invalid caller parameter index",
            ))?;
        if argument.access != checked_trees::CheckedStructuralAccess::Owned {
            validate_borrowed_argument(source, argument, target, type_ids, structural_types)?;
            continue;
        }
        if argument.type_identity != target.type_identity
            || (argument.path.is_empty()
                && source.structural_type != lookup_type_id(type_ids, &argument.type_identity)?)
        {
            return unsupported("Unit structural argument type identity is inconsistent");
        }
        let source_access = match source.access {
            StructuralAccess::Owned => checked_trees::CheckedStructuralAccess::Owned,
            StructuralAccess::SharedBorrow => checked_trees::CheckedStructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow => {
                checked_trees::CheckedStructuralAccess::MutableBorrow
            }
            StructuralAccess::WriteOnlyBorrow => {
                checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
            }
        };
        if argument.access != target.access
            || !checked_access_can_supply(source_access, argument.access)
        {
            return unsupported("Unit structural argument access is inconsistent");
        }
    }
    let actual = transfers
        .iter()
        .map(|transfer| transfer.argument_index)
        .collect::<Vec<_>>();
    if actual != expected_claim_arguments
        || actual.iter().any(|index| {
            usize::try_from(*index)
                .ok()
                .is_none_or(|index| index >= arguments.len())
        })
    {
        return unsupported("Unit claim transfer does not exactly match target entry custody");
    }
    Ok(())
}

/// The call's emitted transfer roster: each checked row resolves through the
/// caller's claim table, and a claim-carrying `self` formal fed by a
/// completed result names the minted binding the call consumes. The checker
/// records that consume as a permission event rather than a transfer row —
/// the result's claim was established at its own binding statement, not
/// carried through an entry claim — so the emitted operation publishes the
/// result's carried claim directly. The verifier's roster joins it against
/// the result place's own frontier the same way it resolves every other
/// caller claim.
pub(crate) fn emitted_claim_transfers(
    arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
    target_parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    completed_results: &[(
        u32,
        crate::emission::operation_emission::buffer::StructuralBinding,
    )],
    claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
) -> Result<Vec<ClaimTransfer>, LoweringError> {
    let mut emitted = transfers
        .iter()
        .map(|transfer| {
            Ok(ClaimTransfer {
                claim: lookup_claim_id(claim_bindings, transfer.claim_identity)?,
                argument_index: transfer.argument_index,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    for (argument_index, (argument, target)) in arguments.iter().zip(target_parameters).enumerate()
    {
        let Some(binding_ordinal) = argument.source_structural_result_binding_ordinal() else {
            continue;
        };
        if !target.is_self || target.multiplicity != Multiplicity::Linear {
            continue;
        }
        // Transfer-shape validation already joined this argument: the
        // completed result carries exactly the formal's whole-value entry
        // claim, bound in the caller's table by the producing statement's
        // mint. The emitted transfer is that consume, not a new claim.
        let (_, result) = completed_results
            .iter()
            .find(|(ordinal, _)| *ordinal == binding_ordinal)
            .ok_or(LoweringError::Unsupported(
                "Unit self argument lost its completed result claim binding",
            ))?;
        let [binding] = result.operation_result()?.claims.as_slice() else {
            return unsupported("Unit self argument lost its completed result claim binding");
        };
        emitted.push(ClaimTransfer {
            claim: binding.claim,
            argument_index: u32::try_from(argument_index).map_err(|_| {
                LoweringError::Unsupported("Unit claim transfer argument index exceeds u32")
            })?,
        });
    }
    emitted.sort();
    Ok(emitted)
}

/// Resolve every borrowed operand against its established home, regardless of
/// whether a parameter or a completed operation introduced that home. The path
/// preparation used by emission supplies the exact endpoint and every explicit
/// reference or view crossing. This checks presentation, not establishment, live-loan
/// authority, runtime bounds, or qualification implication; those retain their
/// independent source and Terminal replay.
fn validate_borrowed_argument(
    source: &StructuralParameterDeclaration,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    target: &checked_trees::CheckedUnitStructuralParameterPlan,
    type_ids: &[(String, StructuralTypeId)],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<(), LoweringError> {
    use crate::expression_preparation::bindings::structural_paths::{
        PreparedArgumentTip, prepare_argument_projection,
    };
    let access = terminal_access(argument.access);
    if access == StructuralAccess::Owned
        || argument.type_identity != target.type_identity
        || argument.access != target.access
        || target.multiplicity != Multiplicity::Unrestricted
        || !structural_access_can_supply(source.access, access)
    {
        return unsupported("borrowed argument changed its checked access or target custody");
    }
    let projection =
        prepare_argument_projection(&argument.path, source.structural_type, structural_types)?;
    if projection
        .crossed_accesses
        .iter()
        .any(|reference| !structural_access_can_supply(*reference, access))
    {
        return unsupported("borrowed argument amplifies a reference or view crossing's access");
    }
    let target_type = lookup_type_id(type_ids, &target.type_identity)?;
    let mut declarations = structural_types
        .iter()
        .filter(|declaration| declaration.id == target_type);
    let expected = declarations.next().ok_or(LoweringError::Unsupported(
        "borrowed argument lost its target type",
    ))?;
    if declarations.next().is_some() {
        return unsupported("borrowed argument target type is ambiguous");
    }
    // Program-level parameter custody and service-forwarding replay already
    // authenticate the receipt and exact authored actual before lowering. A
    // borrowed Binding still uses this ordinary loan geometry: erased storage
    // is neither a reason to reject it nor permission to relabel another type.
    if target.fused_service_erasure.is_some()
        && expected.shape != StructuralTypeShape::FusedServiceBinding
    {
        return unsupported("borrowed service receipt requires its Fused binding carrier");
    }
    let exact = match &projection.tip {
        PreparedArgumentTip::Declaration(declaration) => declaration.id == expected.id,
        PreparedArgumentTip::Inline(field) => {
            field.canonical_leaf_shape().as_ref() == Some(&expected.shape)
        }
        PreparedArgumentTip::ByteElement => matches!(expected.shape,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                if integer == semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8).expect("u8")),
        PreparedArgumentTip::ByteWindow => false,
    };
    if exact {
        return Ok(());
    }
    // Inline byte storage has no standalone leaf identity. Its carrier is
    // presented in place, preserving the enclosing home and field predicates;
    // it must never be treated as an owned copy of a descriptor. Independent
    // source/Terminal replay checks those predicates against the formal.
    if let (PreparedArgumentTip::Inline(field), StructuralTypeShape::ByteSequence(expected)) =
        (&projection.tip, &expected.shape)
        && let Some(carrier) = field.byte_sequence_carrier()
        && !target.is_self
        && target.projected_qualifications.is_empty()
    {
        let compatible = match (carrier, *expected) {
            (
                ByteSequenceCarrier::BoundedOwned { capacity },
                ByteSequenceCarrier::BoundedOwned { capacity: expected },
            ) => capacity == expected,
            (
                ByteSequenceCarrier::BoundedOwned { .. },
                ByteSequenceCarrier::BorrowedView { access: embedded },
            ) => {
                matches!(
                    access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                ) && embedded.is_none_or(|embedded| structural_access_can_supply(embedded, access))
            }
            (
                ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::SharedBorrow),
                },
                ByteSequenceCarrier::BorrowedView {
                    access: None | Some(StructuralAccess::SharedBorrow),
                },
            ) => access == StructuralAccess::SharedBorrow,
            _ => false,
        };
        if compatible {
            return Ok(());
        }
    }
    // Adapting an array or byte owner to a view is a carrier operation, not
    // type equality. It keeps the original place and complete backing path.
    let element_view = matches!((&projection.tip, &expected.shape),
        (PreparedArgumentTip::Declaration(declaration), StructuralTypeShape::ElementView { element: expected_element })
            if matches!(declaration.shape, StructuralTypeShape::FixedArray { element, length }
                if length > 0 && element == *expected_element))
        && matches!(
            access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        )
        && !target.is_self
        && target.qualifications.is_empty()
        && target.projected_qualifications.is_empty();
    // A shared view of a bounded owner carries its live length and existing
    // predicates. Source/Terminal qualification replay proves the formal's
    // requirements; representation compatibility does not establish them.
    let shared_owner_view = matches!((&projection.tip, &expected.shape),
        (PreparedArgumentTip::Declaration(declaration), StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { access: None | Some(StructuralAccess::SharedBorrow) }))
            if matches!(declaration.shape, StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })))
        && access == StructuralAccess::SharedBorrow
        && !target.is_self
        && target.projected_qualifications.is_empty();
    if element_view
        || shared_owner_view
        || byte_view_transfer(source, argument, target, type_ids, structural_types)
    {
        return Ok(());
    }
    unsupported("borrowed argument presentation differs from its selected storage")
}

fn structural_access_can_supply(source: StructuralAccess, presented: StructuralAccess) -> bool {
    match source {
        StructuralAccess::Owned => true,
        StructuralAccess::SharedBorrow => presented == StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow => matches!(
            presented,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        ),
        StructuralAccess::WriteOnlyBorrow => presented == StructuralAccess::WriteOnlyBorrow,
    }
}

fn byte_view_transfer(
    source: &StructuralParameterDeclaration,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    target: &checked_trees::CheckedUnitStructuralParameterPlan,
    type_ids: &[(String, StructuralTypeId)],
    structural_types: &[StructuralTypeDeclaration],
) -> bool {
    if argument.access != target.access
        || target.multiplicity != Multiplicity::Unrestricted
        || !target.qualifications.is_empty()
        || !target.projected_qualifications.is_empty()
    {
        return false;
    }
    let access = match target.access {
        checked_trees::CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
        checked_trees::CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
        _ => return false,
    };
    let Ok(structural_type) = lookup_type_id(type_ids, &target.type_identity) else {
        return false;
    };
    let expected = StructuralParameterDeclaration {
        place: source.place,
        position: target.position,
        is_self: target.is_self,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let argument = StructuralArgument {
        place: source.place,
        path: match lower_structural_path(&argument.path) {
            Ok(path) => path,
            Err(_) => return false,
        },
        access,
    };
    terminal_semantics::fixed_byte_array_window(
        structural_types.iter(),
        source,
        &argument,
        &expected,
    )
    .is_some()
        // The bounded carrier keeps its runtime length and owned backing;
        // presenting a shared view does not create a fixed-size window.
        || terminal_semantics::shared_boundary_buffer_capacity(
            structural_types.iter(),
            source.structural_type,
            &argument,
            &expected,
        )
        .is_some()
}

fn checked_access_can_supply(
    source: checked_trees::CheckedStructuralAccess,
    presented: checked_trees::CheckedStructuralAccess,
) -> bool {
    structural_access_can_supply(terminal_access(source), terminal_access(presented))
}

pub(crate) fn literal_argument_places(
    arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    literals: &[StructuralPlaceDeclaration],
    next_literal: &mut usize,
) -> Result<Vec<PlaceId>, LoweringError> {
    let count = arguments
        .iter()
        .filter(|argument| argument.byte_sequence_literal().is_some())
        .count();
    let end = next_literal
        .checked_add(count)
        .ok_or(LoweringError::Unsupported(
            "byte-sequence literal argument count overflows usize",
        ))?;
    let places = literals
        .get(*next_literal..end)
        .ok_or(LoweringError::Unsupported(
            "byte-sequence literal argument place is absent",
        ))?
        .iter()
        .map(|place| place.id)
        .collect();
    *next_literal = end;
    Ok(places)
}

/// Emit one `IntegerStructuralField` read for every `StructuralField`
/// runtime-index selector in argument order, before the call they select
/// into. The call's `RuntimeIndex` segment then carries the read's value as
/// its operand — the same operand position a `Parameter` selector carries —
/// so the reads land on the operation buffer ahead of the call, which is why
/// they mint here rather than inside `lower_structural_arguments` (its
/// earlier-results slice holds the buffer by shared borrow).
pub(crate) fn mint_runtime_index_selectors(
    arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    fields: &[StructuralScalarFieldBinding],
    caller_scalar_values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<Vec<ValueId>, LoweringError> {
    let mut values = Vec::new();
    for argument in arguments {
        for segment in &argument.path {
            let CheckedUnitStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::StructuralField(field),
            ) = segment
            else {
                continue;
            };
            let (source, path, leaf, scalar_type) =
                crate::expression_preparation::bindings::structural_fields::resolve_typed(
                    fields,
                    field.parameter_position,
                    &field.path,
                )?;
            if !matches!(scalar_type, ScalarType::Integer(integer) if !integer.is_address()) {
                return unsupported("runtime index field selector is not an integer read");
            }
            values.push(emit_direct_expression(
                &LoweredDirectExpression::StructuralField {
                    source: source.into(),
                    path,
                    field: leaf,
                    scalar_type,
                },
                caller_scalar_values,
                next_value,
                operations,
            ));
        }
    }
    Ok(values)
}

/// A runtime-index argument rooted at an element view owes its bound the
/// view's live extent, not a declaration: mint the dominating
/// `ElementViewLength` observation whose scalar the segment's
/// `index < length` obligation names — the same observation an element-view
/// write carries as its `length` operand. Only a segment at path position
/// zero resolves the view at the projection root; one observation per root
/// place dominates every argument the call projects from that view.
pub(crate) fn mint_element_view_length_observations(
    arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    parameters: &[StructuralParameterDeclaration],
    structural_results: &[(StructuralPlaceDeclaration, bool)],
    structural_types: &[StructuralTypeDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    let mut observed = BTreeSet::new();
    for argument in arguments {
        if !matches!(
            argument.path.first(),
            Some(CheckedUnitStructuralPathSegment::RuntimeIndex(_))
        ) {
            continue;
        }
        let (place, structural_type) =
            if let Some(ordinal) = argument.source_structural_result_binding_ordinal() {
                let (source, _) = structural_results
                    .get(usize::try_from(ordinal).map_err(|_| {
                        LoweringError::Unsupported(
                            "Unit structural result binding ordinal exceeds usize",
                        )
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "Unit structural result binding is not produced",
                    ))?;
                let StructuralPlaceKind::OperationResult {
                    structural_type, ..
                } = source.kind
                else {
                    continue;
                };
                (source.id, structural_type)
            } else if let Some(parameter_index) = argument.source_parameter_index() {
                let parameter = parameters
                    .get(usize::try_from(parameter_index).map_err(|_| {
                        LoweringError::Unsupported("Unit structural argument index exceeds usize")
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "Unit structural argument has an invalid caller parameter index",
                    ))?;
                (parameter.place, parameter.structural_type)
            } else {
                continue;
            };
        if !structural_types.iter().any(|declaration| {
            declaration.id == structural_type
                && matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
        }) || !observed.insert(place)
        {
            continue;
        }
        let length = crate::terminal_identities::value_id(allocate_dense(next_value)?);
        let id = operations.allocate();
        operations.push(crate::unit::Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: crate::unit::OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: length,
                scalar_type: terminal_scalar_type(crate::unit::PrimitiveType::U64)?,
            }),
            kind: crate::unit::OperationKind::ElementViewLength { source: place },
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_structural_arguments(
    arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    structural_types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
    trivial_affine_locals: &[StructuralPlaceDeclaration],
    structural_results: &[(StructuralPlaceDeclaration, bool)],
    byte_argument_places: &[PlaceId],
    primitive_locals: &[super::primitive_locals::PrimitiveLocal],
    caller_scalar_values: Option<&[ValueDeclaration]>,
    selector_values: &mut dyn Iterator<Item = ValueId>,
    mut calls: Option<&mut CallEmissionContext<'_>>,
) -> Result<Vec<StructuralArgument>, LoweringError> {
    let mut next_byte_argument = 0usize;
    arguments
        .iter()
        .map(|argument| {
            if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } = argument.source {
                let local = super::primitive_locals::find(primitive_locals, symbol)?;
                if !argument.path.is_empty() { return unsupported("primitive local borrow has a projection"); }
                // Not `terminal_access`: a primitive local lends no owned custody.
                let access = match argument.access {
                    checked_trees::CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
                    checked_trees::CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
                    checked_trees::CheckedStructuralAccess::WriteOnlyBorrow => StructuralAccess::WriteOnlyBorrow,
                    checked_trees::CheckedStructuralAccess::Owned => return unsupported("primitive local cannot transfer owned custody"),
                };
                return Ok(StructuralArgument { place: local.declaration.id, path: Vec::new(), access });
            }
            // Literals and view ranges of either family were staged into
            // places of their own, in argument order.
            if argument.byte_sequence_literal().is_some()
                || matches!(argument.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. } | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. })
            {
                let place = *byte_argument_places
                    .get(next_byte_argument)
                    .ok_or(LoweringError::Unsupported(
                        "byte-sequence argument place is absent",
                    ))?;
                next_byte_argument += 1;
                return Ok(StructuralArgument {
                    place,
                    path: Vec::new(),
                    access: terminal_access(argument.access),
                });
            }
            if let Some(declaration_ordinal) = argument.source_local_declaration_ordinal() {
                let source = trivial_affine_locals
                    .get(usize::try_from(declaration_ordinal).map_err(|_| {
                        LoweringError::Unsupported("Unit local argument ordinal exceeds usize")
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "Unit local argument has an invalid declaration ordinal",
                    ))?;
                if !matches!(
                    source.kind,
                    StructuralPlaceKind::TrivialAffineLocal {
                        declaration_ordinal: source_ordinal,
                        ..
                    } if source_ordinal == declaration_ordinal
                ) || !argument.path.is_empty()
                {
                    return unsupported("Unit local argument drifted from checked custody");
                }
                return Ok(StructuralArgument {
                    place: source.id,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                });
            }
            if let Some(binding_ordinal) = argument.source_structural_result_binding_ordinal() {
                let source = structural_result_source(structural_results, binding_ordinal, argument)?;
                let structural_type = match source.kind {
                    StructuralPlaceKind::OperationResult { structural_type, .. }
                    | StructuralPlaceKind::ByteSequenceLiteral { structural_type, .. } => structural_type,
                    _ => return unsupported("structural result argument has no declared type"),
                };
                return Ok(StructuralArgument {
                    place: source.id,
                    path: lower_typed_argument_path(&argument.path, structural_type, structural_types, caller_scalar_values, &mut *selector_values, calls.as_deref_mut())?,
                    access: terminal_access(argument.access),
                });
            }
            let source_parameter_index =
                argument
                    .source_parameter_index()
                    .ok_or(LoweringError::Unsupported(
                        "Unit structural argument is neither a parameter nor a lowered literal",
                    ))?;
            let parameter = parameters
                .get(usize::try_from(source_parameter_index).map_err(|_| {
                    LoweringError::Unsupported("Unit structural argument index exceeds usize")
                })?)
                .ok_or(LoweringError::Unsupported(
                    "Unit structural argument has an invalid caller parameter index",
                ))?;
            Ok(StructuralArgument {
                place: parameter.place,
                path: lower_typed_argument_path(&argument.path, parameter.structural_type, structural_types, caller_scalar_values, &mut *selector_values, calls.as_deref_mut())?,
                access: terminal_access(argument.access),
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .and_then(|lowered| {
            if next_byte_argument == byte_argument_places.len() {
                Ok(lowered)
            } else {
                unsupported("byte-sequence place count disagrees with arguments")
            }
        })
}

fn structural_result_source<'results>(
    results: &'results [(StructuralPlaceDeclaration, bool)],
    binding_ordinal: u32,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
) -> Result<&'results StructuralPlaceDeclaration, LoweringError> {
    let (source, discard) = results
        .get(usize::try_from(binding_ordinal).map_err(|_| {
            LoweringError::Unsupported("Unit structural result binding ordinal exceeds usize")
        })?)
        .ok_or(LoweringError::Unsupported(
            "Unit structural result binding is not produced",
        ))?;
    // A whole owned move must not come from a result still marked for its
    // return drop. A projected owned move is different: the subtree's custody
    // was proven upstream (the bare reference result's leaf loan), and the
    // carrier's residual legitimately keeps that return drop.
    if (*discard
        && argument.access == checked_trees::CheckedStructuralAccess::Owned
        && argument.path.is_empty())
        || !matches!(
            source.kind,
            StructuralPlaceKind::OperationResult { .. }
                | StructuralPlaceKind::ByteSequenceLiteral { .. }
        )
    {
        return unsupported("Unit structural result argument disagrees with its producer cleanup");
    }
    Ok(source)
}

pub(crate) fn lower_projected_qualifications(
    rows: &[checked_trees::CheckedStructuralPathQualification],
    domain_ids: &[StructuralDomainBinding],
    root: StructuralTypeId,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<Vec<terminal_psi::StructuralPathQualification>, LoweringError> {
    rows.iter()
        .map(|row| {
            if row.path.is_empty() {
                return unsupported("projected qualification has an empty path");
            }
            let path = lower_structural_path(&row.path)?;
            let carrier = terminal_semantics::runtime_structural_path_tip(
                structural_types.iter(),
                root,
                &path,
            )
            .ok_or(LoweringError::Unsupported(
                "projected qualification has no exact structural carrier",
            ))?
            .id;
            Ok(terminal_psi::StructuralPathQualification {
                path,
                domain: lookup_domain_id_for_carrier(domain_ids, row.domain, carrier)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{StructuralDomainBinding, lower_projected_qualifications};
    use crate::terminal_identities::{structural_domain_id, structural_type_id};

    #[test]
    fn array_result_window_replays_backing_bounds_access_and_custody() {
        use checked_trees::{
            CheckedFusedServiceParameterReceipt, CheckedStructuralAccess,
            CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
            CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
        };
        use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
        use terminal_psi::{
            ByteSequenceCarrier, StructuralFieldDeclaration, StructuralFieldType,
            StructuralTypeDeclaration, StructuralTypeShape,
        };

        let byte = structural_type_id(1);
        let backing = structural_type_id(2);
        let view = structural_type_id(3);
        let record = structural_type_id(4);
        let types = vec![
            StructuralTypeDeclaration {
                id: byte,
                identity: "byte".into(),
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
                )),
            },
            StructuralTypeDeclaration {
                id: backing,
                identity: "backing".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: byte,
                    length: 4,
                },
            },
            StructuralTypeDeclaration {
                id: view,
                identity: "view".into(),
                shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: None,
                }),
            },
            StructuralTypeDeclaration {
                id: record,
                identity: "record".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: crate::terminal_identities::structural_field_id(1),
                        identity: "bytes".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(backing),
                    }],
                },
            },
        ];
        let identities = types
            .iter()
            .map(|declaration| (declaration.identity.clone(), declaration.id))
            .collect::<Vec<_>>();
        let argument = CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: 0,
            },
            path: vec![CheckedUnitStructuralPathSegment::FixedByteRange { start: 1, end: 3 }],
            type_identity: "view".into(),
            access: CheckedStructuralAccess::SharedBorrow,
        };
        let target = CheckedUnitStructuralParameterPlan {
            position: 0,
            is_self: false,
            type_identity: "view".into(),
            multiplicity: language_semantics::Multiplicity::Unrestricted,
            access: CheckedStructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            fused_service_erasure: None,
        };
        let admitted = |root,
                        argument: &CheckedUnitStructuralArgumentPlan,
                        target: &CheckedUnitStructuralParameterPlan,
                        types: &[_]| {
            let source = terminal_psi::StructuralParameterDeclaration {
                place: crate::terminal_identities::place_id(1),
                position: 0,
                is_self: false,
                structural_type: root,
                access: terminal_psi::StructuralAccess::Owned,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            };
            super::validate_borrowed_argument(&source, argument, target, &identities, types).is_ok()
        };
        assert!(admitted(backing, &argument, &target, &types));
        let mut whole = argument.clone();
        whole.path.clear();
        assert!(admitted(backing, &whole, &target, &types));
        let mut projected = argument.clone();
        projected
            .path
            .insert(0, CheckedUnitStructuralPathSegment::Field("bytes".into()));
        assert!(admitted(record, &projected, &target, &types));
        projected.path[0] = CheckedUnitStructuralPathSegment::Field("missing".into());
        assert!(!admitted(record, &projected, &target, &types));

        for (start, end) in [(3, 1), (1, 5)] {
            let mut changed = argument.clone();
            changed.path[0] = CheckedUnitStructuralPathSegment::FixedByteRange { start, end };
            assert!(!admitted(backing, &changed, &target, &types));
        }
        let mut changed = argument.clone();
        changed.access = CheckedStructuralAccess::MutableBorrow;
        assert!(!admitted(backing, &changed, &target, &types));
        let mut mutable_target = target.clone();
        mutable_target.access = CheckedStructuralAccess::MutableBorrow;
        assert!(admitted(backing, &changed, &mutable_target, &types));
        // Inline byte owners keep their original backing when presented as
        // mutable views. Predicates are independently replayed by Terminal;
        // the source-level restricted-field tests exercise that rejection.
        let mut inline_types = types.clone();
        let StructuralTypeShape::Record { fields } = &mut inline_types[3].shape else {
            unreachable!()
        };
        fields[0].field_type =
            StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 4 });
        let mut inline_argument = changed.clone();
        inline_argument.path = vec![CheckedUnitStructuralPathSegment::Field("bytes".into())];
        assert!(admitted(
            record,
            &inline_argument,
            &mutable_target,
            &inline_types
        ));
        changed.access = CheckedStructuralAccess::WriteOnlyBorrow;
        mutable_target.access = CheckedStructuralAccess::WriteOnlyBorrow;
        assert!(!admitted(backing, &changed, &mutable_target, &types));

        let mut changed_types = types.clone();
        changed_types[0].shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 16).expect("u16"),
        ));
        assert!(!admitted(backing, &argument, &target, &changed_types));
        let mut erased = target.clone();
        erased.fused_service_erasure = Some(CheckedFusedServiceParameterReceipt {
            source_parameter: symbols::SymbolHandle::from_arena_index(1),
            carrier_type_identity: "view".into(),
            requirement: symbols::SymbolHandle::from_arena_index(2),
        });
        assert!(!admitted(backing, &argument, &erased, &types));
        let mut qualified = target.clone();
        qualified
            .qualifications
            .push(language_semantics::SemanticDomainId(1));
        assert!(!admitted(backing, &argument, &qualified, &types));
    }

    #[test]
    fn byte_projection_preserves_embedded_view_access() {
        use super::*;
        use checked_trees::{
            CheckedRuntimeIndex, CheckedStructuralAccess as Access,
            CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
            CheckedUnitStructuralParameterPlan,
        };
        use terminal_psi::{BindingRelevance, StructuralFieldDeclaration, StructuralFieldType};
        let byte = structural_type_id(1);
        let record = structural_type_id(2);
        let source = StructuralParameterDeclaration {
            place: place_id(1),
            position: 0,
            is_self: false,
            structural_type: record,
            access: StructuralAccess::Owned,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        let identities = vec![("byte".into(), byte), ("record".into(), record)];
        for embedded in [
            None,
            Some(StructuralAccess::SharedBorrow),
            Some(StructuralAccess::MutableBorrow),
        ] {
            let types = vec![
                StructuralTypeDeclaration {
                    id: byte,
                    identity: "byte".into(),
                    shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            8,
                        )
                        .unwrap(),
                    )),
                },
                StructuralTypeDeclaration {
                    id: record,
                    identity: "record".into(),
                    shape: StructuralTypeShape::Record {
                        fields: vec![StructuralFieldDeclaration {
                            id: crate::terminal_identities::structural_field_id(1),
                            identity: "bytes".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::ByteSequence(
                                ByteSequenceCarrier::BorrowedView { access: embedded },
                            ),
                        }],
                    },
                },
            ];
            for selector in [
                CheckedUnitStructuralPathSegment::FixedIndex(0),
                CheckedUnitStructuralPathSegment::RuntimeIndex(CheckedRuntimeIndex::Parameter {
                    position: 0,
                }),
            ] {
                for access in [
                    Access::SharedBorrow,
                    Access::MutableBorrow,
                    Access::WriteOnlyBorrow,
                ] {
                    let argument = CheckedUnitStructuralArgumentPlan {
                        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: 0,
                        },
                        path: vec![
                            CheckedUnitStructuralPathSegment::Field("bytes".into()),
                            selector.clone(),
                        ],
                        type_identity: "byte".into(),
                        access,
                    };
                    let target = CheckedUnitStructuralParameterPlan {
                        position: 0,
                        is_self: false,
                        type_identity: "byte".into(),
                        access,
                        multiplicity: Multiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        fused_service_erasure: None,
                    };
                    assert_eq!(
                        validate_borrowed_argument(
                            &source,
                            &argument,
                            &target,
                            &identities,
                            &types
                        )
                        .is_ok(),
                        embedded != Some(StructuralAccess::SharedBorrow)
                            || access == Access::SharedBorrow
                    );
                }
            }
            let projection = crate::expression_preparation::bindings::structural_paths::prepare_argument_projection(
                &[CheckedUnitStructuralPathSegment::Field("bytes".into()),
                    CheckedUnitStructuralPathSegment::FixedByteRange { start: 0, end: 1 }], record, &types).unwrap();
            assert_eq!(
                projection.crossed_accesses,
                embedded.into_iter().collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn projected_domain_binding_resolves_the_exact_leaf_carrier() {
        let family = language_semantics::SemanticDomainId(1);
        let mut types = Vec::new();
        let mut bindings = Vec::new();
        for (ordinal, capacity) in [(1, 10), (2, 27)] {
            let carrier = structural_type_id(capacity);
            types.push(terminal_psi::StructuralTypeDeclaration {
                id: carrier,
                identity: format!("bytes-{capacity}"),
                shape: terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                ),
            });
            types.push(terminal_psi::StructuralTypeDeclaration {
                id: structural_type_id(100 + ordinal),
                identity: format!("array-{ordinal}"),
                shape: terminal_psi::StructuralTypeShape::FixedArray {
                    element: carrier,
                    length: 1,
                },
            });
            bindings.push(StructuralDomainBinding {
                semantic_domain: family,
                carrier,
                domain: structural_domain_id(ordinal),
            });
        }
        let mut rows = [checked_trees::CheckedStructuralPathQualification {
            path: vec![checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                0,
            )],
            domain: family,
        }];
        for ordinal in [1, 2] {
            let lowered = lower_projected_qualifications(
                &rows,
                &bindings,
                structural_type_id(100 + ordinal),
                &types,
            )
            .unwrap();
            assert_eq!(lowered[0].domain, structural_domain_id(ordinal));
        }
        rows[0].path = vec![checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
            1,
        )];
        assert!(
            lower_projected_qualifications(&rows, &bindings, structural_type_id(101), &types,)
                .is_err()
        );
    }
}
