use omega_calling_conventions::{
    IndirectPointerLocation, ValueLocation, ValuePlacement, ValueShape,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_assigned_target_operations::{
    TerminalAssignedAggregateCopy, TerminalAssignedFunction, TerminalAssignedUnitBody,
    TerminalAssignedUnitOperation,
};
use omega_terminal_machine_code::{
    TerminalAarch64ReturnLinkEvidence, TerminalBoundarySettlementRecord,
    TerminalInternalCallRelocation, TerminalInternalUnitCallArgumentRecord,
    TerminalInternalUnitCallRecord, TerminalNativeFuelAttribution, TerminalNativeFuelSite,
    TerminalPortEffectRecord, TerminalStackAdjustmentPair, TerminalUnitCallStackEvidence,
    TerminalUnitStackEvidence, derive_completion_provider_custody,
};
use omega_terminal_target_operations::TerminalCallSiteOwner;
use psi_core::MachineId;

use super::{
    EmissionError, aarch64_load_base, aarch64_store_base, aarch64_unit_memory_access,
    aarch64_unit_register, aarch64_unit_stack_access, append_aarch64_instructions,
    emit_aarch64_adjust_sp, emit_aarch64_sp_address, emit_x86_64_adjust_sp,
    emit_x86_64_memory_load_width, emit_x86_64_stack_load_width, emit_x86_64_stack_store_width,
    exact_partial_cleanup_partition, executable_nominal_cleanup, placement_fragment,
    stack_adjustment_pair, x86_unit_register,
};

pub(super) struct UnitEmission {
    pub(super) bytes: Vec<u8>,
    pub(super) internal_calls: Vec<TerminalInternalCallRelocation>,
    pub(super) internal_unit_calls: Vec<TerminalInternalUnitCallRecord>,
    pub(super) fuel_attribution: Vec<TerminalNativeFuelAttribution>,
    pub(super) port_effects: Vec<TerminalPortEffectRecord>,
    pub(super) boundary_settlements: Vec<TerminalBoundarySettlementRecord>,
    pub(super) stack: TerminalUnitStackEvidence,
    pub(super) parameter_homes: Vec<omega_terminal_machine_code::TerminalUnitParameterHomeRecord>,
    pub(super) parameters: Vec<omega_terminal_machine_code::TerminalUnitParameterRecord>,
    pub(super) affine_cleanup: Option<omega_terminal_machine_code::TerminalUnitAffineCleanupRecord>,
}

#[derive(Debug, Clone)]
pub(super) struct X86UnitParameterHome {
    place: psi_core::PlaceId,
    shape: ValueShape,
    source: ValuePlacement,
    byte_offset: u32,
    indirect: bool,
}

#[derive(Debug, Clone)]
pub(super) struct Aarch64UnitParameterHome {
    place: psi_core::PlaceId,
    shape: ValueShape,
    source: ValuePlacement,
    byte_offset: u32,
    indirect: bool,
}

pub(super) fn emit_unit_body(
    body: &TerminalAssignedUnitBody,
    target: NativeTarget,
    functions: &[TerminalAssignedFunction],
) -> Result<UnitEmission, EmissionError> {
    let mut bytes = Vec::new();
    let mut internal_calls = Vec::new();
    let mut internal_unit_calls = Vec::new();
    let mut fuel_attribution = Vec::new();
    let mut port_effects = Vec::new();
    let mut boundary_settlements = Vec::new();
    let mut x86_homes = Vec::new();
    let mut x86_frame_bytes = 0;
    let mut aarch64_homes = Vec::new();
    let mut aarch64_frame_bytes = 0;
    let mut aarch64_lr_offset = 0;
    let mut frame_allocation = None;
    let mut frame_release = None;
    let mut aarch64_link_store = None;
    let mut aarch64_link_load = None;
    let parameter_homes;
    match target.architecture {
        Architecture::X86_64 => {
            (x86_homes, x86_frame_bytes) = x86_unit_parameter_homes(body)?;
            parameter_homes = body
                .parameters
                .iter()
                .zip(&x86_homes)
                .map(|(parameter, home)| {
                    omega_terminal_machine_code::TerminalUnitParameterHomeRecord {
                        place: parameter.place,
                        structural_type: parameter.structural_type,
                        multiplicity: parameter.multiplicity,
                        shape: parameter.shape,
                        source: parameter.placement.clone(),
                        byte_offset: home.byte_offset,
                        indirect: home.indirect,
                    }
                })
                .collect();
            if x86_frame_bytes != 0 {
                let offset = bytes.len();
                emit_x86_64_adjust_sp(&mut bytes, x86_frame_bytes, false);
                frame_allocation = Some((offset, bytes.len() - offset));
                emit_x86_64_stage_unit_parameters(&mut bytes, &x86_homes, x86_frame_bytes)?;
            }
        }
        Architecture::Aarch64 => {
            let (homes, frame_bytes, lr_offset) = aarch64_unit_parameter_homes(body)?;
            aarch64_homes = homes;
            aarch64_frame_bytes = frame_bytes;
            aarch64_lr_offset = lr_offset;
            parameter_homes = body
                .parameters
                .iter()
                .zip(&aarch64_homes)
                .map(|(parameter, home)| {
                    omega_terminal_machine_code::TerminalUnitParameterHomeRecord {
                        place: parameter.place,
                        structural_type: parameter.structural_type,
                        multiplicity: parameter.multiplicity,
                        shape: parameter.shape,
                        source: parameter.placement.clone(),
                        byte_offset: home.byte_offset,
                        indirect: home.indirect,
                    }
                })
                .collect();
            let mut instructions = Vec::new();
            emit_aarch64_adjust_sp(&mut instructions, frame_bytes, false)?;
            frame_allocation = Some((0, 4));
            aarch64_link_store = Some(4);
            instructions.push(aarch64_unit_stack_access(0xf900_0000, 30, lr_offset, 8)?);
            emit_aarch64_stage_unit_parameters(&mut instructions, &aarch64_homes, frame_bytes)?;
            append_aarch64_instructions(&mut bytes, instructions);
        }
    };
    let mut returned = false;
    let mut affine_cleanup = None;
    let mut established_affine_locals = Vec::new();
    for (operation_ordinal, operation) in body.operations.iter().enumerate() {
        if returned {
            return Err(EmissionError::UnitOperationAfterReturn);
        }
        let code_offset = bytes.len();
        let mut operation_site = None;
        let mut edge_site = None;
        match operation {
            TerminalAssignedUnitOperation::EstablishTrivialAffineLocal {
                psi_operation,
                place,
                structural_type,
            } => {
                operation_site = Some(*psi_operation);
                established_affine_locals.push((*psi_operation, *place, structural_type.clone()));
            }
            TerminalAssignedUnitOperation::Call {
                psi_operation,
                callee,
                result,
                copies,
                claim_transfers,
            } => {
                operation_site = Some(*psi_operation);
                let argument_intervals = match target.architecture {
                    Architecture::X86_64 => emit_x86_64_unit_call(
                        &mut bytes,
                        TerminalCallSiteOwner::Operation(*psi_operation),
                        *callee,
                        copies,
                        target,
                        &x86_homes,
                        &mut internal_calls,
                    )?,
                    Architecture::Aarch64 => emit_aarch64_unit_call(
                        &mut bytes,
                        TerminalCallSiteOwner::Operation(*psi_operation),
                        *callee,
                        copies,
                        &aarch64_homes,
                        &mut internal_calls,
                    )?,
                };
                internal_unit_calls.push(TerminalInternalUnitCallRecord {
                    owner: TerminalCallSiteOwner::Operation(*psi_operation),
                    target: *callee,
                    result: *result,
                    arguments: copies
                        .iter()
                        .zip(argument_intervals)
                        .map(
                            |(
                                copy,
                                (
                                    code_offset,
                                    byte_count,
                                    source_home_byte_offset,
                                    call_stack_bytes,
                                ),
                            )| {
                                TerminalInternalUnitCallArgumentRecord {
                                    place: copy.place,
                                    path: copy.path.clone(),
                                    root_structural_type: copy.root_structural_type,
                                    structural_type: copy.structural_type,
                                    shape: copy.shape,
                                    source_byte_offset: copy.source_byte_offset,
                                    source_home_byte_offset,
                                    call_stack_bytes,
                                    fixed_array_length: copy.fixed_array_length,
                                    element_stride: copy.element_stride,
                                    source: copy.source.clone(),
                                    destination: copy.destination.clone(),
                                    code_offset,
                                    byte_count,
                                    bytes: bytes[code_offset..code_offset + byte_count].to_vec(),
                                }
                            },
                        )
                        .collect(),
                    claim_transfers: claim_transfers.clone(),
                    operation_ordinal,
                    code_offset,
                    byte_count: bytes.len() - code_offset,
                });
            }
            TerminalAssignedUnitOperation::PortWrite {
                psi_operation,
                service,
                port,
                value,
            } => {
                operation_site = Some(*psi_operation);
                if target.architecture != Architecture::X86_64 {
                    return Err(EmissionError::PortWriteUnsupportedOnArchitecture(
                        target.architecture,
                    ));
                }
                let code_offset = bytes.len();
                bytes.extend_from_slice(&omega_x86_encoding::encode_immediate_port_write(
                    *port, *value,
                ));
                port_effects.push(TerminalPortEffectRecord {
                    psi_operation: *psi_operation,
                    service: *service,
                    port: *port,
                    value: *value,
                    operation_ordinal,
                    code_offset,
                    byte_count: bytes.len() - code_offset,
                });
            }
            TerminalAssignedUnitOperation::BoundarySettlement {
                psi_operation,
                boundary,
                provider_execution,
                realization,
                arguments,
                completion_claim_sources,
                completion_receipts,
            } => {
                operation_site = Some(*psi_operation);
                let provider_execution = (*provider_execution).into();
                let completion_provider_custody = derive_completion_provider_custody(
                    provider_execution,
                    completion_claim_sources,
                    completion_receipts,
                )
                .ok_or(EmissionError::InvalidCompletionProviderCustody)?;
                boundary_settlements.push(TerminalBoundarySettlementRecord {
                    psi_operation: *psi_operation,
                    boundary: *boundary,
                    provider_execution,
                    realization: *realization,
                    arguments: arguments.clone(),
                    completion_claim_sources: completion_claim_sources.clone(),
                    completion_receipts: completion_receipts.clone(),
                    completion_provider_custody,
                    native_result: None,
                    operation_ordinal,
                    code_offset: bytes.len(),
                    byte_count: 0,
                });
            }
            TerminalAssignedUnitOperation::Return {
                psi_edge,
                cleanup_actions,
            } => {
                let transferred_roots = body.operations[..operation_ordinal]
                    .iter()
                    .filter_map(|operation| match operation {
                        TerminalAssignedUnitOperation::Call { copies, .. } => Some(copies),
                        _ => None,
                    })
                    .flatten()
                    .filter(|copy| copy.path.is_empty())
                    .map(|copy| copy.place)
                    .collect::<std::collections::BTreeSet<_>>();
                let expected_local_prefix = established_affine_locals
                    .iter()
                    .rev()
                    .map(|(_, place, _)| place.id)
                    .collect::<Vec<_>>();
                let expected_discards = expected_local_prefix
                    .iter()
                    .copied()
                    .chain(
                        body.parameters
                            .iter()
                            .rev()
                            .filter(|parameter| {
                                parameter.multiplicity
                                    == psi_terminal::StructuralMultiplicity::Affine
                                    && !transferred_roots.contains(&parameter.place)
                            })
                            .map(|parameter| parameter.place),
                    )
                    .collect::<Vec<_>>();
                let expected_root_actions = expected_discards
                    .iter()
                    .copied()
                    .map(psi_terminal::TerminalAffineCleanupAction::DiscardRoot)
                    .collect::<Vec<_>>();
                let expected_local_actions = expected_local_prefix
                    .iter()
                    .copied()
                    .map(psi_terminal::TerminalAffineCleanupAction::DiscardRoot)
                    .collect::<Vec<_>>();
                let nominal_cleanups = cleanup_actions
                    .iter()
                    .filter_map(|action| match action {
                        psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                            Some(cleanup)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let partial_cleanup_valid = if cleanup_actions == &expected_root_actions {
                    true
                } else {
                    let residual_actions = cleanup_actions
                        .get(expected_local_actions.len()..)
                        .unwrap_or_default();
                    let residuals = residual_actions
                        .iter()
                        .filter_map(|action| match action {
                            psi_terminal::TerminalAffineCleanupAction::DiscardResidual(
                                residual,
                            ) => Some(residual),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let residual_root = residuals.first().map(|residual| residual.place);
                    let moved = body.operations[..operation_ordinal]
                        .iter()
                        .filter_map(|operation| match operation {
                            TerminalAssignedUnitOperation::Call { copies, .. } => Some(copies),
                            _ => None,
                        })
                        .flatten()
                        .filter(|copy| Some(copy.place) == residual_root)
                        .map(|copy| (copy.path.as_slice(), copy.structural_type))
                        .collect::<Vec<_>>();
                    cleanup_actions.get(..expected_local_actions.len())
                        == Some(expected_local_actions.as_slice())
                        && !residuals.is_empty()
                        && residuals.len() == residual_actions.len()
                        && residual_root.is_some_and(|root| {
                            expected_discards.get(expected_local_actions.len()..) == Some(&[root])
                        })
                        && residuals.iter().all(|residual| {
                            Some(residual.place) == residual_root
                                && !residual.path.is_empty()
                                && residual.path.iter().all(|segment| {
                                    matches!(segment,
                                        psi_terminal::StructuralPathSegment::Field(identity)
                                            if !identity.is_empty())
                                })
                                && body.parameters.iter().any(|parameter| {
                                    parameter.place == residual.place
                                        && parameter.multiplicity
                                            == psi_terminal::StructuralMultiplicity::Affine
                                        && parameter.structural_type != residual.structural_type
                                })
                        })
                        && residuals.iter().enumerate().all(|(index, residual)| {
                            residuals[..index].iter().all(|earlier| {
                                !residual.path.starts_with(&earlier.path)
                                    && !earlier.path.starts_with(&residual.path)
                            })
                        })
                        && !moved.is_empty()
                        && moved.iter().all(|(moved_path, _)| {
                            !moved_path.is_empty()
                                && moved_path.iter().all(|segment| {
                                    matches!(segment,
                                        psi_terminal::StructuralPathSegment::Field(identity)
                                            if !identity.is_empty())
                                })
                                && residuals.iter().all(|residual| {
                                    !moved_path.starts_with(&residual.path)
                                        && !residual.path.starts_with(moved_path)
                                })
                        })
                        && moved.iter().enumerate().all(|(index, (moved_path, _))| {
                            moved[..index].iter().all(|(earlier, _)| {
                                !moved_path.starts_with(earlier) && !earlier.starts_with(moved_path)
                            })
                        })
                        && residual_root
                            .and_then(|root| {
                                body.parameters
                                    .iter()
                                    .find(|parameter| parameter.place == root)
                            })
                            .is_some_and(|parameter| {
                                exact_partial_cleanup_partition(
                                    &body.structural_types,
                                    parameter.structural_type,
                                    &moved,
                                    &residuals,
                                )
                            })
                } || (expected_local_prefix.is_empty()
                    && !nominal_cleanups.is_empty()
                    && nominal_cleanups.len() == cleanup_actions.len()
                    && nominal_cleanups.len() == body.parameters.len()
                    && body.parameters.iter().rev().zip(&nominal_cleanups).all(
                        |(parameter, cleanup)| {
                            parameter.place == cleanup.place
                                && parameter.structural_type == cleanup.structural_type
                                && parameter.multiplicity
                                    == psi_terminal::StructuralMultiplicity::Affine
                        },
                    ));
                if !partial_cleanup_valid {
                    return Err(EmissionError::UnsupportedAggregatePlacement);
                }
                edge_site = Some(*psi_edge);
                let nominal_execution = cleanup_actions
                    .iter()
                    .enumerate()
                    .filter_map(|(action_ordinal, action)| {
                        let psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) =
                            action
                        else {
                            return None;
                        };
                        Some(
                            u32::try_from(action_ordinal)
                                .map_err(|_| EmissionError::UnsupportedAggregatePlacement)
                                .and_then(|action_ordinal| {
                                    executable_nominal_cleanup(cleanup, functions)
                                        .map(|executable| (action_ordinal, cleanup, executable))
                                }),
                        )
                    })
                    .collect::<Result<Vec<_>, EmissionError>>()?;
                for (action_ordinal, cleanup, executable) in nominal_execution {
                    if executable {
                        let owner = TerminalCallSiteOwner::CleanupAction {
                            edge: *psi_edge,
                            action_ordinal,
                        };
                        let call_code_offset = bytes.len();
                        match target.architecture {
                            Architecture::X86_64 => {
                                emit_x86_64_unit_call(
                                    &mut bytes,
                                    owner,
                                    cleanup.cleanup_machine,
                                    &[],
                                    target,
                                    &x86_homes,
                                    &mut internal_calls,
                                )?;
                            }
                            Architecture::Aarch64 => {
                                emit_aarch64_unit_call(
                                    &mut bytes,
                                    owner,
                                    cleanup.cleanup_machine,
                                    &[],
                                    &aarch64_homes,
                                    &mut internal_calls,
                                )?;
                            }
                        }
                        internal_unit_calls.push(TerminalInternalUnitCallRecord {
                            owner,
                            target: cleanup.cleanup_machine,
                            result: None,
                            arguments: Vec::new(),
                            claim_transfers: Vec::new(),
                            operation_ordinal,
                            code_offset: call_code_offset,
                            byte_count: bytes.len() - call_code_offset,
                        });
                    }
                }
                match target.architecture {
                    Architecture::X86_64 => {
                        if x86_frame_bytes != 0 {
                            let offset = bytes.len();
                            emit_x86_64_adjust_sp(&mut bytes, x86_frame_bytes, true);
                            frame_release = Some((offset, bytes.len() - offset));
                        }
                        bytes.push(0xc3)
                    }
                    Architecture::Aarch64 => {
                        let mut instructions = Vec::new();
                        aarch64_link_load = Some(bytes.len());
                        instructions.push(aarch64_unit_stack_access(
                            0xf940_0000,
                            30,
                            aarch64_lr_offset,
                            8,
                        )?);
                        let release_offset = bytes.len() + 4;
                        emit_aarch64_adjust_sp(&mut instructions, aarch64_frame_bytes, true)?;
                        frame_release = Some((release_offset, 4));
                        append_aarch64_instructions(&mut bytes, instructions);
                        bytes.extend_from_slice(&0xd65f_03c0_u32.to_le_bytes())
                    }
                }
                affine_cleanup = Some(
                    omega_terminal_machine_code::TerminalUnitAffineCleanupRecord {
                        psi_edge: *psi_edge,
                        structural_types: body.structural_types.clone(),
                        locals: established_affine_locals.clone(),
                        actions: cleanup_actions.clone(),
                        code_offset,
                        byte_count: bytes.len() - code_offset,
                    },
                );
                returned = true;
            }
        }
        let site = match (operation_site, edge_site) {
            (Some(operation), None) => TerminalNativeFuelSite::Operation(operation),
            (None, Some(edge)) => TerminalNativeFuelSite::Edge(edge),
            _ => unreachable!("one Unit operation owns exactly one fuel site"),
        };
        fuel_attribution.push(TerminalNativeFuelAttribution {
            schedule: psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
            site,
            units: 1,
            operation_ordinal,
            code_offset,
            byte_count: bytes.len() - code_offset,
        });
    }
    if !returned {
        return Err(EmissionError::UnitFunctionHasNoReturn);
    }
    Ok(UnitEmission {
        bytes,
        internal_calls,
        internal_unit_calls,
        fuel_attribution,
        port_effects,
        boundary_settlements,
        stack: TerminalUnitStackEvidence {
            frame: match (frame_allocation, frame_release) {
                (
                    Some((allocation_offset, allocation_byte_count)),
                    Some((release_offset, release_byte_count)),
                ) => Some(TerminalStackAdjustmentPair {
                    byte_size: match target.architecture {
                        Architecture::X86_64 => x86_frame_bytes,
                        Architecture::Aarch64 => aarch64_frame_bytes,
                    },
                    allocation_offset,
                    allocation_byte_count,
                    release_offset,
                    release_byte_count,
                }),
                (None, None) => None,
                _ => unreachable!("Unit frame allocation and release are paired"),
            },
            aarch64_return_link: match (aarch64_link_store, aarch64_link_load) {
                (Some(store_offset), Some(load_offset)) => {
                    Some(TerminalAarch64ReturnLinkEvidence {
                        frame_byte_offset: aarch64_lr_offset,
                        store_offset,
                        load_offset,
                    })
                }
                (None, None) => None,
                _ => unreachable!("AArch64 Unit link save and restore are paired"),
            },
            stack_alignment: 16,
        },
        parameter_homes,
        parameters: body
            .parameters
            .iter()
            .map(
                |parameter| omega_terminal_machine_code::TerminalUnitParameterRecord {
                    place: parameter.place,
                    structural_type: parameter.structural_type,
                    multiplicity: parameter.multiplicity,
                    shape: parameter.shape,
                },
            )
            .collect(),
        affine_cleanup,
    })
}

pub(super) fn emit_x86_64_unit_call(
    bytes: &mut Vec<u8>,
    owner: TerminalCallSiteOwner,
    callee: MachineId,
    copies: &[TerminalAssignedAggregateCopy],
    target: NativeTarget,
    homes: &[X86UnitParameterHome],
    internal_calls: &mut Vec<TerminalInternalCallRelocation>,
) -> Result<Vec<(usize, usize, u32, u32)>, EmissionError> {
    let outgoing_bytes = copies
        .iter()
        .map(|copy| outgoing_placement_extent(&copy.destination))
        .try_fold(0_u32, |extent, candidate| {
            candidate.map(|value| extent.max(value))
        })?
        .max(if target.object_format == ObjectFormat::Coff {
            32
        } else {
            0
        });
    // Function-entry RSP is 8 mod 16. Before CALL it must be 0 mod 16.
    let padding = (8 + 16 - (outgoing_bytes % 16)) % 16;
    let call_stack_bytes = outgoing_bytes
        .checked_add(padding)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    let mut allocation = None;
    if call_stack_bytes != 0 {
        let allocation_offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, false);
        allocation = Some((allocation_offset, bytes.len() - allocation_offset));
    }
    let mut argument_intervals = Vec::with_capacity(copies.len());
    for copy in copies {
        let copy_offset = bytes.len();
        let home = homes
            .iter()
            .find(|home| home.place == copy.place)
            .ok_or(EmissionError::MissingUnitParameterHome(copy.place))?;
        if home.source != copy.source
            || copy
                .source_byte_offset
                .checked_add(u32::from(copy.shape.byte_size))
                .is_none_or(|end| end > u32::from(home.shape.byte_size))
        {
            return Err(EmissionError::UnitParameterHomeMismatch(copy.place));
        }
        emit_x86_64_aggregate_copy_from_home(bytes, copy, home, call_stack_bytes)?;
        argument_intervals.push((
            copy_offset,
            bytes.len() - copy_offset,
            home.byte_offset,
            call_stack_bytes,
        ));
    }
    bytes.push(0xe8);
    let offset = bytes.len();
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    let mut release = None;
    if call_stack_bytes != 0 {
        let release_offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, true);
        release = Some((release_offset, bytes.len() - release_offset));
    }
    internal_calls.push(TerminalInternalCallRelocation {
        owner,
        target: callee,
        unit_stack: Some(TerminalUnitCallStackEvidence {
            outbound: stack_adjustment_pair(call_stack_bytes, allocation, release),
        }),
        scalar_stack: None,
        offset,
    });
    Ok(argument_intervals)
}

pub(super) fn emit_aarch64_unit_call(
    bytes: &mut Vec<u8>,
    owner: TerminalCallSiteOwner,
    callee: MachineId,
    copies: &[TerminalAssignedAggregateCopy],
    homes: &[Aarch64UnitParameterHome],
    internal_calls: &mut Vec<TerminalInternalCallRelocation>,
) -> Result<Vec<(usize, usize, u32, u32)>, EmissionError> {
    let outgoing_bytes = copies
        .iter()
        .map(|copy| aarch64_outgoing_placement_extent(&copy.destination))
        .try_fold(0_u32, |extent, candidate| {
            candidate.map(|value| extent.max(value))
        })?;
    let call_stack_bytes = align_u32(outgoing_bytes, 16)?;
    let mut instructions = Vec::new();
    let mut allocation = None;
    if call_stack_bytes != 0 {
        allocation = Some((bytes.len(), 4));
        emit_aarch64_adjust_sp(&mut instructions, call_stack_bytes, false)?;
    }
    let mut argument_intervals = Vec::with_capacity(copies.len());
    for copy in copies {
        let instruction_offset = instructions.len();
        let home = homes
            .iter()
            .find(|home| home.place == copy.place)
            .ok_or(EmissionError::MissingUnitParameterHome(copy.place))?;
        if home.source != copy.source
            || copy
                .source_byte_offset
                .checked_add(u32::from(copy.shape.byte_size))
                .is_none_or(|end| end > u32::from(home.shape.byte_size))
        {
            return Err(EmissionError::UnitParameterHomeMismatch(copy.place));
        }
        emit_aarch64_aggregate_copy_from_home(&mut instructions, copy, home, call_stack_bytes)?;
        argument_intervals.push((
            bytes.len() + instruction_offset * 4,
            (instructions.len() - instruction_offset) * 4,
            home.byte_offset,
            call_stack_bytes,
        ));
    }
    append_aarch64_instructions(bytes, instructions);
    let offset = bytes.len();
    bytes.extend_from_slice(&0x9400_0000_u32.to_le_bytes()); // bl #0
    let mut release = None;
    if call_stack_bytes != 0 {
        let mut instructions = Vec::new();
        release = Some((bytes.len(), 4));
        emit_aarch64_adjust_sp(&mut instructions, call_stack_bytes, true)?;
        append_aarch64_instructions(bytes, instructions);
    }
    internal_calls.push(TerminalInternalCallRelocation {
        owner,
        target: callee,
        unit_stack: Some(TerminalUnitCallStackEvidence {
            outbound: stack_adjustment_pair(call_stack_bytes, allocation, release),
        }),
        scalar_stack: None,
        offset,
    });
    Ok(argument_intervals)
}

fn x86_unit_parameter_homes(
    body: &TerminalAssignedUnitBody,
) -> Result<(Vec<X86UnitParameterHome>, u32), EmissionError> {
    let mut homes = Vec::with_capacity(body.parameters.len());
    let mut cursor = 0_u32;
    for parameter in &body.parameters {
        cursor = align_u32(cursor, 8)?;
        let indirect = matches!(
            parameter.placement.locations.as_slice(),
            [ValueLocation::Indirect { .. }]
        );
        if parameter
            .placement
            .locations
            .iter()
            .any(|location| matches!(location, ValueLocation::Indirect { .. }))
            && !indirect
        {
            return Err(EmissionError::UnsupportedAggregatePlacement);
        }
        let byte_size = if indirect {
            8
        } else {
            u32::from(parameter.shape.byte_size)
        };
        homes.push(X86UnitParameterHome {
            place: parameter.place,
            shape: parameter.shape,
            source: parameter.placement.clone(),
            byte_offset: cursor,
            indirect,
        });
        cursor = cursor
            .checked_add(byte_size)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    }
    Ok((homes, align_u32(cursor, 16)?))
}

fn aarch64_unit_parameter_homes(
    body: &TerminalAssignedUnitBody,
) -> Result<(Vec<Aarch64UnitParameterHome>, u32, u32), EmissionError> {
    let mut homes = Vec::with_capacity(body.parameters.len());
    let mut cursor = 0_u32;
    for parameter in &body.parameters {
        cursor = align_u32(cursor, u32::from(parameter.shape.alignment.clamp(8, 16)))?;
        let indirect = matches!(
            parameter.placement.locations.as_slice(),
            [ValueLocation::Indirect { .. }]
        );
        if parameter
            .placement
            .locations
            .iter()
            .any(|location| matches!(location, ValueLocation::Indirect { .. }))
            && !indirect
        {
            return Err(EmissionError::UnsupportedAggregatePlacement);
        }
        let byte_size = if indirect {
            8
        } else {
            u32::from(parameter.shape.byte_size)
        };
        homes.push(Aarch64UnitParameterHome {
            place: parameter.place,
            shape: parameter.shape,
            source: parameter.placement.clone(),
            byte_offset: cursor,
            indirect,
        });
        cursor = cursor
            .checked_add(byte_size)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    }
    let lr_offset = align_u32(cursor, 8)?;
    let frame_bytes = lr_offset
        .checked_add(8)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)
        .and_then(|size| align_u32(size, 16))?;
    Ok((homes, frame_bytes, lr_offset))
}

fn emit_aarch64_stage_unit_parameters(
    instructions: &mut Vec<u32>,
    homes: &[Aarch64UnitParameterHome],
    frame_bytes: u32,
) -> Result<(), EmissionError> {
    for home in homes {
        if home.indirect {
            let [ValueLocation::Indirect { pointer, .. }] = home.source.locations.as_slice() else {
                return Err(EmissionError::UnsupportedAggregatePlacement);
            };
            match *pointer {
                IndirectPointerLocation::Register(register) => {
                    instructions.push(aarch64_unit_stack_access(
                        0xf900_0000,
                        aarch64_unit_register(register)?,
                        home.byte_offset,
                        8,
                    )?)
                }
                IndirectPointerLocation::Stack {
                    stack_byte_offset, ..
                } => {
                    let incoming = frame_bytes
                        .checked_add(stack_byte_offset)
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    instructions.push(aarch64_unit_stack_access(0xf940_0000, 9, incoming, 8)?);
                    instructions.push(aarch64_unit_stack_access(
                        0xf900_0000,
                        9,
                        home.byte_offset,
                        8,
                    )?);
                }
            }
            continue;
        }
        for source in &home.source.locations {
            let (value_offset, byte_size) = placement_fragment(source)?;
            let destination = home
                .byte_offset
                .checked_add(u32::from(value_offset))
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
            match *source {
                ValueLocation::Register { register, .. } => {
                    instructions.push(aarch64_unit_stack_access(
                        aarch64_store_base(byte_size)?,
                        aarch64_unit_register(register)?,
                        destination,
                        byte_size,
                    )?)
                }
                ValueLocation::Stack {
                    stack_byte_offset, ..
                } => {
                    let incoming = frame_bytes
                        .checked_add(stack_byte_offset)
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    instructions.push(aarch64_unit_stack_access(
                        aarch64_load_base(byte_size)?,
                        9,
                        incoming,
                        byte_size,
                    )?);
                    instructions.push(aarch64_unit_stack_access(
                        aarch64_store_base(byte_size)?,
                        9,
                        destination,
                        byte_size,
                    )?);
                }
                ValueLocation::Indirect { .. } => unreachable!(),
            }
        }
    }
    Ok(())
}

fn align_u32(value: u32, alignment: u32) -> Result<u32, EmissionError> {
    let remainder = value % alignment;
    if remainder == 0 {
        return Ok(value);
    }
    value
        .checked_add(alignment - remainder)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)
}

