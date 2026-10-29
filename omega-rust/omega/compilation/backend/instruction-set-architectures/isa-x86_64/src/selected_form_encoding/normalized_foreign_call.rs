//! Canonical unresolved AMD64 normalized-foreign-call templates.
//!
//! The emitted bytes are the same `E8 + zero rel32` placeholder an internal
//! call uses, but the fixup names the selected `{boundary, ordinal}` roster
//! row rather than an internal machine. The field stays unresolved until
//! object construction binds it to the declared import symbol; raw locator
//! bytes never enter this encoding contract.
//!
//! The admission both instruction sets run, the fixup record, and the validated
//! template record live in `normalized_foreign_call_plan`; this module keeps the
//! AMD64 callback materialization spellings, the `E8` emission, and the fixup
//! vocabulary that names them.

use normalized_foreign_call_plan::{Fixup, Template};
use register_model::{RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionKind,
    SelectedNormalizedForeignCall,
};
use semantic_vocabulary::BoundaryMachineId;
use target::NativeTarget;

use crate::machine_effects::x86_64_selected_abi;

/// Callback-bearing rows place one address-materialization group per private
/// callback destination before the call itself: a `lea <destination>,
/// [rip+disp32]` for a private parameter slot, or a `lea r11,
/// [rip+disp32]` + `mov [root+offset], r11` pair for a record-field
/// destination — then the canonical 5-byte call.
pub(crate) const X86_64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH: u8 = 4;
pub(crate) const X86_64_CALLBACK_MATERIALIZATION_BYTE_COUNT: u16 = 7;
pub(crate) const X86_64_CALLBACK_FIELD_MATERIALIZATION_BYTE_COUNT: u16 = 14;
pub(crate) const X86_64_CALLBACK_MATERIALIZATION_RELOCATION_OFFSET: u16 = 3;

/// Where one callback materialization's resolved private-function address
/// lands: an ABI parameter register, an outgoing stack slot at pre-call RSP,
/// or a private field inside a register-resident record. Memory destinations
/// use scratch `r11`; the retained call row excludes its overlap with inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64CallbackMaterializationDestination {
    Register(calling_conventions::MachineRegister),
    OutgoingStack {
        byte_offset: u32,
    },
    RecordField {
        root_register: calling_conventions::MachineRegister,
        byte_offset: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64NormalizedForeignCallFixupKind {
    Relative32FromNextInstructionToNormalizedForeignImportV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64NormalizedForeignCallFixupState {
    UnresolvedImportFieldV1,
}

/// Target-owned private-callback materialization record: a `lea
/// <destination>, [rip+disp32]` prefix (optionally followed by the
/// record-field slot store) whose displacement field binds the selected
/// private function symbol at object construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64CallbackAddressMaterialization {
    pub destination: X86_64CallbackMaterializationDestination,
    pub materialization_byte_offset: u16,
    pub materialization_byte_count: u16,
    pub relocation_byte_offset: u16,
}

/// Target-owned unresolved import-field fixup for one normalized foreign
/// call. `{boundary, ordinal}` names the selected roster row owning the
/// evaluated locator; no raw foreign coordinate appears here. One entry per
/// callback destination the call's private schedule names, in evaluated
/// roster order.
pub type X86_64NormalizedForeignCallFixup = Fixup<
    X86_64NormalizedForeignCallFixupKind,
    X86_64NormalizedForeignCallFixupState,
    X86_64CallbackAddressMaterialization,
>;

/// Exact selected inputs and canonical bytes for one unresolved normalized
/// foreign call.
pub type ValidatedX86_64SelectedNormalizedForeignCallTemplate =
    Template<X86_64NormalizedForeignCallFixup>;

/// Every way an AMD64 normalized-foreign-call template can disagree with the
/// selected record it claims to encode.
pub type X86_64NormalizedForeignCallTemplateError = normalized_foreign_call_plan::Error;

/// Produce `E8 + zero rel32` with an explicit unresolved import-field fixup.
/// `callback_destinations` names each private callback destination in the
/// call's evaluated roster order; every entry emits its materialization
/// group ahead of the call bytes and lands a custody record in the fixup.
pub fn encode_x86_64_selected_normalized_foreign_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    effects: &MachineEncodedEffects,
    callback_destinations: &[X86_64CallbackMaterializationDestination],
    record: &SelectedNormalizedForeignCall,
) -> Result<
    ValidatedX86_64SelectedNormalizedForeignCallTemplate,
    X86_64NormalizedForeignCallTemplateError,
