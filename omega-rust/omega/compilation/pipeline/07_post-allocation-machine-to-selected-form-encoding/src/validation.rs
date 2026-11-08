/*
We independently replay and validate a `SelectedFormEncoding` against its
selected-instruction, post-allocation-machine, physical-register-model, and
frame-layout roots (`:IndependentByteReplay`).

After checking root digests and frame geometry (`frame_address::validate_frame_root`),
`validate_ordinary` walks the selected-to-machine instruction correspondence
(`for_each_row`) in lockstep with the artifact's `SelectedFormEncodingRow`
table. For each row, we first verify the frame-space address equation
(`frame_address::validate_address`) and then dispatch `validate_row` on the
selected instruction kind using our own kind-to-address-operation matching
(`admits_address_operation`) rather than trusting the producer's route table:
- Memory, frame, hosted byte-channel, and ordinary scalar forms are decoded by
  the target ISA's independent byte validators (`isa_x86_64` / `isa_aarch64`),
  and `admit_decoded_footprint` checks that the decoded register reads, writes,
  implicit unit effects, and byte length match both the machine alternative and
  the interned `SelectedFormDecodedFootprint`.
- Internal scalar calls (`validate_scalar_call`) and normalized foreign calls
  (`validate_normalized_foreign_call`) re-verify their call templates, import
  fixups, and callback materialization records against the ISA validators.
- Deferred control-flow rows must carry `RequiresResolvedBranchLayout` and no
  resolved address.
- Finally, `validate_aggregate` recomputes the row-state counters (`counts_of`)
  and the canonical `SelectedFormEncodingIdentity` digest.
*/

use isa_aarch64::{
    Aarch64CallbackAddressMaterialization, Aarch64CallbackMaterializationDestination,
    Aarch64NormalizedForeignCallFixup, Aarch64NormalizedForeignCallFixupKind,
    Aarch64NormalizedForeignCallFixupState, Aarch64ScalarCallFixup, Aarch64ScalarCallFixupKind,
    Aarch64ScalarCallFixupState, validate_aarch64_selected_form_encoding,
    validate_aarch64_selected_normalized_foreign_call_template,
    validate_aarch64_selected_scalar_call_template,
};
use isa_x86_64::{
    X86_64CallbackAddressMaterialization, X86_64CallbackMaterializationDestination,
    X86_64NormalizedForeignCallFixup, X86_64NormalizedForeignCallFixupKind,
    X86_64NormalizedForeignCallFixupState, X86_64ScalarCallFixup, X86_64ScalarCallFixupKind,
    X86_64ScalarCallFixupState, validate_x86_64_selected_form_encoding,
    validate_x86_64_selected_normalized_foreign_call_template,
    validate_x86_64_selected_scalar_call_template,
};
use machine_code::{
    DeferredControlEncodingReason, ImageAddressFields, SelectedFormDecodedFootprint,
    SelectedFormEncodedRowStore, SelectedFormEncoding, SelectedFormEncodingRow,
    SelectedFormEncodingState, SelectedFormInternalMachineFixupKind,
    SelectedFormInternalMachineFixupState, SelectedFormMachineDisposition,
    SelectedFormNormalizedForeignCallFixupKind, SelectedFormNormalizedForeignCallFixupState,
    TargetFrameLayoutPlan,
};
use physical_instructions::{PostAllocationMachineInstruction, PostAllocationMachinePlan};
use register_model::{RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    CurrentSelectedProgram, MachineEncodedEffects, MachineSizeKnowledge, SelectedFunction,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
};
use target::{Architecture, NativeTarget};

use crate::OptimizedSelectedFormEncodingError as Error;
use crate::row_encoding::normalized_foreign_call_record;
use crate::{counts_of, for_each_row, frame_address};

macro_rules! decoded_row_footprint {
    ($decoded:expr, $map:expr) => {{
        let decoded = $decoded.map_err($map)?;
        decoded_footprint(
            &decoded.footprint().register_reads,
            &decoded.footprint().register_writes,
            &decoded.footprint().encoded,
        )
    }};
}

