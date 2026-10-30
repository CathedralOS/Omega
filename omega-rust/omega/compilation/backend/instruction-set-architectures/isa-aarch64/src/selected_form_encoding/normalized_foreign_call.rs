//! Canonical unresolved AArch64 normalized-foreign-call templates.
//!
//! The emitted word is the same `BL #0` placeholder an internal call uses,
//! but the fixup names the selected `{boundary, ordinal}` roster row rather
//! than an internal machine. Private callbacks materialize their function
//! address before the call word: one `adrp`/`add` pair per register
//! destination, and that pair plus a `str` through the architecture-fixed
//! scratch register for a record-field destination. The field stays
//! unresolved until object construction binds it to the declared import
//! symbol; raw locator bytes never enter this encoding contract.
//!
//! The admission both instruction sets run, the fixup record, and the validated
//! template record live in `normalized_foreign_call_plan`; this module keeps the
//! AArch64 callback materialization words, the `BL #0` emission, and the fixup
//! vocabulary that names them.

use normalized_foreign_call_plan::{Fixup, Template};
use register_model::{RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionKind,
    SelectedNormalizedForeignCall,
};
use semantic_vocabulary::BoundaryMachineId;
use target::NativeTarget;

use crate::codec_bytes::push_encoded;
use crate::machine_effects::aarch64_selected_abi;

pub(crate) const AARCH64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH: u8 = 4;

/// One register-destination materialization group: an `adrp`/`add` pair.
/// A record-field destination adds the scratch store word after its pair.
pub(crate) const AARCH64_CALLBACK_MATERIALIZATION_BYTE_COUNT: u16 = 8;
pub(crate) const AARCH64_CALLBACK_FIELD_MATERIALIZATION_BYTE_COUNT: u16 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64NormalizedForeignCallFixupKind {
    BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64NormalizedForeignCallFixupState {
    UnresolvedImportFieldV1,
}

/// Where one private callback address lands when the call executes: the
/// parameter register the provider plan assigned, or a field inside the
/// record addressed by `root_register` at `byte_offset`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64CallbackMaterializationDestination {
    Register(calling_conventions::MachineRegister),
    RecordField {
        root_register: calling_conventions::MachineRegister,
        byte_offset: u32,
    },
}

/// Target-owned private-callback materialization record: an `adrp`/`add`
/// pair whose page and page-offset fields bind the selected private function
/// symbol at object construction, followed by the scratch store when the
/// destination is a record field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64CallbackAddressMaterialization {
    pub destination: Aarch64CallbackMaterializationDestination,
    pub materialization_byte_offset: u16,
    pub materialization_byte_count: u16,
    pub page_relocation_byte_offset: u16,
    pub page_offset_relocation_byte_offset: u16,
}

/// Target-owned unresolved import-field fixup for one normalized foreign
/// call. `{boundary, ordinal}` names the selected roster row owning the
/// evaluated locator; `callbacks` carries every private-callback
/// materialization in the call's roster order.
pub type Aarch64NormalizedForeignCallFixup = Fixup<
    Aarch64NormalizedForeignCallFixupKind,
    Aarch64NormalizedForeignCallFixupState,
    Aarch64CallbackAddressMaterialization,
>;

/// Exact selected inputs and canonical bytes for one unresolved normalized
/// foreign call.
pub type ValidatedAarch64SelectedNormalizedForeignCallTemplate =
    Template<Aarch64NormalizedForeignCallFixup>;

/// Every way an AArch64 normalized-foreign-call template can disagree with the
/// selected record it claims to encode.
pub type Aarch64NormalizedForeignCallTemplateError = normalized_foreign_call_plan::Error;

/// Produce the canonical `BL #0` word, preceded by one materialization
/// group per callback destination in roster order, with an unresolved
/// import-field fixup naming every group.
pub fn encode_aarch64_selected_normalized_foreign_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    effects: &MachineEncodedEffects,
    callback_destinations: &[Aarch64CallbackMaterializationDestination],
    record: &SelectedNormalizedForeignCall,
) -> Result<
    ValidatedAarch64SelectedNormalizedForeignCallTemplate,
    Aarch64NormalizedForeignCallTemplateError,
