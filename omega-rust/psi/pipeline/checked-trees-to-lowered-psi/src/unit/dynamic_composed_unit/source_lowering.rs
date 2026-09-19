//! Lowering dynamic sources, selection sources and dynamic call custody.

use crate::unit::dynamic_composed_unit::dynamic_lanes::{DynamicLoweringLane, ForwardedHelperIds};
use crate::unit::dynamic_composed_unit::structural_types::terminal_structural_multiplicity;
use crate::unit::{LoweringError, lookup_type_id, lower_structural_path, unsupported};
use checked_trees::{
    CheckedDynamicScalarCallPlan, CheckedStructuralAccess, CheckedUnitStructuralPathSegment,
};
use language_semantics::Multiplicity;
use terminal_psi::{
    ClosedConformanceApplication, ClosedConformanceRow, OperationKind, StructuralAccess,
    StructuralArgument, StructuralParameterDeclaration, TerminalDirectDynamicDispatch,
    TerminalDynamicConformanceSelection, TerminalDynamicDescriptorArgument,
    TerminalDynamicDescriptorParameter, TerminalDynamicDescriptorSource,
    TerminalDynamicDispatchCatalog, TerminalDynamicRequirement, TerminalIndirectDynamicDispatch,
    TerminalParameterDynamicDispatch, TerminalReboundDynamicDescriptor,
    TerminalStoredDynamicDescriptor, TerminalStoredDynamicDispatch,
};

pub(crate) fn validate_and_lower_source(
    caller_self: &StructuralParameterDeclaration,
    plan: &CheckedDynamicScalarCallPlan,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    type_ids: &[(String, semantic_vocabulary::StructuralTypeId)],
) -> Result<StructuralArgument, LoweringError> {
    validate_and_lower_selection_source(
        caller_self,
        plan,
        &plan.source_path,
        &plan.source_type_identity,
        structural_types,
        type_ids,
    )
}