fn emit_x86_64_stage_unit_parameters(
    bytes: &mut Vec<u8>,
    homes: &[X86UnitParameterHome],
    frame_bytes: u32,
) -> Result<(), EmissionError> {
    for home in homes {
        if home.indirect {
            let [ValueLocation::Indirect { pointer, .. }] = home.source.locations.as_slice() else {
                return Err(EmissionError::UnsupportedAggregatePlacement);
            };
            match *pointer {
                IndirectPointerLocation::Register(register) => emit_x86_64_stack_store_width(
                    bytes,
                    x86_unit_register(register)?,
                    home.byte_offset,
                    8,
                )?,
                IndirectPointerLocation::Stack {
                    stack_byte_offset, ..
                } => {
                    let incoming = frame_bytes
                        .checked_add(8)
                        .and_then(|offset| offset.checked_add(stack_byte_offset))
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    emit_x86_64_stack_load_width(bytes, 0, incoming, 8)?;
                    emit_x86_64_stack_store_width(bytes, 0, home.byte_offset, 8)?;
                }
            }
            continue;
        }
        for source in &home.source.locations {
            let (value_offset, byte_size) = placement_fragment(source)?;
            let destination = home
                .byte_offset
                .checked_add(u32::from(value_offset))
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
            match *source {
                ValueLocation::Register { register, .. } => emit_x86_64_stack_store_width(
                    bytes,
                    x86_unit_register(register)?,
                    destination,
                    byte_size,
                )?,
                ValueLocation::Stack {
                    stack_byte_offset, ..
                } => {
                    let incoming = frame_bytes
                        .checked_add(8)
                        .and_then(|offset| offset.checked_add(stack_byte_offset))
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    emit_x86_64_stack_load_width(bytes, 0, incoming, byte_size)?;
                    emit_x86_64_stack_store_width(bytes, 0, destination, byte_size)?;
                }
                ValueLocation::Indirect { .. } => unreachable!(),
            }
        }
    }
    Ok(())
}

