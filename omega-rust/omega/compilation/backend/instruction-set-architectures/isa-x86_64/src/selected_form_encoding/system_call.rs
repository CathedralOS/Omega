//! Linux supervisor entries use the ordinary external-call operand transport.
//!
//! The selected call roster supplies the exact nonnegative kernel number. This
//! ISA owner materializes that number and enters the kernel; it does not stage
//! arguments, manufacture an import relocation, or infer a source pattern.
//! Fixed-width materialization keeps layout independent of the number. Replay
//! decodes the number and instruction fields separately from byte production.

use super::{
    ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError,
    X86_64SelectedFormFootprint,
};
use register_model::{
    RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel, views_with_access,
};
use selected_instructions::{
    MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionKind,
};
use target::NativeTarget;

/// Encode the retained kernel number after ordinary argument placement.
pub fn encode_x86_64_selected_system_call_form(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    number: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let mut bytes = Vec::with_capacity(12);
    bytes.extend_from_slice(&[0x48, 0xb8]);
    crate::codec_bytes::push_encoded(&mut bytes, number.to_le_bytes());
    bytes.extend_from_slice(&[0x0f, 0x05]);
    validate_x86_64_selected_system_call_form(
        target,
        physical,
        kind,
        alternative,
        operand_views,
        operand_accesses,
        number,
        &bytes,
    )
}

/// Decode the materialized number and entry opcode, then reconstruct the
/// canonical ABI footprint without trusting producer-supplied effects.
pub fn validate_x86_64_selected_system_call_form(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    number: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    if target != NativeTarget::linux_x64()
        || number < 0
        || physical.identity() != crate::canonical_x86_64_physical_register_model_identity()
        || !matches!(kind, SelectedInstructionKind::NormalizedForeignCall { .. })
        || alternative != kind.call_alternative_key()
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    if bytes.len() != 12 || bytes[..2] != [0x48, 0xb8] || bytes[10..] != [0x0f, 0x05] {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let immediate: [u8; 8] = bytes[2..10]
        .try_into()
        .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
    if i64::from_le_bytes(immediate) != number {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let catalog = crate::x86_64_register_constraint_catalog_for(physical);
    let keys = crate::x86_64_linux_system_call_keys();
    let row = catalog
        .row_matching(&keys, operand_views, operand_accesses)
        .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
    let effects = MachineEncodedEffects::supervisor_call_v1(row);
    let views_for = |access| views_with_access(operand_views, operand_accesses, access);
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: X86_64SelectedFormFootprint {
            register_reads: views_for(RegisterOperandAccess::Use),
            register_writes: views_for(RegisterOperandAccess::Def),
            writes_rflags: true,
            encoded: effects,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use isa_conformance::system_call::{Encoding, conformance};
    use semantic_vocabulary::BoundaryMachineId;

    #[test]
    fn system_call_encoding_replays_all_transport_rows_and_rejects_substituted_bytes() {
        let physical = crate::validated_x86_64_physical_register_model();
        let target = NativeTarget::linux_x64();
        let catalog = crate::x86_64_register_constraint_catalog_for(physical);
        let kind = SelectedInstructionKind::NormalizedForeignCall {
            boundary: BoundaryMachineId::new(3).unwrap(),
            ordinal: 0,
        };
        let alternative = MachineAlternativeKey {
            family: selected_instructions::MachineAlternativeFamily::NormalizedForeignCall,
            variant: 0,
        };
        let summarize = |encoded: ValidatedX86_64SelectedFormEncoding| Encoding {
            bytes: encoded.bytes().to_vec(),
            effects: encoded.footprint().encoded.clone(),
        };
        let rows: Vec<_> = crate::x86_64_linux_system_call_keys()
            .into_iter()
            .map(|key| catalog.constraint(key).unwrap())
            .collect();
        conformance(
            &rows,
            physical.model().view_named("rsp").unwrap().id,
            |views, accesses, number| {
                encode_x86_64_selected_system_call_form(
                    target,
                    physical,
                    kind,
                    alternative,
                    views,
                    accesses,
                    number,
                )
                .map(summarize)
            },
            |views, accesses, number, bytes| {
                validate_x86_64_selected_system_call_form(
                    target,
                    physical,
                    kind,
                    alternative,
                    views,
                    accesses,
                    number,
                    bytes,
                )
                .map(summarize)
            },
            |views, accesses, number, bytes| {
                validate_x86_64_selected_system_call_form(
                    NativeTarget::windows_x64(),
                    physical,
                    kind,
                    alternative,
                    views,
                    accesses,
                    number,
                    bytes,
                )
                .is_err()
            },
        );
    }
}