pub(super) fn validate(
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&TargetFrameLayoutPlan>,
    artifact: &SelectedFormEncoding,
) -> Result<(), Error> {
    frame_address::validate_frame_root(machine, frame)?;
    if artifact.frame.as_ref() != frame {
        return Err(Error::ArtifactMismatch);
    }
    if machine.selected != selected.identity || artifact.selected != selected.identity {
        return Err(Error::SelectedRootMismatch);
    }
    if machine.physical_register_model != physical.identity() {
        return Err(Error::PhysicalModelMismatch);
    }
    if artifact.machine != machine.identity {
        return Err(Error::ArtifactMismatch);
    }
    if artifact.post_allocation_machine_optimization.is_some() {
        return Err(Error::ArtifactMismatch);
    }

    validate_ordinary(
        selected,
        machine,
        physical,
        frame,
        artifact.rows(),
        artifact.encoded(),
    )?;
    validate_aggregate(artifact)
}

fn validate_ordinary(
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&TargetFrameLayoutPlan>,
    rows: &[SelectedFormEncodingRow],
    encoded: &SelectedFormEncodedRowStore,
) -> Result<(), Error> {
    let target = selected.plan.target;
    let mut candidate_rows = rows.iter();
    for_each_row(
        selected,
        machine,
        |selected_function, selected_instruction, machine_function, machine_instruction| {
            let candidate = candidate_rows
                .next()
                .ok_or(Error::InstructionRosterMismatch)?;
            frame_address::validate_address(
                machine_function,
                frame,
                machine_instruction,
                candidate.address(),
            )?;
            validate_row(
                target,
                selected_function,
                selected_instruction,
                machine_instruction,
                physical,
                candidate,
                encoded,
            )
        },
    )?;
    if candidate_rows.next().is_some() {
        return Err(Error::InstructionRosterMismatch);
    }
    Ok(())
}

fn validate_aggregate(artifact: &SelectedFormEncoding) -> Result<(), Error> {
    if artifact.counts != counts_of(artifact.rows())? {
        return Err(Error::ArtifactMismatch);
    }
    let identity = artifact.recomputed_identity();
    if artifact.identity != identity {
        return Err(Error::ArtifactMismatch);
    }
    Ok(())
}

