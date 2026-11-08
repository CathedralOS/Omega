/*
We encode one selected instruction and its paired post-allocation physical
machine instruction into a `SelectedFormEncodingRow` and intern its bytes and
decoded footprint into the shared `SelectedFormEncodedRowStore`
(`:RouteDrivenEncoding`).

Each `SelectedInstructionKind` declares its encoding route (`route_of`):
- `RowRoute::ResolvedAddress` requires a resolved physical address in the
  matching `AddressOperationFamily` and encodes the frame, memory, or hosted
  byte-channel instruction form.
- `RowRoute::InternalCallTemplate` encodes a dynamic call or a direct internal
  scalar call template carrying an `UnresolvedInternalMachineCall` fixup.
- `RowRoute::NormalizedForeignCallTemplate` resolves the function's
  `SelectedNormalizedForeignCall` record (`normalized_foreign_call_record`) and
  encodes a table-dispatched call, direct system call, or foreign import call
  template with callback materializations.
- `RowRoute::ByteLiteralAddress` encodes a PC-relative byte-literal address
  (`lea [rip + disp32]` on x86-64, `adrp` + `add` on AArch64) with an
  `UnresolvedImageAddress` fixup.
- `RowRoute::DeferredControlFlow` records `DeferredControl` without bytes so
  Stage 08 can encode the branch once block offsets are resolved.
- `RowRoute::Ordinary` encodes a self-contained scalar or hosted process-exit
  instruction.

Every non-address route rejects a present `ResolvedPhysicalAddress`, and every
encoded draft verifies that the ISA encoder's register reads, register writes,
implicit unit effects, and byte length match the machine alternative's
declarations before interning.
*/

use calling_conventions::{NativePlace, ValueLocation};
use isa_aarch64::{
    AARCH64_BYTE_LITERAL_ADDRESS_PAGE_FIELD_OFFSET,
    AARCH64_BYTE_LITERAL_ADDRESS_PAGE_OFFSET_FIELD_OFFSET,
    Aarch64CallbackMaterializationDestination, Aarch64NormalizedForeignCallFixupKind,
    Aarch64NormalizedForeignCallFixupState, Aarch64ScalarCallFixupKind,
    Aarch64ScalarCallFixupState, encode_aarch64_selected_form, encode_aarch64_selected_frame_form,
    encode_aarch64_selected_hosted_exit_process_form,
    encode_aarch64_selected_hosted_read_byte_form, encode_aarch64_selected_hosted_write_byte_form,
    encode_aarch64_selected_memory_form, encode_aarch64_selected_normalized_foreign_call_template,
    encode_aarch64_selected_scalar_call_template,
};
use isa_x86_64::{
    X86_64_BYTE_LITERAL_ADDRESS_FIELD_OFFSET, X86_64CallbackMaterializationDestination,
    X86_64NormalizedForeignCallFixupKind, X86_64NormalizedForeignCallFixupState,
    X86_64ScalarCallFixupKind, X86_64ScalarCallFixupState, encode_x86_64_selected_form,
    encode_x86_64_selected_frame_form, encode_x86_64_selected_hosted_exit_process_form,
    encode_x86_64_selected_hosted_read_byte_form, encode_x86_64_selected_hosted_write_byte_form,
    encode_x86_64_selected_memory_form, encode_x86_64_selected_normalized_foreign_call_template,
    encode_x86_64_selected_scalar_call_template,
};
use machine_code::{
    CallbackAddressDestination, DeferredControlEncodingReason, ImageAddressFields,
    ResolvedPhysicalAddress, SelectedFormCallbackMaterialization,
    SelectedFormCallbackMaterializationEncoding, SelectedFormDecodedFootprint,
    SelectedFormEncodedRowStore, SelectedFormEncodingRow, SelectedFormEncodingState,
    SelectedFormImageAddressFixup, SelectedFormInternalMachineFixup,
    SelectedFormInternalMachineFixupKind, SelectedFormInternalMachineFixupState,
    SelectedFormMachineDisposition, SelectedFormNormalizedForeignCallFixup,
    SelectedFormNormalizedForeignCallFixupKind, SelectedFormNormalizedForeignCallFixupState,
};
use physical_instructions::{PhysicalAddressOperation, PostAllocationMachineInstruction};
use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeKey, MachineEncodedEffects, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedNormalizedForeignCall,
};
use target::{Architecture, NativeTarget};

use crate::OptimizedSelectedFormEncodingError as Error;
use crate::validation::validate_size;

macro_rules! selected_encoded_parts {
    ($encoded:expr) => {{
        let footprint = $encoded.footprint();
        (
            $encoded.bytes().to_vec(),
            footprint.register_reads.clone(),
            footprint.register_writes.clone(),
            footprint.encoded.clone(),
        )
    }};
}

macro_rules! selected_unresolved_fixup {
    ($encoded:expr, $fixup:expr, $kind:expr, $state:expr, $callbacks:expr) => {
        (
            $encoded.bytes().to_vec(),
            $encoded.effects().clone(),
            SelectedFormNormalizedForeignCallFixup {
                kind: $kind,
                state: $state,
                boundary: $fixup.boundary,
                ordinal: $fixup.ordinal,
                opcode_row_offset: $fixup.opcode_byte_offset,
                patch_row_offset: $fixup.patch_byte_offset,
                reference_row_offset: $fixup.reference_byte_offset,
                patch_byte_width: $fixup.patch_byte_width,
                addend: 0,
                callbacks: $callbacks,
            },
        )
    };
}

struct SelectedFormEncodedRowDraft {
    state: SelectedFormEncodingState,
    bytes: Vec<u8>,
    footprint: Option<SelectedFormDecodedFootprint>,
}

impl SelectedFormEncodedRowDraft {
    const fn encoded(
        state: SelectedFormEncodingState,
        bytes: Vec<u8>,
        footprint: SelectedFormDecodedFootprint,
    ) -> Self {
        Self {
            state,
            bytes,
            footprint: Some(footprint),
        }
    }

    const fn deferred(reason: DeferredControlEncodingReason) -> Self {
        Self {
            state: SelectedFormEncodingState::DeferredControl { reason },
            bytes: Vec::new(),
            footprint: None,
        }
    }