fn outgoing_placement_extent(placement: &ValuePlacement) -> Result<u32, EmissionError> {
    placement
        .locations
        .iter()
        .try_fold(0_u32, |extent, location| {
            let end = match *location {
                ValueLocation::Register { .. } => 0,
                ValueLocation::Stack {
                    stack_byte_offset,
                    byte_size,
                    ..
                } => stack_byte_offset
                    .checked_add(u32::from(byte_size.max(8)))
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
                // Forwarding a by-value structural argument may reuse the exact
                // caller-owned copy. Only a stack-resident pointer needs outgoing
                // space; no second aggregate copy is fabricated.
                ValueLocation::Indirect { pointer, .. } => match pointer {
                    IndirectPointerLocation::Register(_) => 0,
                    IndirectPointerLocation::Stack {
                        stack_byte_offset, ..
                    } => stack_byte_offset
                        .checked_add(8)
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
                },
            };
            Ok(extent.max(end))
        })
}

fn aarch64_outgoing_placement_extent(placement: &ValuePlacement) -> Result<u32, EmissionError> {
    placement
        .locations
        .iter()
        .try_fold(0_u32, |extent, location| {
            let end = match *location {
                ValueLocation::Register { .. } => 0,
                ValueLocation::Stack {
                    stack_byte_offset,
                    byte_size,
                    ..
                } => stack_byte_offset
                    .checked_add(u32::from(byte_size.max(8)))
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size,
                    ..
                } => {
                    let pointer_end = match pointer {
                        IndirectPointerLocation::Register(_) => 0,
                        IndirectPointerLocation::Stack {
                            stack_byte_offset, ..
                        } => stack_byte_offset
                            .checked_add(8)
                            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
                    };
                    let copy_end = copy_stack_byte_offset
                        .ok_or(EmissionError::UnsupportedAggregatePlacement)?
                        .checked_add(u32::from(byte_size).next_multiple_of(8))
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    pointer_end.max(copy_end)
                }
            };
            Ok(extent.max(end))
        })
}