pub(crate) fn validate_row(
    target: NativeTarget,
    selected_function: &SelectedFunction,
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    row: &SelectedFormEncodingRow,
    encoded: &SelectedFormEncodedRowStore,
) -> Result<(), Error> {
    let architecture = target.architecture;
    if row.instruction != selected.id
        || row.alternative != machine.alternative.key
        || row.machine_disposition != SelectedFormMachineDisposition::RetainedV1
    {
        return Err(Error::ArtifactMismatch);
    }
    match selected.kind {
        kind @ (SelectedInstructionKind::AccessorScalarAccess { .. }
        | SelectedInstructionKind::Store { .. }
        | SelectedInstructionKind::AddressOffset { .. }
        | SelectedInstructionKind::Load64 { .. }
        | SelectedInstructionKind::LoadPacked { .. }
        | SelectedInstructionKind::StorePacked { .. }
        | selected_instructions::load_width_triple_pattern!(..)
        | SelectedInstructionKind::HostedWriteByteI32 { .. }
        | SelectedInstructionKind::HostedReadByte { .. }
        | SelectedInstructionKind::Load8Indexed
        | SelectedInstructionKind::SaveFloatingControl { .. }
        | SelectedInstructionKind::RestoreFloatingControl { .. }
        | SelectedInstructionKind::Store64 { .. }
        | SelectedInstructionKind::FrameAddress { .. }) => {
            let address = row.address().ok_or(Error::ArtifactMismatch)?;
            if !admits_address_operation(kind, address.symbolic) {
                return Err(Error::ArtifactMismatch);
            }
            let (bytes, footprint) = encoded_row(row, encoded)?;
            let decoded = if architecture == Architecture::Aarch64 {
                let encoded = if matches!(kind, SelectedInstructionKind::HostedWriteByteI32 { .. })
                {
                    isa_aarch64::validate_aarch64_selected_hosted_write_byte_form(
                        target,
                        physical,
                        kind,
                        machine.alternative.key,
                        &operand_views(machine),
                        address.displacement,
                        bytes,
                    )
                } else if matches!(kind, SelectedInstructionKind::HostedReadByte { .. }) {
                    isa_aarch64::validate_aarch64_selected_hosted_read_byte_form(
                        target,
                        physical,
                        kind,
                        machine.alternative.key,
                        &operand_views(machine),
                        address.displacement,
                        bytes,
                    )
                } else if matches!(
                    kind,
                    SelectedInstructionKind::FrameAddress { .. }
                        | SelectedInstructionKind::Store64 { .. }
                ) {
                    isa_aarch64::validate_aarch64_selected_frame_form(
                        physical,
                        kind,
                        machine.alternative.key,
                        &operand_views(machine),
                        u32::try_from(address.displacement).map_err(|_| Error::ArtifactMismatch)?,
                        address.frame_alignment_bytes,
                        bytes,
                    )
                } else {
                    isa_aarch64::validate_aarch64_selected_memory_form(
                        physical,
                        kind,
                        machine.alternative.key,
                        &operand_views(machine),
                        address.displacement,
                        bytes,
                    )
                };
                decoded_row_footprint!(encoded, Error::Aarch64)
            } else {
                let encoded = if matches!(kind, SelectedInstructionKind::HostedWriteByteI32 { .. })
                {
                    isa_x86_64::validate_x86_64_selected_hosted_write_byte_form(
                        target,
                        physical,
                        kind,
                        machine.alternative.key,
                        &operand_views(machine),
                        address.displacement,
                        bytes,
                    )
                } else if matches!(kind, SelectedInstructionKind::HostedReadByte { .. }) {
                    isa_x86_64::validate_x86_64_selected_hosted_read_byte_form(
                        target,
                        physical,
                        kind,
                        machine.alternative.key,
                        &operand_views(machine),
                        address.displacement,
                        bytes,
                    )
                } else if matches!(
                    kind,
                    SelectedInstructionKind::FrameAddress { .. }
                        | SelectedInstructionKind::Store64 { .. }
                ) {
                    isa_x86_64::validate_x86_64_selected_frame_form(
                        physical,
                        kind,
                        machine.alternative.key,
                        &operand_views(machine),
                        address.displacement,
                        address.frame_alignment_bytes,
                        bytes,
                    )
                } else {
                    isa_x86_64::validate_x86_64_selected_memory_form(
                        physical,
                        kind,
                        machine.alternative.key,
                        &operand_views(machine),
                        address.displacement,
                        bytes,
                    )
                };
                decoded_row_footprint!(encoded, Error::X86_64)
            };
            admit_decoded_footprint(selected.id, machine, bytes.len(), footprint, &decoded)?;
            Ok(())
        }
        kind @ (selected_instructions::selected_call_pattern!(..)) => {
            if row.address.is_some() {
                return Err(Error::ArtifactMismatch);
            }
            validate_scalar_call(target, selected.id, kind, machine, physical, row, encoded)
        }
        kind @ SelectedInstructionKind::NormalizedForeignCall { .. } => {
            if row.address.is_some() {
                return Err(Error::ArtifactMismatch);
            }
            validate_normalized_foreign_call(
                target,
                selected_function,
                selected.id,
                kind,
                machine,
                physical,
                row,
                encoded,
            )
        }
        SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan
        | SelectedInstructionKind::Jump => {
            if row.address.is_some()
                || row.state
                    != (SelectedFormEncodingState::DeferredControl {
                        reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
                    })
            {
                return Err(Error::ArtifactMismatch);
            }
            Ok(())
        }
        kind @ SelectedInstructionKind::ImageAddress { reference } => {
            let fields = match architecture {
                Architecture::X86_64 => ImageAddressFields::X86_64Relative32 {
                    patch_offset: u64::from(isa_x86_64::X86_64_BYTE_LITERAL_ADDRESS_FIELD_OFFSET),
                },
                Architecture::Aarch64 => ImageAddressFields::Aarch64PageAddress {
                    page_patch_offset: u64::from(
                        isa_aarch64::AARCH64_BYTE_LITERAL_ADDRESS_PAGE_FIELD_OFFSET,
                    ),
                    page_offset_patch_offset: u64::from(
                        isa_aarch64::AARCH64_BYTE_LITERAL_ADDRESS_PAGE_OFFSET_FIELD_OFFSET,
                    ),
                },
            };
            let matched = matches!(&row.state,
                SelectedFormEncodingState::UnresolvedImageAddress { fixup }
                    if fixup.reference == reference && fixup.fields == fields);
            if !matched {
                return Err(Error::ArtifactMismatch);
            }
            let retagged = SelectedFormEncodingRow {
                state: SelectedFormEncodingState::Encoded,
                ..row.clone()
            };
            validate_baseline(
                target,
                selected.id,
                kind,
                machine,
                physical,
                &retagged,
                encoded,
            )
        }
        kind => validate_baseline(target, selected.id, kind, machine, physical, row, encoded),
    }
}

