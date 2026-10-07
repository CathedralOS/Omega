//! Parameter, transfer, and structural-argument lowering for attached Unit
//! closures.

use super::super::{
    BTreeSet, CheckedUnitStructuralPathSegment, ClaimId, PermissionClaimIdentity, ServiceId,
    ServiceReachId, ServiceReachInterface, ServiceReachPlan, StructuralAccess, StructuralArgument,
    StructuralParameterDeclaration, StructuralTypeDeclaration,
};
use super::{
    CheckedBoundaryMachinePlan, CheckedTrees, ClaimTransfer, LoweringError, Multiplicity, PlaceId,
    ScalarType, ServiceReachSummary, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralTypeId, ValueDeclaration, allocate_dense, lookup_claim_id, lookup_service_id,
    lookup_type_id, place_id, require_valid_service_row, terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::emit_direct_expression;
use crate::expression_preparation::bindings::structural_paths::{
    lower_structural_path, terminal_access,
};
use crate::expression_preparation::bindings::{
    StructuralLocalBinding, StructuralScalarFieldBinding,
};
use crate::terminal_identities::{StructuralDomainBinding, lookup_domain_id_for_carrier};
use semantic_vocabulary::ValueId;
mod reference_sources;
pub(crate) use reference_sources::{lower_reference_result_sources, lower_reference_source};
mod service_forward;
pub(in crate::unit::attached_unit) use service_forward::validate_parameter as validate_service_parameter;
mod source_path;
pub(crate) use source_path::{
    expression_producer, source_path, source_place_path, statement_receiver_path,
};

/// The carrier beneath a type's qualification shells, for contents
/// classification and shape inspection. Qualifications have their own
/// retained row, and reference presentation is preserved rather than silently
/// turning a view into ownership. Rejoining a structural plan compares
/// identities through `validation::stored_structural_type_identity` instead,
/// which keeps the qualification a bounded byte owner's plan is named by.
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
        let _ = checked_scalar_source_parameters(checked, state)?;
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
            if source.is_const
                || source.is_mutable
                    != (carrier.access == checked_trees::CheckedStructuralAccess::MutableBorrow)
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
            let mut qualifications = carrier.qualifications;
            qualifications.sort_by_key(|domain| domain.0);
            qualifications.dedup();
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
            if source.is_const {
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
                && (parameter.multiplicity != service_parameter_multiplicity(parameter.access)?
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
                structural_type: lookup_type_id(type_ids, &parameter.type_identity)?,
                multiplicity: crate::terminal_identities::structural_multiplicity(
                    parameter.multiplicity,
                ),
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

/// Resolve every checked transfer through the caller's exact claim table.
pub(crate) fn emitted_claim_transfers(
    transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
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
    emitted.sort();
    Ok(emitted)
}

/// Resolve every borrowed operand against its established home, regardless of
/// whether a parameter or a completed operation introduced that home. The path
/// preparation used by emission supplies the exact endpoint and every explicit
/// reference or view crossing. This checks presentation, not establishment, live-loan
/// authority, runtime bounds, or qualification implication; those retain their
/// independent source and Terminal replay.
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

/// Emit the declaration-resolved scalar read for every `StructuralField`
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
            let observation =
                crate::expression_preparation::bindings::structural_fields::resolve_typed(
                    fields,
                    field.parameter_position,
                    &field.path,
                )?;
            if !matches!(observation.scalar_type(), ScalarType::Integer(integer) if !integer.is_address())
            {
                return unsupported("runtime index field selector is not an integer read");
            }
            values.push(emit_direct_expression(
                &observation,
                caller_scalar_values,
                next_value,
                operations,
            ));
        }
    }
    Ok(values)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_structural_arguments(
    arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    structural_types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
    trivial_affine_locals: &[StructuralPlaceDeclaration],
    structural_results: &[(Option<StructuralPlaceDeclaration>, bool)],
    completed_results: &[(
        u32,
        crate::emission::operation_emission::buffer::StructuralBinding,
    )],
    byte_argument_places: &[PlaceId],
    primitive_locals: &[super::primitive_locals::PrimitiveLocal],
    structural_locals: &[StructuralLocalBinding],
    caller_scalar_values: Option<&[ValueDeclaration]>,
    selector_values: &mut dyn Iterator<Item = ValueId>,
    mut calls: Option<&mut CallEmissionContext<'_>>,
    mut emission: Option<(&mut u64, &mut OperationBuffer)>,
) -> Result<Vec<StructuralArgument>, LoweringError> {
    let mut lower_path = |path: &[CheckedUnitStructuralPathSegment],
                          root,
                          place,
                          prefix: &[terminal_psi::StructuralPathSegment]| {
        let suffix =
            crate::expression_preparation::bindings::structural_paths::prepare_argument_path(
                path,
                root,
                structural_types,
            )?;
        let path = prefix.iter().cloned().map(crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment::Static).chain(suffix).collect::<Vec<_>>();
        crate::expression_preparation::bindings::structural_paths::emit_captured_argument_path(
            &path,
            place,
            caller_scalar_values,
            &mut *selector_values,
            calls.as_deref_mut(),
            emission
                .as_mut()
                .map(|(next, operations)| (&mut **next, &mut **operations)),
        )
    };
    let mut next_byte_argument = 0usize;
    arguments
        .iter()
        .map(|argument| {
            if let Some(root) = argument.structural_storage_root() {
                let mut bindings = structural_locals.iter().filter(|binding| binding.root == root);
                let binding = bindings.next().ok_or(LoweringError::Unsupported(
                    "structural argument has no established storage",
                ))?;
                let access = terminal_access(argument.access);
                if bindings.next().is_some()
                    || !structural_access_can_supply(binding.argument.access, access)
                {
                    return unsupported("structural argument changes its established storage custody");
                }
                // A published local may own a projected subtree without a
                // separate place. Continue from its retained path and type.
                let path = lower_path(&argument.path, binding.structural_type, binding.argument.place, &binding.argument.path)?;
                return Ok(StructuralArgument {
                    place: binding.argument.place,
                    path,
                    access,
                });
            }
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
                return Ok(StructuralArgument {
                    place: source.id,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                });
            }
            if let Some(binding_ordinal) = argument.source_structural_result_binding_ordinal() {
                // A projected owned alias declares no place: it names an
                // owned subtree of a live root, so the argument is that
                // root's place walked to the leaf, and the use's own
                // projection continues from there. This is the operation
                // lane Terminal Psi admits for a partial move.
                if let Some(projected_alias) = projected_alias(completed_results, binding_ordinal) {
                    let path = lower_path(&argument.path, projected_alias.structural_type, projected_alias.place, projected_alias.path)?;
                    return Ok(StructuralArgument {
                        place: projected_alias.place,
                        path,
                        access: terminal_access(argument.access),
                    });
                }
                let source = structural_result_source(structural_results, binding_ordinal, argument)?;
                let structural_type = match source.kind {
                    StructuralPlaceKind::OperationResult { structural_type, .. }
                    | StructuralPlaceKind::ByteSequenceLiteral { structural_type, .. } => structural_type,
                    StructuralPlaceKind::BlockParameter { .. } => {
                        // A completed block arrival owns its published whole value.
                        // Projected arrival typing is handled by its storage binding.
                        return Ok(StructuralArgument {
                            place: source.id,
                            path: Vec::new(),
                            access: terminal_access(argument.access),
                        });
                    }
                    _ => return unsupported("structural result argument has no declared type"),
                };
                return Ok(StructuralArgument {
                    place: source.id,
                    path: lower_path(&argument.path, structural_type, source.id, &[])?,
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
                path: lower_path(&argument.path, parameter.structural_type, parameter.place, &[])?,
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

/// One completed binding that is a projected owned alias: the place that
/// holds the storage, the walk to the aliased leaf, and the leaf's declared
/// carrier. Every other binding occupies its whole place and answers here
/// with `None`.
struct ProjectedAliasBinding<'results> {
    place: PlaceId,
    path: &'results [terminal_psi::StructuralPathSegment],
    structural_type: StructuralTypeId,
}

fn projected_alias<'results>(
    completed_results: &'results [(
        u32,
        crate::emission::operation_emission::buffer::StructuralBinding,
    )],
    binding_ordinal: u32,
) -> Option<ProjectedAliasBinding<'results>> {
    let (_, binding) = completed_results
        .iter()
        .find(|(ordinal, _)| *ordinal == binding_ordinal)?;
    let crate::emission::operation_emission::buffer::StructuralBinding::ProjectedAlias {
        place,
        path,
        structural_type,
        ..
    } = binding
    else {
        return None;
    };
    Some(ProjectedAliasBinding {
        place: *place,
        path,
        structural_type: *structural_type,
    })
}

fn structural_result_source<'results>(
    results: &'results [(Option<StructuralPlaceDeclaration>, bool)],
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
    // A projected owned alias declares no place of its own, so there is no
    // producer custody for this join to read. Its uses resolve through the
    // local namespace, which composes the alias path with the use's own.
    let source = source.as_ref().ok_or(LoweringError::Unsupported(
        "Unit structural result binding is a projected alias",
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
                | StructuralPlaceKind::BlockParameter { .. }
        )
        || (matches!(source.kind, StructuralPlaceKind::BlockParameter { .. })
            && (!argument.path.is_empty()
                || argument.access != checked_trees::CheckedStructuralAccess::Owned))
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