fn emit_x86_64_aggregate_copy_from_home(
    bytes: &mut Vec<u8>,
    copy: &TerminalAssignedAggregateCopy,
    home: &X86UnitParameterHome,
    call_stack_bytes: u32,
) -> Result<(), EmissionError> {
    if home.indirect {
        if !copy.path.is_empty() {
            if copy
                .destination
                .locations
                .iter()
                .any(|location| matches!(location, ValueLocation::Indirect { .. }))
            {
                return Err(EmissionError::UnsupportedAggregatePlacement);
            }
            let pointer_home = call_stack_bytes
                .checked_add(home.byte_offset)
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
            emit_x86_64_stack_load_width(bytes, 11, pointer_home, 8)?;
            for destination in &copy.destination.locations {
                let (value_offset, width) = placement_fragment(destination)?;
                let source_offset = copy
                    .source_byte_offset
                    .checked_add(u32::from(value_offset))
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                match *destination {
                    ValueLocation::Register { register, .. } => {
                        emit_x86_64_memory_load_width(
                            bytes,
                            x86_unit_register(register)?,
                            11,
                            source_offset,
                            width,
                        )?;
                    }
                    ValueLocation::Stack {
                        stack_byte_offset, ..
                    } => {
                        emit_x86_64_memory_load_width(bytes, 0, 11, source_offset, width)?;
                        emit_x86_64_stack_store_width(bytes, 0, stack_byte_offset, width)?;
                    }
                    ValueLocation::Indirect { .. } => unreachable!(),
                }
            }
            return Ok(());
        }
        if copy.shape != home.shape {
            return Err(EmissionError::UnsupportedAggregatePlacement);
        }
        let [ValueLocation::Indirect { pointer, .. }] = copy.destination.locations.as_slice()
        else {
            return Err(EmissionError::UnsupportedAggregatePlacement);
        };
        let source_offset = call_stack_bytes
            .checked_add(home.byte_offset)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
        return match *pointer {
            IndirectPointerLocation::Register(register) => {
                emit_x86_64_stack_load_width(bytes, x86_unit_register(register)?, source_offset, 8)
            }
            IndirectPointerLocation::Stack {
                stack_byte_offset, ..
            } => {
                emit_x86_64_stack_load_width(bytes, 0, source_offset, 8)?;
                emit_x86_64_stack_store_width(bytes, 0, stack_byte_offset, 8)
            }
        };
    }
    if copy
        .destination
        .locations
        .iter()
        .any(|location| matches!(location, ValueLocation::Indirect { .. }))
    {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    }
    for destination in &copy.destination.locations {
        let (destination_offset, destination_size) = placement_fragment(destination)?;
        let source_offset = call_stack_bytes
            .checked_add(home.byte_offset)
            .and_then(|offset| offset.checked_add(copy.source_byte_offset))
            .and_then(|offset| offset.checked_add(u32::from(destination_offset)))
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
        match *destination {
            ValueLocation::Register { register, .. } => {
                let destination_register = x86_unit_register(register)?;
                emit_x86_64_stack_load_width(
                    bytes,
                    destination_register,
                    source_offset,
                    destination_size,
                )?;
            }
            ValueLocation::Stack {
                stack_byte_offset, ..
            } => {
                emit_x86_64_stack_load_width(bytes, 0, source_offset, destination_size)?;
                emit_x86_64_stack_store_width(bytes, 0, stack_byte_offset, destination_size)?;
            }
            ValueLocation::Indirect { .. } => unreachable!(),
        }
    }
    Ok(())
}