fn admits_address_operation(
    kind: SelectedInstructionKind,
    operation: physical_instructions::PhysicalAddressOperation,
) -> bool {
    use SelectedInstructionKind as Kind;
    use physical_instructions::PhysicalAddressOperation as Operation;
    if let (
        Kind::AccessorScalarAccess {
            form,
            contract,
            byte_offset,
        },
        Operation::AccessorScalarAccess {
            base_operand: 0,
            form: actual_form,
            contract: actual_contract,
            byte_offset: actual_offset,
        },
    ) = (kind, operation)
    {
        return form == actual_form
            && contract == actual_contract
            && byte_offset == actual_offset
            && form.ordinary_semantic().is_some();
    }
    matches!(
        (kind, operation),
        (Kind::Store { .. }, Operation::Store { .. })
            | (Kind::AddressOffset { .. }, Operation::AddressOffset { .. })
            | (Kind::Load64 { .. }, Operation::Load64 { .. })
            | (Kind::LoadPacked { .. }, Operation::LoadPacked { .. })
            | (Kind::StorePacked { .. }, Operation::StorePacked { .. })
            | (Kind::Load8 { .. }, Operation::Load8 { .. })
            | (Kind::Load16 { .. }, Operation::Load16 { .. })
            | (Kind::Load32 { .. }, Operation::Load32 { .. })
            | (Kind::Load8Indexed, Operation::Load8Indexed { .. })
            | (
                Kind::HostedReadByte { .. },
                Operation::HostedReadByte { .. }
            )
            | (
                Kind::HostedWriteByteI32 { .. },
                Operation::HostedWriteByteI32 { .. }
            )
            | (
                Kind::SaveFloatingControl { .. },
                Operation::SaveFloatingControl { .. }
            )
            | (
                Kind::RestoreFloatingControl { .. },
                Operation::RestoreFloatingControl { .. }
            )
            | (Kind::Store64 { .. }, Operation::Store64 { .. })
            | (Kind::FrameAddress { .. }, Operation::FrameAddress { .. })
    )
}

fn validate_baseline(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    row: &SelectedFormEncodingRow,
    encoded: &SelectedFormEncodedRowStore,
) -> Result<(), Error> {
    if row.address.is_some() {
        return Err(Error::ArtifactMismatch);
    }
    let (bytes, footprint) = encoded_row(row, encoded)?;
    let views = operand_views(machine);
    let decoded = match target.architecture {
        Architecture::X86_64 => {
            let decoded = if kind == SelectedInstructionKind::HostedExitProcessI32 {
                isa_x86_64::validate_x86_64_selected_hosted_exit_process_form(
                    target,
                    physical,
                    kind,
                    machine.alternative.key,
                    &views,
                    bytes,
                )
            } else {
                validate_x86_64_selected_form_encoding(
                    physical,
                    kind,
                    machine.alternative.key,
                    &views,
                    bytes,
                )
            };
            decoded_row_footprint!(decoded, |_| { Error::ArtifactMismatch })
        }
        Architecture::Aarch64 => {
            let decoded = if kind == SelectedInstructionKind::HostedExitProcessI32 {
                isa_aarch64::validate_aarch64_selected_hosted_exit_process_form(
                    target,
                    physical,
                    kind,
                    machine.alternative.key,
                    &views,
                    bytes,
                )
            } else {
                validate_aarch64_selected_form_encoding(
                    physical,
                    kind,
                    machine.alternative.key,
                    &views,
                    bytes,
                )
            };
            decoded_row_footprint!(decoded, |_| { Error::ArtifactMismatch })
        }
    };
    admit_decoded_footprint(instruction, machine, bytes.len(), footprint, &decoded)?;
    Ok(())
}

fn decoded_footprint(
    reads: &[RegisterViewId],
    writes: &[RegisterViewId],
    encoded: &MachineEncodedEffects,
) -> SelectedFormDecodedFootprint {
    SelectedFormDecodedFootprint {
        register_reads: reads.to_vec(),
        register_writes: writes.to_vec(),
        implicit_defs: encoded.implicit_unit_defs.clone(),
        implicit_clobbers: encoded.implicit_unit_clobbers.clone(),
        encoded: encoded.clone(),
    }
}

fn validate_machine_footprint(
    instruction: SelectedInstructionId,
    machine: &PostAllocationMachineInstruction,
    decoded: &SelectedFormDecodedFootprint,
) -> Result<(), Error> {
    validate_external_operands(instruction, machine, decoded)?;
    if decoded.encoded != machine.alternative.encoded {
        return Err(Error::ImplicitFootprintMismatch(instruction));
    }
    Ok(())
}