> {
    let (boundary, ordinal) = match kind {
        SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => (boundary, ordinal),
        _ => return Err(Aarch64NormalizedForeignCallTemplateError::InstructionKindMismatch),
    };
    let bytes = canonical_call_bytes(callback_destinations)
        .ok_or(Aarch64NormalizedForeignCallTemplateError::AlternativeMismatch)?;
    let fixup = callback_fixup(boundary, ordinal, callback_destinations)
        .ok_or(Aarch64NormalizedForeignCallTemplateError::AlternativeMismatch)?;
    validate_aarch64_selected_normalized_foreign_call_template(
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
pub fn validate_aarch64_selected_normalized_foreign_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    effects: &MachineEncodedEffects,
    bytes: &[u8],
    fixup: Aarch64NormalizedForeignCallFixup,
    callback_destinations: &[Aarch64CallbackMaterializationDestination],
    record: &SelectedNormalizedForeignCall,
) -> Result<
    ValidatedAarch64SelectedNormalizedForeignCallTemplate,
    Aarch64NormalizedForeignCallTemplateError,
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
        |native| aarch64_selected_abi(native).is_ok(),
        crate::canonical_aarch64_physical_register_model_identity,
        crate::machine_effects::normalized_foreign_constraint,
        crate::aarch64_normalized_foreign_call_declaration,
    )?;
    let destinations = record
        .call
        .callbacks
        .iter()
        .map(|callback| {
            let [calling_conventions::ValueLocation::Register { register, .. }] =
                callback.application.placement.locations.as_slice()
            else {
                return None;
            };
            match callback.destination {
                calling_conventions::NativePlace::Parameter(_) => Some(
                    Aarch64CallbackMaterializationDestination::Register(*register),
                ),
                calling_conventions::NativePlace::Field { .. } => {
                    Some(Aarch64CallbackMaterializationDestination::RecordField {
                        root_register: *register,
                        byte_offset: u32::try_from(callback.field_byte_offset?).ok()?,
                    })
                }
            }
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(Aarch64NormalizedForeignCallTemplateError::OperandViewMismatch)?;
    if destinations != callback_destinations {
        return Err(Aarch64NormalizedForeignCallTemplateError::OperandViewMismatch);
    }
    let expected_bytes = canonical_call_bytes(callback_destinations)
        .ok_or(Aarch64NormalizedForeignCallTemplateError::MalformedTemplate)?;
    if bytes != expected_bytes.as_slice() {
        return Err(Aarch64NormalizedForeignCallTemplateError::MalformedTemplate);
    }
    let expected_fixup = callback_fixup(boundary, ordinal, callback_destinations)
        .ok_or(Aarch64NormalizedForeignCallTemplateError::FixupMismatch)?;
    if fixup != expected_fixup {
        return Err(Aarch64NormalizedForeignCallTemplateError::FixupMismatch);
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

/// The canonical unresolved word stream: one materialization group per
/// callback destination in roster order — `adrp`/`add` into the destination
/// register (or the scratch `x16` plus its store) — then `BL #0`.
fn canonical_call_bytes(
    destinations: &[Aarch64CallbackMaterializationDestination],
) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    for destination in destinations {
        match *destination {
            Aarch64CallbackMaterializationDestination::Register(register) => {
                let calling_conventions::MachineRegister::Aarch64X(index) = register else {
                    return None;
                };
                if index > 30 {
                    return None;
                }
                let index = u32::from(index);
                let adrp = (0x9000_0000_u32 | index).to_le_bytes();
                let add = (0x9100_0000_u32 | (index << 5) | index).to_le_bytes();
                push_encoded(&mut bytes, adrp);
                push_encoded(&mut bytes, add);
            }
            Aarch64CallbackMaterializationDestination::RecordField {
                root_register,
                byte_offset,
            } => {
                let calling_conventions::MachineRegister::Aarch64X(root_index) = root_register
                else {
                    return None;
                };
                if root_index > 30 || byte_offset % 8 != 0 || byte_offset / 8 >= 4096 {
                    return None;
                }
                let scratch = 16_u32;
                let adrp = (0x9000_0000_u32 | scratch).to_le_bytes();
                let add = (0x9100_0000_u32 | (scratch << 5) | scratch).to_le_bytes();
                let store = (0xF900_0000_u32
                    | ((byte_offset / 8) << 10)
                    | (u32::from(root_index) << 5)
                    | scratch)
                    .to_le_bytes();
                push_encoded(&mut bytes, adrp);
                push_encoded(&mut bytes, add);
                push_encoded(&mut bytes, store);
            }
        }
    }
    push_encoded(&mut bytes, 0x9400_0000_u32.to_le_bytes());
    Some(bytes)
}

fn callback_fixup(
    boundary: BoundaryMachineId,
    ordinal: u32,
    destinations: &[Aarch64CallbackMaterializationDestination],
) -> Option<Aarch64NormalizedForeignCallFixup> {
    let mut materializations = Vec::with_capacity(destinations.len());
    let mut offset = 0_u16;
    for destination in destinations {
        let byte_count = match destination {
            Aarch64CallbackMaterializationDestination::Register(_) => {
                AARCH64_CALLBACK_MATERIALIZATION_BYTE_COUNT
            }
            Aarch64CallbackMaterializationDestination::RecordField { .. } => {
                AARCH64_CALLBACK_FIELD_MATERIALIZATION_BYTE_COUNT
            }
        };
        materializations.push(Aarch64CallbackAddressMaterialization {
            destination: *destination,
            materialization_byte_offset: offset,
            materialization_byte_count: byte_count,
            page_relocation_byte_offset: offset,
            page_offset_relocation_byte_offset: offset + 4,
        });
        offset = offset.checked_add(byte_count)?;
    }
    Some(Aarch64NormalizedForeignCallFixup {
        kind: Aarch64NormalizedForeignCallFixupKind::BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
        state: Aarch64NormalizedForeignCallFixupState::UnresolvedImportFieldV1,
        boundary,
        ordinal,
        opcode_byte_offset: offset,
        patch_byte_offset: offset,
        reference_byte_offset: offset,
        patch_byte_width: AARCH64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH,
        callbacks: materializations,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        callback_fixup, encode_aarch64_selected_normalized_foreign_call_template,
        validate_aarch64_selected_normalized_foreign_call_template,
    };
    use normalized_foreign_call_plan::fixture;
    use register_model::validate_physical_register_model;
    use selected_instructions::{
        MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
        MachineEncodedStackEffect, MachineEncodedTrapBehavior, SelectedNormalizedForeignCall,
    };
    use target::NativeTarget;

    use crate::aarch64_physical_register_model;

    /// The AArch64 spelling of a fixture callback destination, and the byte
    /// counts its two materialization groups charge.
    fn callback_destination(
        placement: fixture::CallbackPlacement,
    ) -> super::Aarch64CallbackMaterializationDestination {
        match placement.field_byte_offset {
            None => super::Aarch64CallbackMaterializationDestination::Register(placement.register),
            Some(byte_offset) => super::Aarch64CallbackMaterializationDestination::RecordField {
                root_register: placement.register,
                byte_offset: u32::try_from(byte_offset).unwrap(),
            },
        }
    }

    const CALLBACK_BYTE_COUNTS: fixture::CallbackByteCounts = fixture::CallbackByteCounts {
        register: 8,
        field: 12,
    };

    fn inputs(darwin: bool, input_layout: usize, result: usize) -> fixture::Case {
        fixture::case(
            validate_physical_register_model(aarch64_physical_register_model()).unwrap(),
            record(darwin, input_layout, result),
            crate::machine_effects::normalized_foreign_constraint,
            |row, _| MachineEncodedEffects {
                external_operand_reads: row.operand_reads(),
                external_operand_writes: row.operand_writes(),
                implicit_unit_uses: row.implicit_uses.clone(),
                implicit_unit_defs: row.implicit_defs.clone(),
                implicit_unit_clobbers: row.clobbers.clone(),
                memory: MachineEncodedMemoryEffect::NoneV1,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::DirectRelativeCallV1,
            },
        )
    }

    /// These records carry physical call inputs only; source ownership and
    /// declarations remain the selection tests' independent responsibility.
    fn record(darwin: bool, input_layout: usize, result: usize) -> SelectedNormalizedForeignCall {
        use calling_conventions::ValueShape;
        use target::TargetProfile;

        fixture::record(
            if darwin {
                NativeTarget::macos_arm64()
            } else {
                NativeTarget::linux_arm64()
            },
            9,
            ValueShape::homogeneous_float_aggregate(8, 2),
            if darwin {
                fixture::mach_o_locator()
            } else {
                fixture::elf_locator(TargetProfile::LinuxArm64)
            },
            input_layout,
            result,
        )
    }

    /// The AArch64 spelling of one template admission: the callback
    /// destination vocabulary, the fixup vocabulary, and the fixup's own
    /// column record.
    type Entrances = fixture::Entrances<
        super::Aarch64CallbackMaterializationDestination,
        super::Aarch64NormalizedForeignCallFixupKind,
        super::Aarch64NormalizedForeignCallFixupState,
        super::Aarch64CallbackAddressMaterialization,
    >;

    /// The AArch64 entrances the shared template bodies run: the `BL #0` word
    /// and its in-place patch, the callback groups' byte table, and the fixup
    /// columns the encoder reports for them.
    fn entrances() -> Entrances {
        fixture::Entrances {
            call: fixture::CallBytes {
                bytes: 0x9400_0000_u32.to_le_bytes().to_vec(),
                patch_byte_offset: 0,
                reference_byte_offset: 0,
            },
            constraint: crate::machine_effects::normalized_foreign_constraint,
            declaration: crate::aarch64_normalized_foreign_call_declaration,
            fixed_view: crate::aarch64_fixed_register_view,
            encode: encode_aarch64_selected_normalized_foreign_call_template,
            validate: validate_aarch64_selected_normalized_foreign_call_template,
            fixup: |boundary, ordinal, destinations| {
                callback_fixup(boundary, ordinal, destinations)
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
    fn template_has_exact_bytes_and_fixup() {
        fixture::template_has_exact_bytes_and_fixup(&entrances(), inputs);
    }

    #[test]
    fn evaluated_plans_select_exact_operands_and_effects_under_one_template() {
        fixture::evaluated_plans_select_exact_operands_and_effects_under_one_template(
            &entrances(),
            inputs,
            0..81,
            "x8",
        );
    }

    #[test]
    fn rejects_wrong_kind_target_alternative_and_fixup() {
        fixture::rejects_wrong_kind_target_alternative_and_fixup(
            &entrances(),
            inputs,
            &[
                NativeTarget::linux_x64(),
                NativeTarget::windows_x64(),
                NativeTarget {
                    object_format: target::ObjectFormat::Coff,
                    ..NativeTarget::macos_arm64()
                },
            ],
            &0x1400_0000_u32.to_le_bytes(),
        );
    }
}