> {
    let (boundary, ordinal) = match kind {
        SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => (boundary, ordinal),
        _ => return Err(X86_64NormalizedForeignCallTemplateError::InstructionKindMismatch),
    };
    let mut bytes = Vec::new();
    for destination in callback_destinations {
        bytes.extend_from_slice(
            &callback_group_bytes(*destination)
                .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?,
        );
    }
    bytes.extend_from_slice(&[0xe8, 0, 0, 0, 0]);
    let fixup = callback_fixup(boundary, ordinal, callback_destinations)?;
    validate_x86_64_selected_normalized_foreign_call_template(
        target,
        physical,
        kind,
        alternative,
        operand_views,
        operand_accesses,
        effects,
        &bytes,
        fixup,
        callback_destinations,
        record,
    )
}

/// Independently validate every selected input, byte, and unresolved fixup.
/// `callback_destinations` is the same evaluated roster the encoder received:
/// it must equal the exact destinations retained by the independently checked call.
pub fn validate_x86_64_selected_normalized_foreign_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    effects: &MachineEncodedEffects,
    bytes: &[u8],
    fixup: X86_64NormalizedForeignCallFixup,
    callback_destinations: &[X86_64CallbackMaterializationDestination],
    record: &SelectedNormalizedForeignCall,
) -> Result<
    ValidatedX86_64SelectedNormalizedForeignCallTemplate,
    X86_64NormalizedForeignCallTemplateError,
> {
    let (boundary, ordinal) = normalized_foreign_call_plan::admit(
        target,
        physical,
        kind,
        alternative,
        operand_views,
        operand_accesses,
        effects,
        record,
        |native| x86_64_selected_abi(native).is_ok(),
        crate::canonical_x86_64_physical_register_model_identity,
        crate::machine_effects::normalized_foreign_constraint,
        crate::x86_64_normalized_foreign_call_declaration,
    )?;
    let destinations = record
        .call
        .callbacks
        .iter()
        .map(|callback| {
            match (
                &callback.destination,
                callback.application.placement.locations.as_slice(),
            ) {
                (
                    calling_conventions::NativePlace::Parameter(_),
                    [calling_conventions::ValueLocation::Register { register, .. }],
                ) => Some(X86_64CallbackMaterializationDestination::Register(
                    *register,
                )),
                (
                    calling_conventions::NativePlace::Parameter(_),
                    [
                        calling_conventions::ValueLocation::Stack {
                            stack_byte_offset, ..
                        },
                    ],
                ) => Some(X86_64CallbackMaterializationDestination::OutgoingStack {
                    byte_offset: *stack_byte_offset,
                }),
                (
                    calling_conventions::NativePlace::Field { .. },
                    [calling_conventions::ValueLocation::Register { register, .. }],
                ) => Some(X86_64CallbackMaterializationDestination::RecordField {
                    root_register: *register,
                    byte_offset: u32::try_from(callback.field_byte_offset?).ok()?,
                }),
                _ => None,
            }
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch)?;
    if destinations != callback_destinations {
        return Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch);
    }
    let mut expected_bytes = Vec::new();
    for destination in callback_destinations {
        expected_bytes.extend_from_slice(
            &callback_group_bytes(*destination)
                .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?,
        );
    }
    expected_bytes.extend_from_slice(&[0xe8, 0, 0, 0, 0]);
    if bytes != expected_bytes.as_slice() {
        return Err(X86_64NormalizedForeignCallTemplateError::MalformedTemplate);
    }
    let expected_fixup = callback_fixup(boundary, ordinal, callback_destinations)?;
    if fixup != expected_fixup {
        return Err(X86_64NormalizedForeignCallTemplateError::FixupMismatch);
    }
    Ok(Template::new(
        kind,
        alternative,
        operand_views,
        effects,
        bytes,
        fixup,
    ))
}