fn encoded_row<'a>(
    row: &SelectedFormEncodingRow,
    encoded: &'a SelectedFormEncodedRowStore,
) -> Result<(&'a [u8], &'a SelectedFormDecodedFootprint), Error> {
    if !matches!(row.state, SelectedFormEncodingState::Encoded) {
        return Err(Error::ArtifactMismatch);
    }
    encoded.encoded(row).ok_or(Error::ArtifactMismatch)
}

fn admit_decoded_footprint(
    instruction: SelectedInstructionId,
    machine: &PostAllocationMachineInstruction,
    bytes: usize,
    retained: &SelectedFormDecodedFootprint,
    decoded: &SelectedFormDecodedFootprint,
) -> Result<(), Error> {
    validate_machine_footprint(instruction, machine, decoded)?;
    validate_size(instruction, machine.alternative.size, bytes)?;
    if retained != decoded {
        return Err(Error::ArtifactMismatch);
    }
    Ok(())
}

fn decoded_effects_footprint(
    machine: &PostAllocationMachineInstruction,
    effects: &MachineEncodedEffects,
) -> Result<SelectedFormDecodedFootprint, Error> {
    Ok(decoded_footprint(
        &views_for_effects(machine, effects, true)?,
        &views_for_effects(machine, effects, false)?,
        effects,
    ))
}

fn validate_external_operands(
    instruction: SelectedInstructionId,
    machine: &PostAllocationMachineInstruction,
    decoded: &SelectedFormDecodedFootprint,
) -> Result<(), Error> {
    if decoded.register_reads != views_for_effects(machine, &decoded.encoded, true)?
        || decoded.register_writes != views_for_effects(machine, &decoded.encoded, false)?
    {
        return Err(Error::OperandFootprintMismatch(instruction));
    }
    Ok(())
}

pub(crate) fn validate_size(
    instruction: SelectedInstructionId,
    knowledge: MachineSizeKnowledge,
    actual: usize,
) -> Result<(), Error> {
    let actual = u16::try_from(actual).map_err(|_| Error::SizeDeclarationMismatch(instruction))?;
    let matches = match knowledge {
        MachineSizeKnowledge::ExactBytes(expected) => actual == expected,
        MachineSizeKnowledge::EncoderResolved {
            minimum_bytes,
            maximum_bytes,
        } => actual >= minimum_bytes && maximum_bytes.is_none_or(|maximum| actual <= maximum),
    };
    if !matches {
        return Err(Error::SizeDeclarationMismatch(instruction));
    }
    Ok(())
}

fn operand_views(machine: &PostAllocationMachineInstruction) -> Vec<RegisterViewId> {
    machine
        .operands
        .iter()
        .map(|operand| operand.view)
        .collect()
}

fn operand_accesses(machine: &PostAllocationMachineInstruction) -> Vec<RegisterOperandAccess> {
    machine
        .operands
        .iter()
        .map(|operand| operand.access)
        .collect()
}

fn views_for_effects(
    machine: &PostAllocationMachineInstruction,
    effects: &MachineEncodedEffects,
    reads: bool,
) -> Result<Vec<RegisterViewId>, Error> {
    let resolve = |operand: u16| machine.operand_footprint(operand).map(|row| row.view);
    let operands = if reads {
        &effects.external_operand_reads
    } else {
        &effects.external_operand_writes
    };
    operands
        .iter()
        .map(|operand| resolve(*operand))
        .collect::<Option<Vec<_>>>()
        .ok_or(Error::OperandFootprintMismatch(machine.instruction))
}