fn emit_aarch64_aggregate_copy_from_home(
    instructions: &mut Vec<u32>,
    copy: &TerminalAssignedAggregateCopy,
    home: &Aarch64UnitParameterHome,
    call_stack_bytes: u32,
) -> Result<(), EmissionError> {
    if home.indirect {
        if !copy.path.is_empty() {
            if copy
                .destination
                .locations
                .iter()
                .any(|location| matches!(location, ValueLocation::Indirect { .. }))
            {
                return Err(EmissionError::UnsupportedAggregatePlacement);
            }
            let pointer_home = call_stack_bytes
                .checked_add(home.byte_offset)
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
            instructions.push(aarch64_unit_stack_access(0xf940_0000, 9, pointer_home, 8)?);
            for destination in &copy.destination.locations {
                let (value_offset, width) = placement_fragment(destination)?;
                let source_offset = copy
                    .source_byte_offset
                    .checked_add(u32::from(value_offset))
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                match *destination {
                    ValueLocation::Register { register, .. } => {
                        instructions.push(aarch64_unit_memory_access(
                            aarch64_load_base(width)?,
                            aarch64_unit_register(register)?,
                            9,
                            source_offset,
                            width,
                        )?)
                    }
                    ValueLocation::Stack {
                        stack_byte_offset, ..
                    } => {
                        instructions.push(aarch64_unit_memory_access(
                            aarch64_load_base(width)?,
                            10,
                            9,
                            source_offset,
                            width,
                        )?);
                        instructions.push(aarch64_unit_stack_access(
                            aarch64_store_base(width)?,
                            10,
                            stack_byte_offset,
                            width,
                        )?);
                    }
                    ValueLocation::Indirect { .. } => unreachable!(),
                }
            }
            return Ok(());
        }
        if copy.shape != home.shape {
            return Err(EmissionError::UnsupportedAggregatePlacement);
        }
        let [
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset: Some(copy_stack_byte_offset),
                byte_size,
                ..
            },
        ] = copy.destination.locations.as_slice()
        else {
            return Err(EmissionError::UnsupportedAggregatePlacement);
        };
        let source_offset = call_stack_bytes
            .checked_add(home.byte_offset)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
        instructions.push(aarch64_unit_stack_access(0xf940_0000, 9, source_offset, 8)?);
        let mut copied = 0_u32;
        while copied < u32::from(*byte_size) {
            let remaining = u32::from(*byte_size) - copied;
            let width = if remaining >= 8 {
                8_u16
            } else if remaining >= 4 {
                4
            } else if remaining >= 2 {
                2
            } else {
                1
            };
            instructions.push(aarch64_unit_memory_access(
                aarch64_load_base(width)?,
                10,
                9,
                copied,
                width,
            )?);
            instructions.push(aarch64_unit_stack_access(
                aarch64_store_base(width)?,
                10,
                copy_stack_byte_offset
                    .checked_add(copied)
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
                width,
            )?);
            copied += u32::from(width);
        }
        match *pointer {
            IndirectPointerLocation::Register(register) => {
                emit_aarch64_sp_address(
                    instructions,
                    aarch64_unit_register(register)?,
                    *copy_stack_byte_offset,
                )?;
            }
            IndirectPointerLocation::Stack {
                stack_byte_offset, ..
            } => {
                emit_aarch64_sp_address(instructions, 10, *copy_stack_byte_offset)?;
                instructions.push(aarch64_unit_stack_access(
                    0xf900_0000,
                    10,
                    stack_byte_offset,
                    8,
                )?);
            }
        }
        return Ok(());
    }
    if copy
        .destination
        .locations
        .iter()
        .any(|location| matches!(location, ValueLocation::Indirect { .. }))
    {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    }
    for destination in &copy.destination.locations {
        let (destination_offset, destination_size) = placement_fragment(destination)?;
        let source_offset = call_stack_bytes
            .checked_add(home.byte_offset)
            .and_then(|offset| offset.checked_add(copy.source_byte_offset))
            .and_then(|offset| offset.checked_add(u32::from(destination_offset)))
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
        match *destination {
            ValueLocation::Register { register, .. } => {
                instructions.push(aarch64_unit_stack_access(
                    aarch64_load_base(destination_size)?,
                    aarch64_unit_register(register)?,
                    source_offset,
                    destination_size,
                )?)
            }
            ValueLocation::Stack {
                stack_byte_offset, ..
            } => {
                instructions.push(aarch64_unit_stack_access(
                    aarch64_load_base(destination_size)?,
                    9,
                    source_offset,
                    destination_size,
                )?);
                instructions.push(aarch64_unit_stack_access(
                    aarch64_store_base(destination_size)?,
                    9,
                    stack_byte_offset,
                    destination_size,
                )?);
            }
            ValueLocation::Indirect { .. } => unreachable!(),
        }
    }
    Ok(())
}