/// One materialization group: `REX.W 8D /r` — `lea <destination>,
/// [rip+disp32]` — for a private parameter slot, or `lea r11,
/// [rip+disp32]` followed by `mov [root+disp32], r11` for a record-field
/// destination. The lea's zero disp32 is the relocation field; the field
/// store's displacement is the literal private-slot offset.
fn callback_group_bytes(destination: X86_64CallbackMaterializationDestination) -> Option<Vec<u8>> {
    match destination {
        X86_64CallbackMaterializationDestination::Register(register) => {
            let index = x86_64_machine_register_encoding(register)?;
            let rex = 0x48 | (u8::from(index >= 8) << 2);
            let modrm = 0x05 | ((index & 7) << 3);
            Some(vec![rex, 0x8d, modrm, 0, 0, 0, 0])
        }
        X86_64CallbackMaterializationDestination::OutgoingStack { byte_offset } => {
            if byte_offset > i32::MAX as u32 {
                return None;
            }
            // The offset names the caller's outgoing area at pre-call RSP,
            // not the callee's post-CALL stack or its private frame.
            let mut group = vec![0x4c, 0x8d, 0x1d, 0, 0, 0, 0, 0x4c, 0x89];
            if byte_offset <= i8::MAX as u32 {
                group.extend_from_slice(&[0x5c, 0x24, byte_offset as u8]);
            } else {
                group.extend_from_slice(&[0x9c, 0x24]);
                crate::codec_bytes::push_encoded(&mut group, byte_offset.to_le_bytes());
            }
            Some(group)
        }
        X86_64CallbackMaterializationDestination::RecordField {
            root_register,
            byte_offset,
        } => {
            let root = x86_64_machine_register_encoding(root_register)?;
            // The scratch is overwritten by LEA before the root is observed.
            if root == 11 || byte_offset > i32::MAX as u32 {
                return None;
            }
            // `4C 8D 1D` — lea r11, [rip+disp32] — then `4C|B 89 /r` storing
            // r11 at [root + byte_offset] with a mod=10 disp32 form, which
            // R12 additionally needs the no-index SIB byte.
            let mut group = vec![0x4c, 0x8d, 0x1d, 0, 0, 0, 0];
            let rex = 0x4c | u8::from(root >= 8);
            let modrm = 0x98 | (root & 7);
            group.extend_from_slice(&[rex, 0x89, modrm]);
            if root & 7 == 4 {
                group.push(0x24);
            }
            crate::codec_bytes::push_encoded(&mut group, byte_offset.to_le_bytes());
            Some(group)
        }
    }
}

pub(super) fn x86_64_machine_register_encoding(
    register: calling_conventions::MachineRegister,
) -> Option<u8> {
    Some(match register {
        calling_conventions::MachineRegister::X86Rax => 0,
        calling_conventions::MachineRegister::X86Rcx => 1,
        calling_conventions::MachineRegister::X86Rdx => 2,
        calling_conventions::MachineRegister::X86Rbx => 3,
        calling_conventions::MachineRegister::X86Rsi => 6,
        calling_conventions::MachineRegister::X86Rdi => 7,
        calling_conventions::MachineRegister::X86R8 => 8,
        calling_conventions::MachineRegister::X86R9 => 9,
        calling_conventions::MachineRegister::X86R10 => 10,
        calling_conventions::MachineRegister::X86R11 => 11,
        calling_conventions::MachineRegister::X86R12 => 12,
        calling_conventions::MachineRegister::X86R13 => 13,
        calling_conventions::MachineRegister::X86R14 => 14,
        calling_conventions::MachineRegister::X86R15 => 15,
        _ => return None,
    })
}