fn validate_scalar_call(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    row: &SelectedFormEncodingRow,
    encoded: &SelectedFormEncodedRowStore,
) -> Result<(), Error> {
    if matches!(
        kind.call_target(),
        Some(selected_instructions::SelectedCallTarget::Dynamic { .. })
    ) {
        let (bytes, footprint) = encoded_row(row, encoded)?;
        let views = operand_views(machine);
        let (expected_bytes, effects) = match target.architecture {
            Architecture::Aarch64 => isa_aarch64::encode_aarch64_selected_dynamic_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &machine.alternative.encoded,
            )
            .map_err(Error::Aarch64ScalarCall)?,
            Architecture::X86_64 => isa_x86_64::encode_x86_64_selected_dynamic_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &machine.alternative.encoded,
            )
            .map_err(Error::X86_64ScalarCall)?,
        };
        let decoded = decoded_effects_footprint(machine, &effects)?;
        if bytes != expected_bytes || footprint != &decoded {
            return Err(Error::ArtifactMismatch);
        }
        validate_machine_footprint(instruction, machine, &decoded)?;
        validate_size(instruction, machine.alternative.size, bytes.len())?;
        return Ok(());
    }
    let SelectedFormEncodingState::UnresolvedInternalMachineCall { fixup } = &row.state else {
        return Err(Error::ArtifactMismatch);
    };
    let Some((bytes, footprint)) = encoded.encoded(row) else {
        return Err(Error::ArtifactMismatch);
    };
    if fixup.state != SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1
        || fixup.addend != 0
    {
        return Err(Error::ArtifactMismatch);
    }
    let views = operand_views(machine);
    let decoded = match target.architecture {
        Architecture::X86_64 => {
            let target_kind = match fixup.kind {
                SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1 => {
                    X86_64ScalarCallFixupKind::Relative32FromNextInstructionToInternalMachineV1
                }
                _ => return Err(Error::ArtifactMismatch),
            };
            let validated = validate_x86_64_selected_scalar_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &machine.alternative.encoded,
                bytes,
                X86_64ScalarCallFixup {
                    kind: target_kind,
                    state: X86_64ScalarCallFixupState::UnresolvedZeroFieldV1,
                    callee: fixup.callee,
                    opcode_byte_offset: fixup.opcode_row_offset,
                    patch_byte_offset: fixup.patch_row_offset,
                    reference_byte_offset: fixup.reference_row_offset,
                    patch_byte_width: fixup.patch_byte_width,
                },
            )
            .map_err(|_| Error::ArtifactMismatch)?;
            decoded_effects_footprint(machine, validated.effects())?
        }
        Architecture::Aarch64 => {
            let target_kind = match fixup.kind {
                SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1 => {
                    Aarch64ScalarCallFixupKind::SignedImmediate26WordsFromInstructionToInternalMachineV1
                }
                _ => return Err(Error::ArtifactMismatch),
            };
            let validated = validate_aarch64_selected_scalar_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &machine.alternative.encoded,
                bytes,
                Aarch64ScalarCallFixup {
                    kind: target_kind,
                    state: Aarch64ScalarCallFixupState::UnresolvedZeroImmediateV1,
                    callee: fixup.callee,
                    opcode_byte_offset: fixup.opcode_row_offset,
                    patch_byte_offset: fixup.patch_row_offset,
                    reference_byte_offset: fixup.reference_row_offset,
                    patch_byte_width: fixup.patch_byte_width,
                },
            )
            .map_err(|_| Error::ArtifactMismatch)?;
            decoded_effects_footprint(machine, validated.effects())?
        }
    };
    admit_decoded_footprint(instruction, machine, bytes.len(), footprint, &decoded)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_normalized_foreign_call(
    target: NativeTarget,
    selected_function: &SelectedFunction,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    row: &SelectedFormEncodingRow,
    encoded: &SelectedFormEncodedRowStore,
) -> Result<(), Error> {
    let record = normalized_foreign_call_record(target, instruction, kind, selected_function)?;
    if record.call.binding.table_dispatch().is_some() {
        if !matches!(row.state, SelectedFormEncodingState::Encoded) {
            return Err(Error::ArtifactMismatch);
        }
        let (bytes, retained) = encoded.encoded(row).ok_or(Error::ArtifactMismatch)?;
        let accesses = operand_accesses(machine);
        let decoded = match target.architecture {
            Architecture::X86_64 => decoded_row_footprint!(
                isa_x86_64::validate_x86_64_selected_table_call_form(
                    record,
                    machine.alternative.key,
                    &operand_views(machine),
                    &accesses,
                    bytes,
                ),
                Error::X86_64
            ),
            Architecture::Aarch64 => decoded_row_footprint!(
                isa_aarch64::validate_aarch64_selected_table_call_form(
                    record,
                    machine.alternative.key,
                    &operand_views(machine),
                    &accesses,
                    bytes,
                ),
                Error::Aarch64
            ),
        };
        admit_decoded_footprint(instruction, machine, bytes.len(), retained, &decoded)?;
        return Ok(());
    }
    if let Some(binding) = record.call.binding.system_call() {
        let (bytes, footprint) = encoded_row(row, encoded)?;
        if !record.call.callbacks.is_empty() {
            return Err(Error::ArtifactMismatch);
        }
        let views = operand_views(machine);
        let accesses = operand_accesses(machine);
        let decoded = match target.architecture {
            Architecture::X86_64 => decoded_row_footprint!(
                isa_x86_64::validate_x86_64_selected_system_call_form(
                    target,
                    physical,
                    kind,
                    machine.alternative.key,
                    &views,
                    &accesses,
                    binding.number,
                    bytes,
                ),
                |_| Error::ArtifactMismatch
            ),
            Architecture::Aarch64 => decoded_row_footprint!(
                isa_aarch64::validate_aarch64_selected_system_call_form(
                    target,
                    physical,
                    kind,
                    machine.alternative.key,
                    &views,
                    &accesses,
                    binding.number,
                    bytes,
                ),
                |_| Error::ArtifactMismatch
            ),
        };
        admit_decoded_footprint(instruction, machine, bytes.len(), footprint, &decoded)?;
        return Ok(());
    }
    let SelectedFormEncodingState::UnresolvedNormalizedForeignCall { fixup } = &row.state else {
        return Err(Error::ArtifactMismatch);
    };
    let Some((bytes, footprint)) = encoded.encoded(row) else {
        return Err(Error::ArtifactMismatch);
    };
    if fixup.state != SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1
        || fixup.addend != 0
    {
        return Err(Error::ArtifactMismatch);
    }
    let (boundary, ordinal) = match kind {
        SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => (boundary, ordinal),
        _ => return Err(Error::ArtifactMismatch),
    };
    if fixup.boundary != boundary || fixup.ordinal != ordinal {
        return Err(Error::ArtifactMismatch);
    }
    let views = operand_views(machine);
    let accesses = operand_accesses(machine);
    let decoded = match target.architecture {
        Architecture::X86_64 => {
            let target_kind = match fixup.kind {
                SelectedFormNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1 => {
                    X86_64NormalizedForeignCallFixupKind::Relative32FromNextInstructionToNormalizedForeignImportV1
                }
                _ => return Err(Error::ArtifactMismatch),
            };
            let (destinations, callbacks) = x86_64_callback_replay(fixup)?;
            let validated = validate_x86_64_selected_normalized_foreign_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &accesses,
                &machine.alternative.encoded,
                bytes,
                X86_64NormalizedForeignCallFixup {
                    kind: target_kind,
                    state: X86_64NormalizedForeignCallFixupState::UnresolvedImportFieldV1,
                    boundary: fixup.boundary,
                    ordinal: fixup.ordinal,
                    opcode_byte_offset: fixup.opcode_row_offset,
                    patch_byte_offset: fixup.patch_row_offset,
                    reference_byte_offset: fixup.reference_row_offset,
                    patch_byte_width: fixup.patch_byte_width,
                    callbacks,
                },
                &destinations,
                record,
            )
            .map_err(|_| Error::ArtifactMismatch)?;
            decoded_effects_footprint(machine, validated.effects())?
        }
        Architecture::Aarch64 => {
            let target_kind = match fixup.kind {
                SelectedFormNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => {
                    Aarch64NormalizedForeignCallFixupKind::BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1
                }
                _ => return Err(Error::ArtifactMismatch),
            };
            let (destinations, callbacks) = aarch64_callback_replay(fixup)?;
            let validated = validate_aarch64_selected_normalized_foreign_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &accesses,
                &machine.alternative.encoded,
                bytes,
                Aarch64NormalizedForeignCallFixup {
                    kind: target_kind,
                    state: Aarch64NormalizedForeignCallFixupState::UnresolvedImportFieldV1,
                    boundary: fixup.boundary,
                    ordinal: fixup.ordinal,
                    opcode_byte_offset: fixup.opcode_row_offset,
                    patch_byte_offset: fixup.patch_row_offset,
                    reference_byte_offset: fixup.reference_row_offset,
                    patch_byte_width: fixup.patch_byte_width,
                    callbacks,
                },
                &destinations,
                record,
            )
            .map_err(|_| Error::ArtifactMismatch)?;
            decoded_effects_footprint(machine, validated.effects())?
        }
    };
    admit_decoded_footprint(instruction, machine, bytes.len(), footprint, &decoded)?;
    Ok(())
}