fn validate_and_lower_selection_source(
    caller_self: &StructuralParameterDeclaration,
    plan: &CheckedDynamicScalarCallPlan,
    source_path: &[CheckedUnitStructuralPathSegment],
    source_type_identity: &str,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    type_ids: &[(String, semantic_vocabulary::StructuralTypeId)],
) -> Result<StructuralArgument, LoweringError> {
    validate_and_lower_dynamic_source(
        caller_self,
        plan.source_parameter_position,
        plan.caller_parameter_access,
        plan.caller_multiplicity,
        plan.source_access,
        &plan.caller_attachment_type_identity,
        source_path,
        source_type_identity,
        structural_types,
        type_ids,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_and_lower_dynamic_source(
    caller_self: &StructuralParameterDeclaration,
    source_parameter_position: u32,
    caller_parameter_access: CheckedStructuralAccess,
    caller_multiplicity: Multiplicity,
    source_access: CheckedStructuralAccess,
    caller_attachment_type_identity: &str,
    source_path: &[CheckedUnitStructuralPathSegment],
    source_type_identity: &str,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    type_ids: &[(String, semantic_vocabulary::StructuralTypeId)],
) -> Result<StructuralArgument, LoweringError> {
    if source_parameter_position != caller_self.position
        || caller_parameter_access
            != match caller_self.access {
                StructuralAccess::SharedBorrow => CheckedStructuralAccess::SharedBorrow,
                StructuralAccess::MutableBorrow => CheckedStructuralAccess::MutableBorrow,
                _ => return unsupported("direct dynamic caller self access is unsupported"),
            }
        || !caller_self.is_self
        || caller_self.multiplicity != terminal_structural_multiplicity(caller_multiplicity)
        || !matches!(
            source_access,
            CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
        )
        || (source_access == CheckedStructuralAccess::MutableBorrow
            && caller_self.access != StructuralAccess::MutableBorrow)
    {
        return unsupported("direct dynamic caller self does not license the field subloan");
    }
    let attachment_id = lookup_type_id(type_ids, caller_attachment_type_identity)?;
    let source_type = lookup_type_id(type_ids, source_type_identity)?;
    let attachment = structural_types
        .iter()
        .find(|declaration| declaration.id == attachment_id)
        .ok_or(LoweringError::Unsupported(
            "direct dynamic caller attachment declaration is absent",
        ))?;
    let [CheckedUnitStructuralPathSegment::Field(field_identity)] = source_path else {
        unreachable!("direct source path was validated")
    };
    let terminal_psi::StructuralTypeShape::Record { fields } = &attachment.shape else {
        return unsupported("direct dynamic caller attachment must be a record");
    };
    let matching_fields = fields
        .iter()
        .filter(|field| {
            field.identity == *field_identity
                && field.field_type == terminal_psi::StructuralFieldType::Structural(source_type)
        })
        .count();
    if matching_fields != 1 {
        return unsupported("direct dynamic source field no longer matches its structural carrier");
    }
    Ok(StructuralArgument {
        place: caller_self.place,
        path: lower_structural_path(source_path),
        access: match source_access {
            CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
            CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
            _ => unreachable!("borrowed dynamic source access was validated"),
        },
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_dynamic_call_custody(
    lane: DynamicLoweringLane<'_>,
    caller_self: &StructuralParameterDeclaration,
    plan: &CheckedDynamicScalarCallPlan,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    type_ids: &[(String, semantic_vocabulary::StructuralTypeId)],
    caller_machine: semantic_vocabulary::MachineId,
    call_operation: semantic_vocabulary::OperationId,
    descriptor_store_operation: Option<semantic_vocabulary::OperationId>,
    latest_source: StructuralArgument,
    initial_application: Option<&ClosedConformanceApplication>,
    application: &ClosedConformanceApplication,
    selected_row: &ClosedConformanceRow,
    callable_identity: String,
    realization_machine: semantic_vocabulary::MachineId,
    forwarded_helper: Option<ForwardedHelperIds>,
) -> Result<(TerminalDynamicDispatchCatalog, OperationKind), LoweringError> {
    let latest_selection = TerminalDynamicConformanceSelection {
        owner: caller_machine,
        ordinal: u32::from(matches!(lane, DynamicLoweringLane::Rebound(_))),
        source: latest_source.clone(),
        conformance_application_report_fingerprint: application.report_fingerprint,
        conformance_application_commitment: application.commitment,
    };
    let row_dispatch = |descriptor_ordinal| TerminalIndirectDynamicDispatch {
        owner: caller_machine,
        operation: call_operation,
        descriptor_ordinal,
        declaring_trait_identity: selected_row.declaring_trait_identity.clone(),
        public_requirement_identity: selected_row.public_requirement_identity.clone(),
        family_tuple: selected_row.family_tuple.clone(),
        requirement_identity: selected_row.requirement_identity.clone(),
        realization_identity: selected_row.realization_identity.clone(),
        realization_callable_identity: callable_identity.clone(),
        realization: realization_machine,
    };
    let stored_row_dispatch = |descriptor_ordinal| TerminalStoredDynamicDispatch {
        owner: caller_machine,
        operation: call_operation,
        descriptor_ordinal,
        declaring_trait_identity: selected_row.declaring_trait_identity.clone(),
        public_requirement_identity: selected_row.public_requirement_identity.clone(),
        family_tuple: selected_row.family_tuple.clone(),
        requirement_identity: selected_row.requirement_identity.clone(),
        realization_identity: selected_row.realization_identity.clone(),
        realization_callable_identity: callable_identity.clone(),
        realization: realization_machine,
    };
    Ok(match lane {
        DynamicLoweringLane::Direct => {
            if initial_application.is_some() {
                return unsupported("direct dynamic dispatch retained a rebound application");
            }
            let mut catalog = TerminalDynamicDispatchCatalog {
                parameters: Vec::new(),
                arguments: Vec::new(),
                selections: vec![latest_selection],
                rebound_descriptors: Vec::new(),
                stored_descriptors: Vec::new(),
                direct_dispatches: Vec::new(),
                indirect_dispatches: Vec::new(),
                stored_dispatches: Vec::new(),
                parameter_dispatches: Vec::new(),
            };
            let call_kind = if let Some(helper) = forwarded_helper {
                let (requirements, requirement_slot) =
                    dynamic_parameter_interface(application, selected_row)?;
                catalog.parameters.push(TerminalDynamicDescriptorParameter {
                    owner: helper.machine,
                    ordinal: 0,
                    source_position: 0,
                    trait_identity: application.trait_identity.clone(),
                    access: latest_source.access,
                    requirements,
                });
                catalog.arguments.push(TerminalDynamicDescriptorArgument {
                    owner: caller_machine,
                    operation: call_operation,
                    parameter_ordinal: 0,
                    source: TerminalDynamicDescriptorSource::Selection { ordinal: 0 },
                });
                catalog
                    .parameter_dispatches
                    .push(TerminalParameterDynamicDispatch {
                        owner: helper.machine,
                        operation: helper.operation,
                        parameter_ordinal: 0,
                        requirement_slot,
                    });
                OperationKind::CallStructuralScalar {
                    callee: helper.machine,
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                }
            } else {
                catalog
                    .direct_dispatches
                    .push(TerminalDirectDynamicDispatch {
                        owner: caller_machine,
                        operation: call_operation,
                        selection_ordinal: 0,
                        declaring_trait_identity: selected_row.declaring_trait_identity.clone(),
                        public_requirement_identity: selected_row
                            .public_requirement_identity
                            .clone(),
                        family_tuple: selected_row.family_tuple.clone(),
                        requirement_identity: selected_row.requirement_identity.clone(),
                        realization_identity: selected_row.realization_identity.clone(),
                        realization_callable_identity: callable_identity,
                        realization: realization_machine,
                    });
                OperationKind::CallStructuralScalar {
                    callee: realization_machine,
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    structural_arguments: vec![latest_source],
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                }
            };
            (catalog, call_kind)
        }
        DynamicLoweringLane::Rebound(initial) => {
            let initial_source = validate_and_lower_selection_source(
                caller_self,
                plan,
                &initial.path,
                &initial.type_identity,
                structural_types,
                type_ids,
            )?;
            let mut catalog = TerminalDynamicDispatchCatalog {
                parameters: Vec::new(),
                arguments: Vec::new(),
                selections: vec![
                    TerminalDynamicConformanceSelection {
                        owner: caller_machine,
                        ordinal: 0,
                        source: initial_source,
                        conformance_application_report_fingerprint: initial_application
                            .unwrap_or(application)
                            .report_fingerprint,
                        conformance_application_commitment: initial_application
                            .unwrap_or(application)
                            .commitment,
                    },
                    latest_selection,
                ],
                rebound_descriptors: vec![TerminalReboundDynamicDescriptor {
                    owner: caller_machine,
                    ordinal: 0,
                    initial_selection_ordinal: 0,
                    rebound_selection_ordinal: 1,
                }],
                stored_descriptors: Vec::new(),
                direct_dispatches: Vec::new(),
                indirect_dispatches: Vec::new(),
                stored_dispatches: Vec::new(),
                parameter_dispatches: Vec::new(),
            };
            let call_kind = if let Some(helper) = forwarded_helper {
                let (requirements, requirement_slot) =
                    dynamic_parameter_interface(application, selected_row)?;
                catalog.parameters.push(TerminalDynamicDescriptorParameter {
                    owner: helper.machine,
                    ordinal: 0,
                    source_position: 0,
                    trait_identity: application.trait_identity.clone(),
                    access: latest_source.access,
                    requirements,
                });
                catalog.arguments.push(TerminalDynamicDescriptorArgument {
                    owner: caller_machine,
                    operation: call_operation,
                    parameter_ordinal: 0,
                    source: TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal: 0 },
                });
                catalog
                    .parameter_dispatches
                    .push(TerminalParameterDynamicDispatch {
                        owner: helper.machine,
                        operation: helper.operation,
                        parameter_ordinal: 0,
                        requirement_slot,
                    });
                OperationKind::CallStructuralScalar {
                    callee: helper.machine,
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                }
            } else {
                catalog.indirect_dispatches.push(row_dispatch(0));
                OperationKind::CallDynamicScalar {
                    descriptor_ordinal: 0,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                }
            };
            (catalog, call_kind)
        }
        DynamicLoweringLane::Stored(stored) => {
            if initial_application.is_some() || forwarded_helper.is_some() {
                return unsupported(
                    "stored dynamic dispatch acquired unrelated descriptor custody",
                );
            }
            let catalog = TerminalDynamicDispatchCatalog {
                parameters: Vec::new(),
                arguments: Vec::new(),
                selections: vec![latest_selection],
                rebound_descriptors: Vec::new(),
                stored_descriptors: vec![TerminalStoredDynamicDescriptor {
                    owner: caller_machine,
                    ordinal: 0,
                    establishment_operation: descriptor_store_operation.ok_or(
                        LoweringError::Unsupported(
                            "stored dynamic descriptor has no allocated establishment operation",
                        ),
                    )?,
                    selection_ordinal: 0,
                    aggregate_type_identity: stored.destination_type_identity.clone(),
                    field_identity: stored.destination_field_identity.clone(),
                }],
                direct_dispatches: Vec::new(),
                indirect_dispatches: Vec::new(),
                stored_dispatches: vec![stored_row_dispatch(0)],
                parameter_dispatches: Vec::new(),
            };
            (
                catalog,
                OperationKind::CallDynamicScalar {
                    descriptor_ordinal: 0,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            )
        }
    })
}

pub(crate) fn dynamic_parameter_interface(
    application: &ClosedConformanceApplication,
    selected_row: &ClosedConformanceRow,
) -> Result<(Vec<TerminalDynamicRequirement>, u32), LoweringError> {
    let mut selected_slot = None;
    let requirements = application
        .rows
        .iter()
        .enumerate()
        .map(|(ordinal, row)| {
            let slot = u32::try_from(ordinal).map_err(|_| {
                LoweringError::Unsupported("dynamic parameter requirement ordinal exceeds u32")
            })?;
            if row == selected_row && selected_slot.replace(slot).is_some() {
                return unsupported("dynamic parameter selected requirement is duplicated");
            }
            let callable_identity =
                row.realization_callable_identity
                    .as_ref()
                    .ok_or(LoweringError::Unsupported(
                        "dynamic parameter application row has no realization callable",
                    ))?;
            let matching = application
                .realization_callables
                .iter()
                .filter(|callable| callable.source_callable_identity == *callable_identity)
                .collect::<Vec<_>>();
            let [callable] = matching.as_slice() else {
                return unsupported(
                    "dynamic parameter application row callable is absent or ambiguous",
                );
            };
            Ok(TerminalDynamicRequirement {
                slot,
                declaring_trait_identity: row.declaring_trait_identity.clone(),
                public_requirement_identity: row.public_requirement_identity.clone(),
                family_tuple: row.family_tuple.clone(),
                result: callable.result,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let selected_slot = selected_slot.ok_or(LoweringError::Unsupported(
        "dynamic parameter selected requirement is absent",
    ))?;
    Ok((requirements, selected_slot))
}