fn callback_fixup(
    boundary: BoundaryMachineId,
    ordinal: u32,
    destinations: &[X86_64CallbackMaterializationDestination],
) -> Result<X86_64NormalizedForeignCallFixup, X86_64NormalizedForeignCallTemplateError> {
    let mut callbacks = Vec::with_capacity(destinations.len());
    let mut offset = 0_u16;
    for destination in destinations {
        let byte_count = match destination {
            X86_64CallbackMaterializationDestination::Register(_) => {
                X86_64_CALLBACK_MATERIALIZATION_BYTE_COUNT
            }
            X86_64CallbackMaterializationDestination::OutgoingStack { byte_offset } => {
                if *byte_offset <= i8::MAX as u32 {
                    12
                } else {
                    15
                }
            }
            X86_64CallbackMaterializationDestination::RecordField { root_register, .. } => {
                X86_64_CALLBACK_FIELD_MATERIALIZATION_BYTE_COUNT
                    + u16::from(*root_register == calling_conventions::MachineRegister::X86R12)
            }
        };
        callbacks.push(X86_64CallbackAddressMaterialization {
            destination: *destination,
            materialization_byte_offset: offset,
            materialization_byte_count: byte_count,
            relocation_byte_offset: offset
                .checked_add(X86_64_CALLBACK_MATERIALIZATION_RELOCATION_OFFSET)
                .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?,
        });
        offset = offset
            .checked_add(byte_count)
            .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?;
    }
    let patch_byte_offset = offset
        .checked_add(1)
        .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?;
    let reference_byte_offset = offset
        .checked_add(5)
        .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?;
    Ok(X86_64NormalizedForeignCallFixup {
        kind: X86_64NormalizedForeignCallFixupKind::Relative32FromNextInstructionToNormalizedForeignImportV1,
        state: X86_64NormalizedForeignCallFixupState::UnresolvedImportFieldV1,
        boundary,
        ordinal,
        opcode_byte_offset: offset,
        patch_byte_offset,
        reference_byte_offset,
        patch_byte_width: X86_64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH,
        callbacks,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        X86_64NormalizedForeignCallTemplateError, callback_fixup,
        encode_x86_64_selected_normalized_foreign_call_template,
        validate_x86_64_selected_normalized_foreign_call_template,
    };
    use normalized_foreign_call_plan::fixture;
    use register_model::{RegisterOperandAccess, validate_physical_register_model};
    use selected_instructions::SelectedNormalizedForeignCall;
    use semantic_vocabulary::BoundaryMachineId;
    use target::NativeTarget;

    use crate::x86_64_physical_register_model;

    /// The AMD64 spelling of a fixture callback destination, and the byte
    /// counts its two materialization groups charge.
    fn callback_destination(
        placement: fixture::CallbackPlacement,
    ) -> super::X86_64CallbackMaterializationDestination {
        match placement.field_byte_offset {
            None => super::X86_64CallbackMaterializationDestination::Register(placement.register),
            Some(byte_offset) => super::X86_64CallbackMaterializationDestination::RecordField {
                root_register: placement.register,
                byte_offset: u32::try_from(byte_offset).unwrap(),
            },
        }
    }

    const CALLBACK_BYTE_COUNTS: fixture::CallbackByteCounts = fixture::CallbackByteCounts {
        register: 7,
        field: 14,
    };

    fn inputs(windows: bool, input_layout: usize, result: usize) -> fixture::Case {
        fixture::case(
            validate_physical_register_model(x86_64_physical_register_model()).unwrap(),
            record(windows, input_layout, result),
            crate::machine_effects::normalized_foreign_constraint,
            |row, physical| {
                crate::machine_effects::user_direct_call_effects(
                    row,
                    physical.model().view_named("rsp").unwrap().id,
                )
            },
        )
    }

    /// These records carry physical call inputs only; source ownership and
    /// declarations remain the selection tests' independent responsibility.
    fn record(windows: bool, input_layout: usize, result: usize) -> SelectedNormalizedForeignCall {
        use calling_conventions::ValueShape;
        use target::TargetProfile;

        fixture::record(
            if windows {
                NativeTarget::windows_x64()
            } else {
                NativeTarget::linux_x64()
            },
            7,
            if windows {
                ValueShape::integer(16, 8)
            } else {
                ValueShape::system_v_aggregate(
                    16,
                    8,
                    calling_conventions::SystemVEightbyteClass::Integer,
                    calling_conventions::SystemVEightbyteClass::Sse,
                )
            },
            if windows {
                fixture::pe_locator()
            } else {
                fixture::elf_locator(TargetProfile::LinuxX64)
            },
            input_layout,
            result,
        )
    }

    /// The AMD64 spelling of one template admission: the callback destination
    /// vocabulary, the fixup vocabulary, and the fixup's own column record.
    type Entrances = fixture::Entrances<
        super::X86_64CallbackMaterializationDestination,
        super::X86_64NormalizedForeignCallFixupKind,
        super::X86_64NormalizedForeignCallFixupState,
        super::X86_64CallbackAddressMaterialization,
    >;

    /// The AMD64 entrances the shared template bodies run: the `E8` call and
    /// its rel32 patch, the callback groups' byte table, and the fixup columns
    /// the encoder reports for them.
    fn entrances() -> Entrances {
        fixture::Entrances {
            call: fixture::CallBytes {
                bytes: vec![0xe8, 0, 0, 0, 0],
                patch_byte_offset: 1,
                reference_byte_offset: 5,
            },
            constraint: crate::machine_effects::normalized_foreign_constraint,
            declaration: crate::x86_64_normalized_foreign_call_declaration,
            fixed_view: crate::x86_64_fixed_register_view,
            encode: encode_x86_64_selected_normalized_foreign_call_template,
            validate: validate_x86_64_selected_normalized_foreign_call_template,
            fixup: |boundary, ordinal, destinations| {
                callback_fixup(boundary, ordinal, destinations).ok()
            },
            fixup_row: |callback| {
                (
                    callback.destination,
                    callback.materialization_byte_offset,
                    callback.materialization_byte_count,
                )
            },
            advance: |callback| callback.materialization_byte_offset += 1,
        }
    }

    #[test]
    fn table_calls_encode_the_retained_field_and_reject_scratch_aliases() {
        use super::super::table_call::{
            encode_x86_64_selected_table_call_form, validate_x86_64_selected_table_call_form,
        };
        for windows in [false, true] {
            for byte_offset in [0, 8, 65_536] {
                let (_, _, alternative, operands, accesses, _, mut record) = inputs(windows, 1, 1);
                let same_stack_contribution = record
                    .call
                    .binding
                    .same_stack_contribution()
                    .unwrap()
                    .clone();
                record.call.binding.mechanism =
                    target_operations::NormalizedForeignCallMechanism::Table {
                        target: if windows {
                            target::TargetProfile::WindowsX64
                        } else {
                            target::TargetProfile::LinuxX64
                        },
                        byte_offset,
                        dispatch_only: false,
                        same_stack_contribution,
                    };
                let encoded = encode_x86_64_selected_table_call_form(
                    &record,
                    alternative,
                    &operands,
                    &accesses,
                )
                .unwrap();
                assert_eq!(encoded.bytes.len(), 10);
                assert_eq!(&encoded.bytes[3..7], &byte_offset.to_le_bytes());
                assert_eq!(&encoded.bytes[7..], &[0x41, 0xff, 0xd3]);
                for position in 0..encoded.bytes.len() {
                    let mut corrupted = encoded.bytes.clone();
                    corrupted[position] ^= 1;
                    assert!(
                        validate_x86_64_selected_table_call_form(
                            &record,
                            alternative,
                            &operands,
                            &accesses,
                            &corrupted,
                        )
                        .is_err(),
                        "changed byte {position}"
                    );
                }
                let calling_conventions::ValueLocation::Register { register, .. } =
                    &mut record.call.binding.boundary_entry_plan.call.parameters[0].locations[0]
                else {
                    panic!("one pointer argument must be in a register");
                };
                *register = calling_conventions::MachineRegister::X86R11;
                assert!(
                    encode_x86_64_selected_table_call_form(
                        &record,
                        alternative,
                        &operands,
                        &accesses,
                    )
                    .is_err()
                );
            }
        }
    }

    #[test]
    fn dispatch_only_table_is_loaded_from_scratch_without_shifting_wire_arguments() {
        use super::super::table_call::{
            encode_x86_64_selected_table_call_form, validate_x86_64_selected_table_call_form,
        };
        for windows in [false, true] {
            for wire_count in [0, 1] {
                let (_, _, alternative, _, _, _, mut record) = inputs(windows, wire_count, 1);
                let same_stack_contribution = record
                    .call
                    .binding
                    .same_stack_contribution()
                    .unwrap()
                    .clone();
                let original_plan = record.call.binding.boundary_entry_plan.clone();
                record.call.binding.mechanism =
                    target_operations::NormalizedForeignCallMechanism::Table {
                        target: if windows {
                            target::TargetProfile::WindowsX64
                        } else {
                            target::TargetProfile::LinuxX64
                        },
                        byte_offset: 16,
                        dispatch_only: true,
                        same_stack_contribution,
                    };
                let constraint =
                    crate::machine_effects::normalized_foreign_constraint(&record).unwrap();
                let views: Vec<_> = constraint
                    .operands
                    .iter()
                    .map(|operand| operand.fixed_view.unwrap())
                    .collect();
                let accesses: Vec<_> = constraint
                    .operands
                    .iter()
                    .map(|operand| operand.access)
                    .collect();
                let encoded =
                    encode_x86_64_selected_table_call_form(&record, alternative, &views, &accesses)
                        .unwrap();
                assert_eq!(
                    encoded.bytes,
                    [0x4d, 0x8b, 0x9b, 16, 0, 0, 0, 0x41, 0xff, 0xd3]
                );
                assert_eq!(record.call.binding.boundary_entry_plan, original_plan);
                assert_eq!(
                    selected_instructions::calls::constraints::foreign_argument_registers(
                        &record.call
                    )[0],
                    (0, calling_conventions::MachineRegister::X86R11)
                );
                for position in 0..encoded.bytes.len() {
                    let mut corrupted = encoded.bytes.clone();
                    corrupted[position] ^= 1;
                    assert!(
                        validate_x86_64_selected_table_call_form(
                            &record,
                            alternative,
                            &views,
                            &accesses,
                            &corrupted
                        )
                        .is_err()
                    );
                }
            }
        }
    }

    #[test]
    fn retained_multiple_callbacks_bind_operands_effects_bytes_and_fixups() {
        fixture::retained_callbacks(
            &entrances(),
            inputs,
            callback_destination,
            CALLBACK_BYTE_COUNTS,
        );
    }

    #[test]
    fn direct_and_hidden_aggregate_results_share_the_import_template() {
        fixture::direct_and_hidden_aggregate_results_share_the_import_template(
            &entrances(),
            inputs,
        );
    }

    #[test]
    fn callback_stack_bytes_preserve_outgoing_offsets_and_fixup_extent() {
        use super::{X86_64CallbackMaterializationDestination, callback_group_bytes};
        for (offset, store) in [
            (0, vec![0x4c, 0x89, 0x5c, 0x24, 0]),
            (32, vec![0x4c, 0x89, 0x5c, 0x24, 32]),
            (127, vec![0x4c, 0x89, 0x5c, 0x24, 127]),
            (128, vec![0x4c, 0x89, 0x9c, 0x24, 128, 0, 0, 0]),
            (256, vec![0x4c, 0x89, 0x9c, 0x24, 0, 1, 0, 0]),
        ] {
            let destination = X86_64CallbackMaterializationDestination::OutgoingStack {
                byte_offset: offset,
            };
            let bytes = callback_group_bytes(destination).unwrap();
            assert_eq!(&bytes[..7], &[0x4c, 0x8d, 0x1d, 0, 0, 0, 0]);
            assert_eq!(&bytes[7..], store);
            let fixup =
                callback_fixup(BoundaryMachineId::new(3).unwrap(), 0, &[destination]).unwrap();
            assert_eq!(usize::from(fixup.opcode_byte_offset), bytes.len());
            assert_eq!(
                usize::from(fixup.callbacks[0].materialization_byte_count),
                bytes.len()
            );
            assert_eq!(fixup.callbacks[0].relocation_byte_offset, 3);
        }
        assert!(
            callback_group_bytes(X86_64CallbackMaterializationDestination::OutgoingStack {
                byte_offset: i32::MAX as u32 + 1,
            })
            .is_none()
        );
    }

    #[test]
    fn callback_field_bytes_preserve_root_addressing_and_fixup_extent() {
        use super::{X86_64CallbackMaterializationDestination, callback_group_bytes};
        use calling_conventions::MachineRegister;
        for (root, length) in [(MachineRegister::X86Rdi, 14), (MachineRegister::X86R12, 15)] {
            let destination = X86_64CallbackMaterializationDestination::RecordField {
                root_register: root,
                byte_offset: 24,
            };
            let bytes = callback_group_bytes(destination).unwrap();
            assert_eq!(bytes.len(), length);
            assert_eq!(&bytes[..7], &[0x4c, 0x8d, 0x1d, 0, 0, 0, 0]);
            assert_eq!(&bytes[length - 4..], &24_u32.to_le_bytes());
            if root == MachineRegister::X86R12 {
                assert_eq!(&bytes[7..11], &[0x4d, 0x89, 0x9c, 0x24]);
            }
            let fixup =
                callback_fixup(BoundaryMachineId::new(3).unwrap(), 0, &[destination]).unwrap();
            assert_eq!(usize::from(fixup.opcode_byte_offset), length);
            assert_eq!(
                usize::from(fixup.callbacks[0].materialization_byte_count),
                length
            );
        }
        for (root, byte_offset) in [
            (MachineRegister::X86R11, 0),
            (MachineRegister::X86Rdi, u32::MAX),
        ] {
            assert!(
                callback_group_bytes(X86_64CallbackMaterializationDestination::RecordField {
                    root_register: root,
                    byte_offset
                })
                .is_none()
            );
        }
    }

    #[test]
    fn template_has_exact_bytes_and_fixup() {
        fixture::template_has_exact_bytes_and_fixup(&entrances(), inputs);
    }

    #[test]
    fn evaluated_plans_select_exact_operands_and_effects_under_one_template() {
        fixture::evaluated_plans_select_exact_operands_and_effects_under_one_template(
            &entrances(),
            inputs,
            0..63,
            "r11",
        );
    }

    #[test]
    fn microsoft_calls_select_their_exact_operands() {
        let (physical, kind, alternative, operands, accesses, effects, record) = inputs(true, 2, 1);
        assert!(
            encode_x86_64_selected_normalized_foreign_call_template(
                NativeTarget::windows_x64(),
                &physical,
                kind,
                alternative,
                &operands,
                &accesses,
                &effects,
                &[],
                &record
            )
            .is_ok()
        );
        assert_eq!(
            encode_x86_64_selected_normalized_foreign_call_template(
                NativeTarget::windows_x64(),
                &physical,
                kind,
                alternative,
                &[physical.model().view_named("rdi").unwrap().id],
                &[RegisterOperandAccess::Use],
                &effects,
                &[],
                &record
            ),
            Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch)
        );
    }

    #[test]
    fn rejects_wrong_kind_target_alternative_and_fixup() {
        fixture::rejects_wrong_kind_target_alternative_and_fixup(
            &entrances(),
            inputs,
            &[NativeTarget::macos_arm64(), NativeTarget::linux_arm64()],
            &[0xe8, 0, 0, 0, 1],
        );
    }
}