fn x86_64_callback_replay(
    fixup: &machine_code::SelectedFormNormalizedForeignCallFixup,
) -> Result<
    (
        Vec<X86_64CallbackMaterializationDestination>,
        Vec<X86_64CallbackAddressMaterialization>,
    ),
    Error,
> {
    let mut destinations = Vec::with_capacity(fixup.callbacks.len());
    let mut callbacks = Vec::with_capacity(fixup.callbacks.len());
    for callback in &fixup.callbacks {
        let machine_code::SelectedFormCallbackMaterializationEncoding::X86_64Relative32 {
            patch_row_offset,
        } = callback.encoding
        else {
            return Err(Error::ArtifactMismatch);
        };
        let destination = match callback.destination.clone() {
            machine_code::CallbackAddressDestination::Register(register) => {
                X86_64CallbackMaterializationDestination::Register(register)
            }
            machine_code::CallbackAddressDestination::RecordField {
                root_register,
                placements,
            } => X86_64CallbackMaterializationDestination::RecordField {
                root_register,
                placements,
            },
            machine_code::CallbackAddressDestination::OutgoingStack { byte_offset } => {
                X86_64CallbackMaterializationDestination::OutgoingStack { byte_offset }
            }
        };
        destinations.push(destination.clone());
        callbacks.push(X86_64CallbackAddressMaterialization {
            destination,
            materialization_byte_offset: callback.code_row_offset,
            materialization_byte_count: callback.code_byte_count,
            relocation_byte_offset: patch_row_offset,
        });
    }
    Ok((destinations, callbacks))
}