    fn retagged(self, state: SelectedFormEncodingState) -> Result<Self, Error> {
        if !matches!(self.state, SelectedFormEncodingState::Encoded) {
            return Err(Error::ArtifactMismatch);
        }
        Ok(Self { state, ..self })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AddressOperationFamily {
    AccessorScalarAccess,
    Store,
    AddressOffset,
    Load64,
    LoadPacked,
    StorePacked,
    Load8,
    Load16,
    Load32,
    Load8Indexed,
    HostedReadByte,
    HostedWriteByteI32,
    Store64,
    FrameAddress,
    SaveFloatingControl,
    RestoreFloatingControl,
}

impl AddressOperationFamily {
    fn admits(self, operation: PhysicalAddressOperation) -> bool {
        use PhysicalAddressOperation as Operation;
        matches!(
            (self, operation),
            (
                Self::AccessorScalarAccess,
                Operation::AccessorScalarAccess { .. }
            ) | (Self::Store, Operation::Store { .. })
                | (Self::AddressOffset, Operation::AddressOffset { .. })
                | (Self::Load64, Operation::Load64 { .. })
                | (Self::LoadPacked, Operation::LoadPacked { .. })
                | (Self::StorePacked, Operation::StorePacked { .. })
                | (Self::Load8, Operation::Load8 { .. })
                | (Self::Load16, Operation::Load16 { .. })
                | (Self::Load32, Operation::Load32 { .. })
                | (Self::Load8Indexed, Operation::Load8Indexed { .. })
                | (Self::HostedReadByte, Operation::HostedReadByte { .. })
                | (
                    Self::HostedWriteByteI32,
                    Operation::HostedWriteByteI32 { .. }
                )
                | (
                    Self::SaveFloatingControl,
                    Operation::SaveFloatingControl { .. }
                )
                | (
                    Self::RestoreFloatingControl,
                    Operation::RestoreFloatingControl { .. }
                )
                | (Self::Store64, Operation::Store64 { .. })
                | (Self::FrameAddress, Operation::FrameAddress { .. })
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostedChannel {
    None,
    WriteByteI32,
    ReadByte,
    ExitProcessI32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowRoute {
    ResolvedAddress {
        operation: AddressOperationFamily,
        channel: HostedChannel,
    },
    InternalCallTemplate,
    NormalizedForeignCallTemplate,
    ByteLiteralAddress,
    DeferredControlFlow,
    Ordinary {
        channel: HostedChannel,
    },
}

fn route_of(kind: SelectedInstructionKind) -> RowRoute {
    use SelectedInstructionKind as Kind;
    let addressed = |operation| RowRoute::ResolvedAddress {
        operation,
        channel: HostedChannel::None,
    };
    match kind {
        Kind::AccessorScalarAccess { .. } => {
            addressed(AddressOperationFamily::AccessorScalarAccess)
        }
        Kind::Store { .. } => addressed(AddressOperationFamily::Store),
        Kind::AddressOffset { .. } => addressed(AddressOperationFamily::AddressOffset),
        Kind::Load64 { .. } => addressed(AddressOperationFamily::Load64),
        Kind::LoadPacked { .. } => addressed(AddressOperationFamily::LoadPacked),
        Kind::StorePacked { .. } => addressed(AddressOperationFamily::StorePacked),
        Kind::Load8 { .. } => addressed(AddressOperationFamily::Load8),
        Kind::Load16 { .. } => addressed(AddressOperationFamily::Load16),
        Kind::Load32 { .. } => addressed(AddressOperationFamily::Load32),
        Kind::Load8Indexed => addressed(AddressOperationFamily::Load8Indexed),
        Kind::SaveFloatingControl { .. } => addressed(AddressOperationFamily::SaveFloatingControl),
        Kind::RestoreFloatingControl { .. } => {
            addressed(AddressOperationFamily::RestoreFloatingControl)
        }
        Kind::Store64 { .. } => addressed(AddressOperationFamily::Store64),
        Kind::FrameAddress { .. } => addressed(AddressOperationFamily::FrameAddress),
        Kind::HostedReadByte { .. } => RowRoute::ResolvedAddress {
            operation: AddressOperationFamily::HostedReadByte,
            channel: HostedChannel::ReadByte,
        },
        Kind::HostedWriteByteI32 { .. } => RowRoute::ResolvedAddress {
            operation: AddressOperationFamily::HostedWriteByteI32,
            channel: HostedChannel::WriteByteI32,
        },
        selected_instructions::selected_call_pattern!(..) => RowRoute::InternalCallTemplate,
        Kind::NormalizedForeignCall { .. } => RowRoute::NormalizedForeignCallTemplate,
        Kind::ImageAddress { .. } => RowRoute::ByteLiteralAddress,
        Kind::ConditionalBranchNonZero
        | Kind::ConditionalBranchU64LessThan
        | Kind::ConditionalBranchI64LessThan
        | Kind::Jump => RowRoute::DeferredControlFlow,
        Kind::HostedExitProcessI32 => RowRoute::Ordinary {
            channel: HostedChannel::ExitProcessI32,
        },
        _ => RowRoute::Ordinary {
            channel: HostedChannel::None,
        },
    }
}

pub(super) fn encode_row(
    target: NativeTarget,
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    selected_function: &SelectedFunction,
    address: Option<ResolvedPhysicalAddress>,
    encoded: &mut SelectedFormEncodedRowStore,
) -> Result<SelectedFormEncodingRow, Error> {
    let alternative = machine.alternative.key;
    let route = route_of(selected.kind);
    let draft = match route {
        RowRoute::ResolvedAddress { operation, channel } => {
            let address = address.ok_or(Error::ArtifactMismatch)?;
            if !operation.admits(address.symbolic) {
                return Err(Error::ArtifactMismatch);
            }
            encode_address_routed(
                target,
                selected.id,
                selected.kind,
                channel,
                alternative,
                machine,
                physical,
                address,
            )?
        }
        RowRoute::InternalCallTemplate => {
            reject_unrouted_address(address)?;
            encode_scalar_call(target, selected.id, selected.kind, machine, physical)?
        }
        RowRoute::NormalizedForeignCallTemplate => {
            reject_unrouted_address(address)?;
            encode_normalized_foreign_call(
                target,
                selected.id,
                selected.kind,
                machine,
                physical,
                selected_function,
            )?
        }
        RowRoute::ByteLiteralAddress => {
            reject_unrouted_address(address)?;
            image_address(target, selected.id, selected.kind, machine, physical)?
        }
        RowRoute::DeferredControlFlow => {
            reject_unrouted_address(address)?;
            SelectedFormEncodedRowDraft::deferred(
                DeferredControlEncodingReason::RequiresResolvedBranchLayout,
            )
        }
        RowRoute::Ordinary { channel } => {
            reject_unrouted_address(address)?;
            encode_scalar(
                target,
                selected.id,
                selected.kind,
                channel,
                alternative,
                machine,
                physical,
            )?
        }
    };
    let (code, footprint) = match draft.footprint {
        Some(footprint) => encoded.intern(&draft.bytes, footprint),
        None => Default::default(),
    };
    Ok(SelectedFormEncodingRow {
        instruction: selected.id,
        alternative,
        machine_disposition: SelectedFormMachineDisposition::RetainedV1,
        state: draft.state,
        address: address.map(Box::new),
        code,
        footprint,
    })
}

fn image_address(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<SelectedFormEncodedRowDraft, Error> {
    let SelectedInstructionKind::ImageAddress { reference } = kind else {
        return Err(Error::ArtifactMismatch);
    };
    encode_scalar(
        target,
        instruction,
        kind,
        HostedChannel::None,
        machine.alternative.key,
        machine,
        physical,
    )?
    .retagged(SelectedFormEncodingState::UnresolvedImageAddress {
        fixup: Box::new(SelectedFormImageAddressFixup {
            reference,
            fields: image_address_fields(target.architecture),
        }),
    })
}

fn image_address_fields(architecture: Architecture) -> ImageAddressFields {
    match architecture {
        Architecture::X86_64 => ImageAddressFields::X86_64Relative32 {
            patch_offset: u64::from(X86_64_BYTE_LITERAL_ADDRESS_FIELD_OFFSET),
        },
        Architecture::Aarch64 => ImageAddressFields::Aarch64PageAddress {
            page_patch_offset: u64::from(AARCH64_BYTE_LITERAL_ADDRESS_PAGE_FIELD_OFFSET),
            page_offset_patch_offset: u64::from(
                AARCH64_BYTE_LITERAL_ADDRESS_PAGE_OFFSET_FIELD_OFFSET,
            ),
        },
    }
}

fn reject_unrouted_address(address: Option<ResolvedPhysicalAddress>) -> Result<(), Error> {
    if address.is_some() {
        return Err(Error::ArtifactMismatch);
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

#[allow(clippy::too_many_arguments)]
fn encode_address_routed(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    channel: HostedChannel,
    alternative: MachineAlternativeKey,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    address: ResolvedPhysicalAddress,
) -> Result<SelectedFormEncodedRowDraft, Error> {
    let views = operand_views(machine);
    let displacement = address.displacement;
    let (bytes, reads, writes, encoded_effects) = match target.architecture {
        Architecture::X86_64 => {
            let encoded = match channel {
                HostedChannel::None
                    if matches!(
                        kind,
                        SelectedInstructionKind::FrameAddress { .. }
                            | SelectedInstructionKind::Store64 { .. }
                    ) =>
                {
                    encode_x86_64_selected_frame_form(
                        physical,
                        kind,
                        alternative,
                        &views,
                        displacement,
                        address.frame_alignment_bytes,
                    )
                }
                HostedChannel::None => encode_x86_64_selected_memory_form(
                    physical,
                    kind,
                    alternative,
                    &views,
                    displacement,
                ),
                HostedChannel::WriteByteI32 => encode_x86_64_selected_hosted_write_byte_form(
                    target,
                    physical,
                    kind,
                    alternative,
                    &views,
                    displacement,
                ),
                HostedChannel::ReadByte => encode_x86_64_selected_hosted_read_byte_form(
                    target,
                    physical,
                    kind,
                    alternative,
                    &views,
                    displacement,
                ),
                HostedChannel::ExitProcessI32 => {
                    return Err(Error::ArtifactMismatch);
                }
            }
            .map_err(Error::X86_64)?;
            selected_encoded_parts!(encoded)
        }
        Architecture::Aarch64 => {
            let encoded = match channel {
                HostedChannel::None
                    if matches!(
                        kind,
                        SelectedInstructionKind::FrameAddress { .. }
                            | SelectedInstructionKind::Store64 { .. }
                    ) =>
                {
                    encode_aarch64_selected_frame_form(
                        physical,
                        kind,
                        alternative,
                        &views,
                        u32::try_from(displacement).map_err(|_| Error::ArtifactMismatch)?,
                        address.frame_alignment_bytes,
                    )
                }
                HostedChannel::None => encode_aarch64_selected_memory_form(
                    physical,
                    kind,
                    alternative,
                    &views,
                    displacement,
                ),
                HostedChannel::WriteByteI32 => encode_aarch64_selected_hosted_write_byte_form(
                    target,
                    physical,
                    kind,
                    alternative,
                    &views,
                    displacement,
                ),
                HostedChannel::ReadByte => encode_aarch64_selected_hosted_read_byte_form(
                    target,
                    physical,
                    kind,
                    alternative,
                    &views,
                    displacement,
                ),
                HostedChannel::ExitProcessI32 => {
                    return Err(Error::ArtifactMismatch);
                }
            }
            .map_err(Error::Aarch64)?;
            selected_encoded_parts!(encoded)
        }
    };
    let footprint =
        validate_operand_footprint(instruction, machine, &encoded_effects, &reads, &writes)?;
    if encoded_effects != machine.alternative.encoded {
        return Err(Error::ArtifactMismatch);
    }
    validate_size(instruction, machine.alternative.size, bytes.len())?;
    Ok(SelectedFormEncodedRowDraft::encoded(
        SelectedFormEncodingState::Encoded,
        bytes,
        footprint,
    ))
}

#[allow(clippy::too_many_arguments)]
fn encode_scalar(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    channel: HostedChannel,
    alternative: MachineAlternativeKey,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<SelectedFormEncodedRowDraft, Error> {
    let views = operand_views(machine);
    let (bytes, reads, writes, encoded_effects) = match target.architecture {
        Architecture::X86_64 => {
            let encoded = match channel {
                HostedChannel::ExitProcessI32 => encode_x86_64_selected_hosted_exit_process_form(
                    target,
                    physical,
                    kind,
                    alternative,
                    &views,
                ),
                HostedChannel::None => {
                    encode_x86_64_selected_form(physical, kind, alternative, &views)
                }
                HostedChannel::WriteByteI32 | HostedChannel::ReadByte => {
                    return Err(Error::ArtifactMismatch);
                }
            }
            .map_err(Error::X86_64)?;
            selected_encoded_parts!(encoded)
        }
        Architecture::Aarch64 => {
            let encoded = match channel {
                HostedChannel::ExitProcessI32 => encode_aarch64_selected_hosted_exit_process_form(
                    target,
                    physical,
                    kind,
                    alternative,
                    &views,
                ),
                HostedChannel::None => {
                    encode_aarch64_selected_form(physical, kind, alternative, &views)
                }
                HostedChannel::WriteByteI32 | HostedChannel::ReadByte => {
                    return Err(Error::ArtifactMismatch);
                }
            }
            .map_err(Error::Aarch64)?;
            selected_encoded_parts!(encoded)
        }
    };
    let footprint =
        validate_operand_footprint(instruction, machine, &encoded_effects, &reads, &writes)?;
    if encoded_effects != machine.alternative.encoded {
        return Err(Error::ImplicitFootprintMismatch(instruction));
    }
    validate_size(instruction, machine.alternative.size, bytes.len())?;
    Ok(SelectedFormEncodedRowDraft::encoded(
        SelectedFormEncodingState::Encoded,
        bytes,
        footprint,
    ))
}

fn decoded_footprint(
    machine: &PostAllocationMachineInstruction,
    effects: &MachineEncodedEffects,
) -> Result<SelectedFormDecodedFootprint, Error> {
    let resolve = |operand: u16| machine.operand_footprint(operand).map(|row| row.view);
    let register_reads = effects
        .external_operand_reads
        .iter()
        .map(|operand| resolve(*operand))
        .collect::<Option<Vec<_>>>()
        .ok_or(Error::OperandFootprintMismatch(machine.instruction))?;
    let register_writes = effects
        .external_operand_writes
        .iter()
        .map(|operand| resolve(*operand))
        .collect::<Option<Vec<_>>>()
        .ok_or(Error::OperandFootprintMismatch(machine.instruction))?;
    Ok(SelectedFormDecodedFootprint {
        register_reads,
        register_writes,
        implicit_defs: effects.implicit_unit_defs.clone(),
        implicit_clobbers: effects.implicit_unit_clobbers.clone(),
        encoded: effects.clone(),
    })
}

fn validated_encoded_draft(
    instruction: SelectedInstructionId,
    machine: &PostAllocationMachineInstruction,
    effects: &MachineEncodedEffects,
    bytes: Vec<u8>,
    state: SelectedFormEncodingState,
) -> Result<SelectedFormEncodedRowDraft, Error> {
    let footprint = decoded_footprint(machine, effects)?;
    validate_size(instruction, machine.alternative.size, bytes.len())?;
    Ok(SelectedFormEncodedRowDraft::encoded(
        state, bytes, footprint,
    ))
}

fn validate_operand_footprint(
    instruction: SelectedInstructionId,
    machine: &PostAllocationMachineInstruction,
    encoded: &MachineEncodedEffects,
    reads: &[RegisterViewId],
    writes: &[RegisterViewId],
) -> Result<SelectedFormDecodedFootprint, Error> {
    let expected = decoded_footprint(machine, encoded)?;
    if expected.register_reads != reads || expected.register_writes != writes {
        return Err(Error::OperandFootprintMismatch(instruction));
    }
    Ok(expected)
}

fn encode_scalar_call(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<SelectedFormEncodedRowDraft, Error> {
    let views = operand_views(machine);
    if matches!(
        kind.call_target(),
        Some(selected_instructions::SelectedCallTarget::Dynamic { .. })
    ) {
        let (bytes, effects) = match target.architecture {
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
        return validated_encoded_draft(
            instruction,
            machine,
            &effects,
            bytes,
            SelectedFormEncodingState::Encoded,
        );
    }
    let (bytes, effects, fixup) = match target.architecture {
        Architecture::X86_64 => {
            let encoded = encode_x86_64_selected_scalar_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &machine.alternative.encoded,
            )
            .map_err(Error::X86_64ScalarCall)?;
            let target_fixup = encoded.fixup();
            let kind = match target_fixup.kind {
                X86_64ScalarCallFixupKind::Relative32FromNextInstructionToInternalMachineV1 => {
                    SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1
                }
            };
            let state = match target_fixup.state {
                X86_64ScalarCallFixupState::UnresolvedZeroFieldV1 => {
                    SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1
                }
            };
            (
                encoded.bytes().to_vec(),
                encoded.effects().clone(),
                SelectedFormInternalMachineFixup {
                    kind,
                    state,
                    callee: target_fixup.callee,
                    opcode_row_offset: target_fixup.opcode_byte_offset,
                    patch_row_offset: target_fixup.patch_byte_offset,
                    reference_row_offset: target_fixup.reference_byte_offset,
                    patch_byte_width: target_fixup.patch_byte_width,
                    addend: 0,
                },
            )
        }
        Architecture::Aarch64 => {
            let encoded = encode_aarch64_selected_scalar_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &machine.alternative.encoded,
            )
            .map_err(Error::Aarch64ScalarCall)?;
            let target_fixup = encoded.fixup();
            let kind = match target_fixup.kind {
                Aarch64ScalarCallFixupKind::SignedImmediate26WordsFromInstructionToInternalMachineV1 => {
                    SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1
                }
            };
            let state = match target_fixup.state {
                Aarch64ScalarCallFixupState::UnresolvedZeroImmediateV1 => {
                    SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1
                }
            };
            (
                encoded.bytes().to_vec(),
                encoded.effects().clone(),
                SelectedFormInternalMachineFixup {
                    kind,
                    state,
                    callee: target_fixup.callee,
                    opcode_row_offset: target_fixup.opcode_byte_offset,
                    patch_row_offset: target_fixup.patch_byte_offset,
                    reference_row_offset: target_fixup.reference_byte_offset,
                    patch_byte_width: target_fixup.patch_byte_width,
                    addend: 0,
                },
            )
        }
    };
    validated_encoded_draft(
        instruction,
        machine,
        &effects,
        bytes,
        SelectedFormEncodingState::UnresolvedInternalMachineCall {
            fixup: Box::new(fixup),
        },
    )
}

pub(crate) fn normalized_foreign_call_record(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    selected_function: &SelectedFunction,
) -> Result<&SelectedNormalizedForeignCall, Error> {
    let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = kind else {
        return Err(Error::ArtifactMismatch);
    };
    let record = selected_function
        .normalized_foreign_calls
        .get(ordinal as usize)
        .ok_or(Error::ArtifactMismatch)?;
    if record.instruction != instruction
        || record.call.boundary != boundary
        || record.call.binding.native_target() != Some(target)
        || !record.call.binding.has_valid_entry_mechanism()
    {
        return Err(Error::ArtifactMismatch);
    }
    Ok(record)
}

fn encode_normalized_foreign_call(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    selected_function: &SelectedFunction,
) -> Result<SelectedFormEncodedRowDraft, Error> {
    let record = normalized_foreign_call_record(target, instruction, kind, selected_function)?;
    let views = operand_views(machine);
    let accesses = machine
        .operands
        .iter()
        .map(|operand| operand.access)
        .collect::<Vec<_>>();
    if record.call.binding.table_dispatch().is_some() {
        let (bytes, effects) = match target.architecture {
            Architecture::X86_64 => {
                let encoded = isa_x86_64::encode_x86_64_selected_table_call_form(
                    record,
                    machine.alternative.key,
                    &views,
                    &accesses,
                )
                .map_err(Error::X86_64)?;
                (
                    encoded.bytes().to_vec(),
                    encoded.footprint().encoded.clone(),
                )
            }
            Architecture::Aarch64 => {
                let encoded = isa_aarch64::encode_aarch64_selected_table_call_form(
                    record,
                    machine.alternative.key,
                    &views,
                    &accesses,
                )
                .map_err(Error::Aarch64)?;
                (
                    encoded.bytes().to_vec(),
                    encoded.footprint().encoded.clone(),
                )
            }
        };
        return validated_encoded_draft(
            instruction,
            machine,
            &effects,
            bytes,
            SelectedFormEncodingState::Encoded,
        );
    }
    if let Some(binding) = record.call.binding.system_call() {
        if !record.call.callbacks.is_empty() {
            return Err(Error::ArtifactMismatch);
        }
        let (bytes, effects) = match target.architecture {
            Architecture::X86_64 => {
                let encoded = isa_x86_64::encode_x86_64_selected_system_call_form(
                    target,
                    physical,
                    kind,
                    machine.alternative.key,
                    &views,
                    &accesses,
                    binding.number,
                )
                .map_err(|_| Error::ArtifactMismatch)?;
                (
                    encoded.bytes().to_vec(),
                    encoded.footprint().encoded.clone(),
                )
            }
            Architecture::Aarch64 => {
                let encoded = isa_aarch64::encode_aarch64_selected_system_call_form(
                    target,
                    physical,
                    kind,
                    machine.alternative.key,
                    &views,
                    &accesses,
                    binding.number,
                )
                .map_err(|_| Error::ArtifactMismatch)?;
                (
                    encoded.bytes().to_vec(),
                    encoded.footprint().encoded.clone(),
                )
            }
        };
        return validated_encoded_draft(
            instruction,
            machine,
            &effects,
            bytes,
            SelectedFormEncodingState::Encoded,
        );
    }
    let destinations = callback_destinations(record)?;
    let (bytes, effects, fixup) = match target.architecture {
        Architecture::X86_64 => {
            let destinations = destinations
                .iter()
                .map(|destination| match destination {
                    CallbackAddressDestination::Register(register) => Ok(
                        X86_64CallbackMaterializationDestination::Register(*register),
                    ),
                    CallbackAddressDestination::RecordField {
                        root_register,
                        placements,
                    } => Ok(X86_64CallbackMaterializationDestination::RecordField {
                        root_register: *root_register,
                        placements: placements.clone(),
                    }),
                    CallbackAddressDestination::OutgoingStack { byte_offset } => {
                        Ok(X86_64CallbackMaterializationDestination::OutgoingStack {
                            byte_offset: *byte_offset,
                        })
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            let encoded = encode_x86_64_selected_normalized_foreign_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &accesses,
                &machine.alternative.encoded,
                &destinations,
                record,
            )
            .map_err(Error::X86_64NormalizedForeignCall)?;
            let target_fixup = encoded.fixup();
            let kind = match target_fixup.kind {
                X86_64NormalizedForeignCallFixupKind::Relative32FromNextInstructionToNormalizedForeignImportV1 => {
                    SelectedFormNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1
                }
            };
            let state = match target_fixup.state {
                X86_64NormalizedForeignCallFixupState::UnresolvedImportFieldV1 => {
                    SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1
                }
            };
            let callbacks = target_fixup
                .callbacks
                .iter()
                .map(|callback| SelectedFormCallbackMaterialization {
                    destination: match callback.destination.clone() {
                        X86_64CallbackMaterializationDestination::Register(register) => {
                            CallbackAddressDestination::Register(register)
                        }
                        X86_64CallbackMaterializationDestination::RecordField {
                            root_register,
                            placements,
                        } => CallbackAddressDestination::RecordField {
                            root_register,
                            placements,
                        },
                        X86_64CallbackMaterializationDestination::OutgoingStack { byte_offset } => {
                            CallbackAddressDestination::OutgoingStack { byte_offset }
                        }
                    },
                    code_row_offset: callback.materialization_byte_offset,
                    code_byte_count: callback.materialization_byte_count,
                    encoding: SelectedFormCallbackMaterializationEncoding::X86_64Relative32 {
                        patch_row_offset: callback.relocation_byte_offset,
                    },
                })
                .collect();
            selected_unresolved_fixup!(encoded, target_fixup, kind, state, callbacks)
        }
        Architecture::Aarch64 => {
            let destinations = destinations
                .iter()
                .map(|destination| match destination {
                    CallbackAddressDestination::Register(register) => Ok(
                        Aarch64CallbackMaterializationDestination::Register(*register),
                    ),
                    CallbackAddressDestination::RecordField {
                        root_register,
                        placements,
                    } => Ok(Aarch64CallbackMaterializationDestination::RecordField {
                        root_register: *root_register,
                        placements: placements.clone(),
                    }),
                    CallbackAddressDestination::OutgoingStack { .. } => {
                        Err(Error::ArtifactMismatch)
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            let encoded = encode_aarch64_selected_normalized_foreign_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &accesses,
                &machine.alternative.encoded,
                &destinations,
                record,
            )
            .map_err(Error::Aarch64NormalizedForeignCall)?;
            let target_fixup = encoded.fixup();
            let kind = match target_fixup.kind {
                Aarch64NormalizedForeignCallFixupKind::BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => {
                    SelectedFormNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1
                }
            };
            let state = match target_fixup.state {
                Aarch64NormalizedForeignCallFixupState::UnresolvedImportFieldV1 => {
                    SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1
                }
            };
            let callbacks = target_fixup
                .callbacks
                .iter()
                .map(|callback| SelectedFormCallbackMaterialization {
                    destination: match callback.destination.clone() {
                        Aarch64CallbackMaterializationDestination::Register(register) => {
                            CallbackAddressDestination::Register(register)
                        }
                        Aarch64CallbackMaterializationDestination::RecordField {
                            root_register,
                            placements,
                        } => CallbackAddressDestination::RecordField {
                            root_register,
                            placements,
                        },
                    },
                    code_row_offset: callback.materialization_byte_offset,
                    code_byte_count: callback.materialization_byte_count,
                    encoding: SelectedFormCallbackMaterializationEncoding::Aarch64PageAddress {
                        page_patch_row_offset: callback.page_relocation_byte_offset,
                        page_offset_patch_row_offset: callback.page_offset_relocation_byte_offset,
                    },
                })
                .collect();
            selected_unresolved_fixup!(encoded, target_fixup, kind, state, callbacks)
        }
    };
    validated_encoded_draft(
        instruction,
        machine,
        &effects,
        bytes,
        SelectedFormEncodingState::UnresolvedNormalizedForeignCall {
            fixup: Box::new(fixup),
        },
    )
}

fn callback_destinations(
    record: &SelectedNormalizedForeignCall,
) -> Result<Vec<CallbackAddressDestination>, Error> {
    record
        .call
        .callbacks
        .iter()
        .map(|callback| {
            match (
                &callback.destination,
                callback.application.placement.locations.as_slice(),
            ) {
                (NativePlace::Parameter(_), [ValueLocation::Register { register, .. }]) => {
                    Ok(CallbackAddressDestination::Register(*register))
                }
                (
                    NativePlace::Parameter(_),
                    [
                        ValueLocation::Stack {
                            stack_byte_offset, ..
                        },
                    ],
                ) => Ok(CallbackAddressDestination::OutgoingStack {
                    byte_offset: *stack_byte_offset,
                }),
                (NativePlace::Field { .. }, [ValueLocation::Register { register, .. }]) => {
                    let placements = callback
                        .field_placements
                        .clone()
                        .ok_or(Error::ArtifactMismatch)?;
                    Ok(CallbackAddressDestination::RecordField {
                        root_register: *register,
                        placements,
                    })
                }
                _ => Err(Error::ArtifactMismatch),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use isa_x86_64::x86_64_physical_register_model;
    use machine_code::{
        ResolvedPhysicalAddress, SelectedFormEncodedRowStore, SelectedFormEncodingRow,
        SelectedFormEncodingState, SelectedFormMachineDisposition,
    };
    use physical_instructions::{
        PhysicalAddressOperation, PhysicalOperandFootprint, PostAllocationMachineFunction,
        PostAllocationMachineInstruction,
    };
    use register_model::{
        RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess, RegisterUnitId,
        ValidatedPhysicalRegisterModel, validate_physical_register_model,
    };
    use selected_instructions::{
        MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
        MachineAlternativeKey, MachineEncodedEffects, MachineLatencyKnowledge, MachineSemanticKind,
        MachineSizeKnowledge, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
        SelectedInstructionProvenance, SelectedOperand, VirtualRegisterId,
    };
    use semantic_vocabulary::{IntegerValue, MachineId};
    use target::{Architecture, NativeTarget};

    use super::{Error, encode_row};

    fn fixture() -> (
        ValidatedPhysicalRegisterModel,
        SelectedInstruction,
        PostAllocationMachineInstruction,
    ) {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let rax = physical.model().view_named("rax").unwrap();
        let instruction = SelectedInstructionId(1);
        let virtual_register = VirtualRegisterId(1);
        let selected = SelectedInstruction {
            id: instruction,
            kind: SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(0_u8.into()),
            },
            constraint: RegisterConstraintKey {
                family: RegisterConstraintFamily::Instruction,
                variant: 0,
            },
            operands: vec![SelectedOperand {
                operand: 0,
                virtual_register,
                access: RegisterOperandAccess::Def,
                class: rax.class,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            }],
            implicit_uses: vec![],
            implicit_defs: vec![],
            clobbers: vec![],
            provenance: SelectedInstructionProvenance::default(),
        };
        let machine = PostAllocationMachineInstruction {
            instruction,
            address: None,
            alternative: MachineAlternative {
                key: MachineAlternativeKey {
                    family: MachineAlternativeFamily::MaterializeI64,
                    variant: 0,
                },
                applicability: MachineAlternativeApplicability::Always,
                size: MachineSizeKnowledge::ExactBytes(10),
                latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                encoded: MachineEncodedEffects::fallthrough_v1(vec![], vec![0]),
            },
            operands: vec![PhysicalOperandFootprint {
                operand: 0,
                virtual_register,
                class: rax.class,
                view: rax.id,
                access: RegisterOperandAccess::Def,
                storage_units: rax.units.clone(),
                read_units: vec![],
                write_units: rax.write_units.clone(),
                write_semantics: Some(rax.write_semantics),
            }],
            implicit_unit_uses: vec![],
            implicit_unit_defs: vec![],
            implicit_unit_clobbers: vec![],
            unit_uses: vec![],
            unit_defs: rax.write_units.clone(),
            unit_clobbers: vec![],
        };
        (physical, selected, machine)
    }

    fn selected_function() -> selected_instructions::SelectedFunction {
        selected_instructions::SelectedFunction {
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            nominal_cleanup_subject: None,
            provenance: Default::default(),
            structural: None,
            local_storage_slots: Vec::new(),
            outgoing_arguments: Vec::new(),
            calls: Vec::new(),
            normalized_foreign_calls: Vec::new(),
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: selected_instructions::SelectedBlockId(0),
            virtual_registers: Vec::new(),
            blocks: (Vec::new()).into(),
        }
    }

    fn encode_fixture_row(
        target: NativeTarget,
        selected: &SelectedInstruction,
        machine: &PostAllocationMachineInstruction,
        physical: &ValidatedPhysicalRegisterModel,
        address: Option<ResolvedPhysicalAddress>,
        encoded: &mut SelectedFormEncodedRowStore,
    ) -> Result<SelectedFormEncodingRow, Error> {
        encode_row(
            target,
            selected,
            machine,
            physical,
            &selected_function(),
            address,
            encoded,
        )
    }

    fn replay_fixture_row(
        target: NativeTarget,
        selected: &SelectedInstruction,
        machine: &PostAllocationMachineInstruction,
        physical: &ValidatedPhysicalRegisterModel,
        row: &SelectedFormEncodingRow,
        encoded: &SelectedFormEncodedRowStore,
    ) -> Result<(), Error> {
        crate::validation::validate_row(
            target,
            &selected_function(),
            selected,
            machine,
            physical,
            row,
            encoded,
        )
    }

    #[test]
    fn current_machine_encoding_preserves_ordinary_bytes_and_rejects_retired_dispositions() {
        let mut encoded = SelectedFormEncodedRowStore::new();
        let (physical, selected, machine) = fixture();
        let target = NativeTarget::linux_x64();
        let row =
            encode_fixture_row(target, &selected, &machine, &physical, None, &mut encoded).unwrap();
        assert!(matches!(row.state, SelectedFormEncodingState::Encoded));
        assert_eq!(
            encoded.bytes(row.code),
            &[0x48, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        replay_fixture_row(target, &selected, &machine, &physical, &row, &encoded).unwrap();
        let mut changed = row.clone();
        changed.machine_disposition =
            SelectedFormMachineDisposition::Aarch64ElidedSameViewCopyI64V1 {
                consumer: SelectedInstructionId(2),
            };
        assert!(
            replay_fixture_row(target, &selected, &machine, &physical, &changed, &encoded).is_err()
        );
        let changed = row;
        encoded.bytes_mut(changed.code)[0] ^= 1;
        assert!(
            replay_fixture_row(target, &selected, &machine, &physical, &changed, &encoded).is_err()
        );
    }

    #[test]
    fn system_call_encoding_rejects_a_detached_selected_custody_roster() {
        let mut encoded = SelectedFormEncodedRowStore::new();
        let (physical, mut selected, machine) = fixture();
        let target = NativeTarget::linux_x64();
        let ordinary =
            encode_fixture_row(target, &selected, &machine, &physical, None, &mut encoded).unwrap();
        selected.kind = SelectedInstructionKind::NormalizedForeignCall {
            boundary: semantic_vocabulary::BoundaryMachineId::new(3).unwrap(),
            ordinal: 0,
        };
        selected.constraint = isa_x86_64::x86_64_linux_system_call_keys()[0];
        assert!(
            encode_fixture_row(target, &selected, &machine, &physical, None, &mut encoded).is_err()
        );
        assert!(
            replay_fixture_row(target, &selected, &machine, &physical, &ordinary, &encoded)
                .is_err()
        );
    }

    fn unrouted_address_refuses_in_producer_and_replay(kind: Option<SelectedInstructionKind>) {
        let mut encoded = SelectedFormEncodedRowStore::new();
        let (physical, mut selected, machine) = fixture();
        let deferred = kind.is_some();
        if let Some(kind) = kind {
            selected.kind = kind;
        }
        let target = NativeTarget::linux_x64();
        let unrouted = Some(ResolvedPhysicalAddress {
            symbolic: PhysicalAddressOperation::Load64 {
                base_operand: 0,
                byte_offset: 0,
            },
            frame_alignment_bytes: 1,
            displacement: 0,
        });
        assert!(matches!(
            encode_fixture_row(
                target,
                &selected,
                &machine,
                &physical,
                unrouted,
                &mut encoded
            ),
            Err(Error::ArtifactMismatch)
        ));
        let row =
            encode_fixture_row(target, &selected, &machine, &physical, None, &mut encoded).unwrap();
        if deferred {
            assert!(matches!(
                row.state,
                SelectedFormEncodingState::DeferredControl { .. }
            ));
            replay_fixture_row(target, &selected, &machine, &physical, &row, &encoded).unwrap();
        }
        let mut changed = row;
        changed.address = unrouted.map(Box::new);
        assert!(
            replay_fixture_row(target, &selected, &machine, &physical, &changed, &encoded).is_err()
        );
    }

    #[test]
    fn unrouted_address_on_ordinary_kind_rejects_in_producer_and_replay() {
        unrouted_address_refuses_in_producer_and_replay(None);
    }

    #[test]
    fn unrouted_address_on_deferred_control_rejects_in_producer_and_replay() {
        unrouted_address_refuses_in_producer_and_replay(Some(SelectedInstructionKind::Jump));
    }

    #[test]
    fn narrow_loads_join_catalog_address_encoder_and_receiving_replay_on_four_targets() {
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            let x86 = target.architecture == Architecture::X86_64;
            let physical = validate_physical_register_model(if x86 {
                isa_x86_64::x86_64_physical_register_model()
            } else {
                isa_aarch64::aarch64_physical_register_model()
            })
            .unwrap();
            let constraints = if x86 {
                isa_x86_64::validate_x86_64_register_constraint_catalog(
                    isa_x86_64::x86_64_register_constraint_catalog(&physical),
                    &physical,
                )
                .unwrap()
            } else {
                isa_aarch64::validate_aarch64_register_constraint_catalog(
                    isa_aarch64::aarch64_register_constraint_catalog(&physical),
                    &physical,
                )
                .unwrap()
            };
            let catalog = if x86 {
                isa_x86_64::x86_64_machine_effect_catalog(target, &constraints).unwrap()
            } else {
                isa_aarch64::aarch64_machine_effect_catalog(target, &constraints).unwrap()
            };
            let catalog = if x86 {
                isa_x86_64::validate_x86_64_machine_effect_catalog(target, &constraints, catalog)
                    .unwrap()
            } else {
                isa_aarch64::validate_aarch64_machine_effect_catalog(target, &constraints, catalog)
                    .unwrap()
            };
            for (semantic, kind, address, width) in [
                (
                    MachineSemanticKind::Load8,
                    SelectedInstructionKind::Load8 { byte_offset: 2 },
                    PhysicalAddressOperation::Load8 {
                        base_operand: 0,
                        byte_offset: 2,
                    },
                    1,
                ),
                (
                    MachineSemanticKind::Load16,
                    SelectedInstructionKind::Load16 { byte_offset: 2 },
                    PhysicalAddressOperation::Load16 {
                        base_operand: 0,
                        byte_offset: 2,
                    },
                    2,
                ),
            ] {
                let declaration = catalog.catalog().declaration_by_semantic(semantic).unwrap();
                let mut selected = SelectedInstruction {
                    id: SelectedInstructionId(1),
                    kind,
                    constraint: declaration.constraint,
                    operands: Vec::new(),
                    implicit_uses: Vec::new(),
                    implicit_defs: Vec::new(),
                    clobbers: Vec::new(),
                    provenance: SelectedInstructionProvenance::default(),
                };
                let mut machine = PostAllocationMachineInstruction {
                    instruction: selected.id,
                    alternative: declaration.alternatives[0].clone(),
                    operands: Vec::new(),
                    address: Some(address),
                    implicit_unit_uses: Vec::new(),
                    implicit_unit_defs: Vec::new(),
                    implicit_unit_clobbers: Vec::new(),
                    unit_uses: Vec::new(),
                    unit_defs: Vec::new(),
                    unit_clobbers: Vec::new(),
                };
                for (operand, name, access) in [
                    (
                        0,
                        if x86 { "r12" } else { "x1" },
                        RegisterOperandAccess::Use,
                    ),
                    (
                        1,
                        if x86 { "r11" } else { "x2" },
                        RegisterOperandAccess::Def,
                    ),
                ] {
                    let view = physical.model().view_named(name).unwrap();
                    let virtual_register = VirtualRegisterId(u32::from(operand));
                    let reads = access == RegisterOperandAccess::Use;
                    selected.operands.push(SelectedOperand {
                        operand,
                        virtual_register,
                        access,
                        class: view.class,
                        fixed_view: None,
                        tied_to: None,
                        early_clobber: false,
                    });
                    machine.operands.push(PhysicalOperandFootprint {
                        operand,
                        virtual_register,
                        class: view.class,
                        view: view.id,
                        access,
                        storage_units: view.units.clone(),
                        read_units: if reads {
                            view.units.clone()
                        } else {
                            Vec::new()
                        },
                        write_units: if reads {
                            Vec::new()
                        } else {
                            view.write_units.clone()
                        },
                        write_semantics: (!reads).then_some(view.write_semantics),
                    });
                    if reads {
                        machine.unit_uses.extend_from_slice(&view.units);
                    } else {
                        machine.unit_defs.extend_from_slice(&view.write_units);
                    }
                }
                let function = PostAllocationMachineFunction {
                    machine: MachineId::new(1).unwrap(),
                    outgoing_arguments: Vec::new(),
                    local_storage_slots: Vec::new(),
                    blocks: Vec::new(),
                };
                let resolved = crate::frame_address::resolve(&function, None, &machine).unwrap();
                crate::frame_address::validate_address(&function, None, &machine, resolved)
                    .unwrap();
                let mut encoded = SelectedFormEncodedRowStore::new();
                let row = encode_fixture_row(
                    target,
                    &selected,
                    &machine,
                    &physical,
                    resolved,
                    &mut encoded,
                )
                .unwrap();
                replay_fixture_row(target, &selected, &machine, &physical, &row, &encoded).unwrap();
                assert_eq!(
                    machine.alternative.encoded.memory,
                    selected_instructions::MachineEncodedMemoryEffect::ReadPointerV1 {
                        pointer_operand: 0,
                        byte_count: width,
                    }
                );
                for mutation in 0..5 {
                    let mut changed_machine = machine.clone();
                    let mut changed_row = row.clone();
                    let mut changed_encoded = encoded.clone();
                    match mutation {
                        0 => {
                            changed_machine.alternative.encoded.memory =
                                selected_instructions::MachineEncodedMemoryEffect::ReadPointerV1 {
                                    pointer_operand: 0,
                                    byte_count: 8,
                                }
                        }
                        1 => changed_row.address.as_mut().unwrap().displacement = 4,
                        2 => changed_machine.operands[0].view = machine.operands[1].view,
                        3 => {
                            assert!(matches!(
                                changed_row.state,
                                SelectedFormEncodingState::Encoded
                            ));
                            changed_encoded.bytes_mut(changed_row.code)[0] ^= 1;
                        }
                        _ => {
                            changed_machine.alternative.key.family =
                                selected_instructions::MachineAlternativeFamily::Load64
                        }
                    }
                    assert!(
                        replay_fixture_row(
                            target,
                            &selected,
                            &changed_machine,
                            &physical,
                            &changed_row,
                            &changed_encoded
                        )
                        .is_err(),
                        "{target:?} {kind:?} mutation {mutation}"
                    );
                }
                let mut changed = resolved.unwrap();
                changed.symbolic = PhysicalAddressOperation::Load64 {
                    base_operand: 0,
                    byte_offset: 2,
                };
                assert!(
                    crate::frame_address::validate_address(
                        &function,
                        None,
                        &machine,
                        Some(changed)
                    )
                    .is_err()
                );
                let mut foreign_machine = machine.clone();
                foreign_machine.address = Some(PhysicalAddressOperation::Load64 {
                    base_operand: 0,
                    byte_offset: 2,
                });
                let foreign =
                    crate::frame_address::resolve(&function, None, &foreign_machine).unwrap();
                assert!(matches!(
                    encode_fixture_row(
                        target,
                        &selected,
                        &foreign_machine,
                        &physical,
                        foreign,
                        &mut encoded.clone()
                    ),
                    Err(Error::ArtifactMismatch)
                ));
                let mut changed_row = row.clone();
                changed_row.address = foreign.map(Box::new);
                assert!(
                    replay_fixture_row(
                        target,
                        &selected,
                        &foreign_machine,
                        &physical,
                        &changed_row,
                        &encoded
                    )
                    .is_err()
                );
            }
        }
    }

    #[test]
    fn implicit_effect_drift_rejects_in_producer_and_replay() {
        let mut encoded = SelectedFormEncodedRowStore::new();
        let (physical, selected, mut machine) = fixture();
        machine
            .alternative
            .encoded
            .implicit_unit_defs
            .push(RegisterUnitId(0xdead));
        assert!(matches!(
            encode_fixture_row(NativeTarget::linux_x64(), &selected, &machine, &physical, None, &mut encoded),
            Err(Error::ImplicitFootprintMismatch(
                instruction
            )) if instruction == SelectedInstructionId(1)
        ));
        let (physical, selected, machine) = fixture();
        let mut mutated = machine.clone();
        mutated
            .alternative
            .encoded
            .implicit_unit_defs
            .push(RegisterUnitId(0xdead));
        let row = encode_fixture_row(
            NativeTarget::linux_x64(),
            &selected,
            &machine,
            &physical,
            None,
            &mut encoded,
        )
        .unwrap();
        assert!(matches!(
            replay_fixture_row(NativeTarget::linux_x64(), &selected, &mutated, &physical, &row, &encoded),
            Err(Error::ImplicitFootprintMismatch(
                instruction
            )) if instruction == SelectedInstructionId(1)
        ));
    }

    #[test]
    fn unresolvable_external_operand_rejects_in_producer_and_replay() {
        let mut encoded = SelectedFormEncodedRowStore::new();
        let (physical, selected, mut machine) = fixture();
        machine.operands[0].operand = 9;
        assert!(matches!(
            encode_fixture_row(NativeTarget::linux_x64(), &selected, &machine, &physical, None, &mut encoded),
            Err(Error::OperandFootprintMismatch(
                instruction
            )) if instruction == SelectedInstructionId(1)
        ));
        let (physical, selected, machine) = fixture();
        let mut mutated = machine.clone();
        mutated.operands[0].operand = 9;
        let row = encode_fixture_row(
            NativeTarget::linux_x64(),
            &selected,
            &machine,
            &physical,
            None,
            &mut encoded,
        )
        .unwrap();
        assert!(matches!(
            replay_fixture_row(NativeTarget::linux_x64(), &selected, &mutated, &physical, &row, &encoded),
            Err(Error::OperandFootprintMismatch(
                instruction
            )) if instruction == SelectedInstructionId(1)
        ));
    }

    #[test]
    fn declared_size_drift_rejects_in_producer_and_replay() {
        let mut encoded = SelectedFormEncodedRowStore::new();
        let target = NativeTarget::linux_x64();
        for knowledge in [
            MachineSizeKnowledge::ExactBytes(9),
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 11,
                maximum_bytes: None,
            },
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 0,
                maximum_bytes: Some(9),
            },
        ] {
            let (physical, selected, mut machine) = fixture();
            machine.alternative.size = knowledge;
            assert!(matches!(
                encode_fixture_row(target, &selected, &machine, &physical, None, &mut encoded),
                Err(Error::SizeDeclarationMismatch(
                    instruction
                )) if instruction == SelectedInstructionId(1)
            ));
            let (physical, selected, machine) = fixture();
            let mut mutated = machine.clone();
            mutated.alternative.size = knowledge;
            let row =
                encode_fixture_row(target, &selected, &machine, &physical, None, &mut encoded)
                    .unwrap();
            assert!(matches!(
                replay_fixture_row(target, &selected, &mutated, &physical, &row, &encoded),
                Err(Error::SizeDeclarationMismatch(
                    instruction
                )) if instruction == SelectedInstructionId(1)
            ));
        }
        let (physical, selected, mut machine) = fixture();
        machine.alternative.size = MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 1,
            maximum_bytes: Some(15),
        };
        let row =
            encode_fixture_row(target, &selected, &machine, &physical, None, &mut encoded).unwrap();
        replay_fixture_row(target, &selected, &machine, &physical, &row, &encoded).unwrap();
    }

    #[test]
    fn stored_footprint_drift_rejects_in_replay() {
        let mut encoded = SelectedFormEncodedRowStore::new();
        let (physical, selected, machine) = fixture();
        let target = NativeTarget::linux_x64();
        let row =
            encode_fixture_row(target, &selected, &machine, &physical, None, &mut encoded).unwrap();
        assert!(matches!(row.state, SelectedFormEncodingState::Encoded));
        let footprint = encoded.footprint_mut(row.footprint).unwrap();
        footprint.register_reads.push(footprint.register_writes[0]);
        assert!(matches!(
            replay_fixture_row(target, &selected, &machine, &physical, &row, &encoded),
            Err(Error::ArtifactMismatch)
        ));
    }

    #[test]
    fn row_header_substitutions_reject_in_replay() {
        let mut encoded = SelectedFormEncodedRowStore::new();
        let (physical, selected, machine) = fixture();
        let target = NativeTarget::linux_x64();
        let row =
            encode_fixture_row(target, &selected, &machine, &physical, None, &mut encoded).unwrap();
        let mut changed = row.clone();
        changed.instruction = SelectedInstructionId(77);
        assert!(matches!(
            replay_fixture_row(target, &selected, &machine, &physical, &changed, &encoded),
            Err(Error::ArtifactMismatch)
        ));
        let mut changed = row.clone();
        changed.alternative.variant = 7;
        assert!(matches!(
            replay_fixture_row(target, &selected, &machine, &physical, &changed, &encoded),
            Err(Error::ArtifactMismatch)
        ));
    }

    #[test]
    fn encoded_state_on_deferred_control_kind_rejects_in_replay() {
        let mut encoded = SelectedFormEncodedRowStore::new();
        let (physical, mut selected, machine) = fixture();
        let target = NativeTarget::linux_x64();
        let ordinary_row =
            encode_fixture_row(target, &selected, &machine, &physical, None, &mut encoded).unwrap();
        selected.kind = SelectedInstructionKind::Jump;
        let mut row =
            encode_fixture_row(target, &selected, &machine, &physical, None, &mut encoded).unwrap();
        assert!(matches!(
            row.state,
            SelectedFormEncodingState::DeferredControl { .. }
        ));
        row.state = ordinary_row.state;
        row.code = ordinary_row.code;
        row.footprint = ordinary_row.footprint;
        assert!(matches!(
            replay_fixture_row(target, &selected, &machine, &physical, &row, &encoded),
            Err(Error::ArtifactMismatch)
        ));
    }
}