fn aarch64_callback_replay(
    fixup: &machine_code::SelectedFormNormalizedForeignCallFixup,
) -> Result<
    (
        Vec<Aarch64CallbackMaterializationDestination>,
        Vec<Aarch64CallbackAddressMaterialization>,
    ),
    Error,
> {
    let mut destinations = Vec::with_capacity(fixup.callbacks.len());
    let mut callbacks = Vec::with_capacity(fixup.callbacks.len());
    for callback in &fixup.callbacks {
        let machine_code::SelectedFormCallbackMaterializationEncoding::Aarch64PageAddress {
            page_patch_row_offset,
            page_offset_patch_row_offset,
        } = callback.encoding
        else {
            return Err(Error::ArtifactMismatch);
        };
        let destination = match callback.destination.clone() {
            machine_code::CallbackAddressDestination::Register(register) => {
                Aarch64CallbackMaterializationDestination::Register(register)
            }
            machine_code::CallbackAddressDestination::RecordField {
                root_register,
                placements,
            } => Aarch64CallbackMaterializationDestination::RecordField {
                root_register,
                placements,
            },
            machine_code::CallbackAddressDestination::OutgoingStack { .. } => {
                return Err(Error::ArtifactMismatch);
            }
        };
        destinations.push(destination.clone());
        callbacks.push(Aarch64CallbackAddressMaterialization {
            destination,
            materialization_byte_offset: callback.code_row_offset,
            materialization_byte_count: callback.code_byte_count,
            page_relocation_byte_offset: page_patch_row_offset,
            page_offset_relocation_byte_offset: page_offset_patch_row_offset,
        });
    }
    Ok((destinations, callbacks))
}

#[cfg(test)]
mod tests {
    use machine_code::{
        SelectedFormEncodedRowStore, SelectedFormEncoding, SelectedFormEncodingCounts,
        SelectedFormEncodingIdentity,
    };
    use physical_instructions::PostAllocationMachineIdentity;
    use selected_instructions::SelectedInstructionPlanIdentity;

    use super::{Error, validate_aggregate};

    #[test]
    fn raw_encoding_rejects_reauthenticated_counts_and_stale_identity() {
        let mut program = SelectedFormEncoding {
            selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
            machine: PostAllocationMachineIdentity::from_bytes([2; 32]),
            post_allocation_machine_optimization: None,
            identity: SelectedFormEncodingIdentity::from_bytes([0; 32]),
            rows: vec![],
            encoded: SelectedFormEncodedRowStore::new(),
            frame: None,
            counts: SelectedFormEncodingCounts::default(),
        };
        program.identity = program.recomputed_identity();
        validate_aggregate(&program).unwrap();
        let original = program.clone();
        program.counts.ordinary_encoded = 1;
        program.identity = program.recomputed_identity();
        assert!(matches!(
            validate_aggregate(&program),
            Err(Error::ArtifactMismatch)
        ));
        program = original;
        program.identity = SelectedFormEncodingIdentity::from_bytes([0; 32]);
        assert!(matches!(
            validate_aggregate(&program),
            Err(Error::ArtifactMismatch)
        ));
    }
}
