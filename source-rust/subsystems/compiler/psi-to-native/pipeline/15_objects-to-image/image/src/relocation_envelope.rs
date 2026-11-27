/*
We prove after relocation patching that only declared relocation bitfields
changed between the compiler-encoded bytes and the final relocated bytes. For
`.text`, we derive per-byte bitmasks directly from `relocation_field_geometry`
so the patcher and the envelope verifier share a single source of truth; for
`.data`, we verify that changes occur exclusively inside non-overlapping,
eight-byte-aligned `Absolute64` slots.
*/

use crate::relocations::relocation_field_geometry;
use diagnostics::Diagnostic;
use object_file::{RelocationKind, RelocationPlan, SectionKind};

pub fn validate_final_initialized_data_relocation_envelope(
    encoded_data_bytes: &[u8],
    final_data_bytes: &[u8],
    relocations: &RelocationPlan,
) -> Result<(), Diagnostic> {
    if final_data_bytes.len() != encoded_data_bytes.len() {
        return Err(Diagnostic::error(format!(
            "relocated initialized data has {} byte(s), expected exactly {}",
            final_data_bytes.len(),
            encoded_data_bytes.len()
        )));
    }

    let mut mutable_bytes = vec![false; encoded_data_bytes.len()];
    for (_, relocation) in relocations.records() {
        if relocation.section != SectionKind::Data {
            continue;
        }
        if relocation.kind != RelocationKind::Absolute64 || relocation.byte_width != 8 {
            return Err(Diagnostic::error(format!(
                "initialized-data relocation at byte {} must be an Absolute64 width-8 slot",
                relocation.offset
            )));
        }
        if relocation.offset % 8 != 0 {
            return Err(Diagnostic::error(format!(
                "initialized-data Absolute64 relocation at byte {} is not eight-byte aligned",
                relocation.offset
            )));
        }
        let end = relocation
            .offset
            .checked_add(8)
            .filter(|end| *end <= mutable_bytes.len())
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "initialized-data relocation at byte {} exceeds encoded data",
                    relocation.offset
                ))
            })?;
        if mutable_bytes[relocation.offset..end]
            .iter()
            .any(|mutable| *mutable)
        {
            return Err(Diagnostic::error(format!(
                "initialized-data relocation at byte {} overlaps another relocation slot",
                relocation.offset
            )));
        }
        mutable_bytes[relocation.offset..end].fill(true);
    }

    for (offset, ((encoded, final_byte), mutable)) in encoded_data_bytes
        .iter()
        .zip(final_data_bytes)
        .zip(&mutable_bytes)
        .enumerate()
    {
        if !mutable && encoded != final_byte {
            return Err(Diagnostic::error(format!(
                "final initialized-data byte {offset} changed outside a declared Absolute64 relocation slot"
            )));
        }
    }
    Ok(())
}

pub fn validate_final_text_relocation_envelope(
    encoded_text_bytes: &[u8],
    final_text_bytes: &[u8],
    relocations: &RelocationPlan,
) -> Result<(), Diagnostic> {
    if final_text_bytes.len() < encoded_text_bytes.len() {
        return Err(Diagnostic::error(format!(
            "relocated .text truncated compiler code from {} to {} byte(s)",
            encoded_text_bytes.len(),
            final_text_bytes.len()
        )));
    }
    let final_compiler_text = &final_text_bytes[..encoded_text_bytes.len()];
    let mut mutable_bits = vec![0u8; encoded_text_bytes.len()];
    for (_, relocation) in relocations.records() {
        if relocation.section != SectionKind::Text {
            continue;
        }
        let geometry = relocation_field_geometry(relocation.kind);
        if relocation.byte_width != geometry.byte_width {
            return Err(Diagnostic::error(format!(
                "text relocation at byte {} has width {}, expected {} for {:?}",
                relocation.offset, relocation.byte_width, geometry.byte_width, relocation.kind
            )));
        }
        let mask_bytes = geometry.mask.to_le_bytes();
        let masks = &mask_bytes[..geometry.byte_width];
        let end = relocation
            .offset
            .checked_add(geometry.byte_width)
            .filter(|end| *end <= mutable_bits.len())
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "text relocation at byte {} exceeds encoded .text",
                    relocation.offset
                ))
            })?;
        if mutable_bits[relocation.offset..end]
            .iter()
            .any(|mask| *mask != 0)
        {
            return Err(Diagnostic::error(format!(
                "text relocation at byte {} overlaps another relocation field",
                relocation.offset
            )));
        }
        mutable_bits[relocation.offset..end].copy_from_slice(masks);
    }

    for (offset, ((encoded, final_byte), mutable_mask)) in encoded_text_bytes
        .iter()
        .zip(final_compiler_text)
        .zip(&mutable_bits)
        .enumerate()
    {
        let changed_bits = encoded ^ final_byte;
        if changed_bits & !mutable_mask != 0 {
            return Err(Diagnostic::error(format!(
                "final compiler .text byte {offset} changed outside its declared relocation field"
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_final_initialized_data_relocation_envelope;
    use object_file::{
        ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationPlan, RelocationRecord,
        SectionKind,
    };
    use target::NativeTarget;

    fn data_relocation(offset: usize) -> RelocationRecord {
        RelocationRecord {
            origin: RelocationOrigin::Materialization {
                object_symbol_handle: ObjectSymbolHandle::invalid(),
            },
            section: SectionKind::Data,
            offset,
            byte_width: 8,
            symbol_handle: ObjectSymbolHandle::invalid(),
            addend: 0,
            kind: RelocationKind::Absolute64,
        }
    }

    #[test]
    fn initialized_data_envelope_accepts_only_declared_slot_mutation() {
        let encoded = vec![0x5a; 24];
        let mut final_data = encoded.clone();
        crate::codec_bytes::patch_encoded(
            &mut final_data,
            0,
            0x1122_3344_5566_7788u64.to_le_bytes(),
        );
        crate::codec_bytes::patch_encoded(
            &mut final_data,
            16,
            0x8877_6655_4433_2211u64.to_le_bytes(),
        );
        let mut relocations = RelocationPlan::with_target(NativeTarget::host());
        relocations.push_record(data_relocation(0));
        relocations.push_record(data_relocation(16));

        validate_final_initialized_data_relocation_envelope(&encoded, &final_data, &relocations)
            .expect("exact declared data slots may change");

        final_data[8] ^= 1;
        assert!(
            validate_final_initialized_data_relocation_envelope(
                &encoded,
                &final_data,
                &relocations,
            )
            .is_err(),
            "mutation outside a declared slot must be rejected"
        );
    }

    #[test]
    fn initialized_data_envelope_rejects_malformed_or_overlapping_slots() {
        let encoded = vec![0; 24];

        let mut wrong_kind = RelocationPlan::with_target(NativeTarget::host());
        let mut relocation = data_relocation(0);
        relocation.kind = RelocationKind::X86_64Relative32;
        wrong_kind.push_record(relocation);
        assert!(
            validate_final_initialized_data_relocation_envelope(&encoded, &encoded, &wrong_kind,)
                .is_err()
        );

        let mut wrong_width = RelocationPlan::with_target(NativeTarget::host());
        let mut relocation = data_relocation(0);
        relocation.byte_width = 4;
        wrong_width.push_record(relocation);
        assert!(
            validate_final_initialized_data_relocation_envelope(&encoded, &encoded, &wrong_width,)
                .is_err()
        );

        let mut misaligned = RelocationPlan::with_target(NativeTarget::host());
        misaligned.push_record(data_relocation(1));
        assert!(
            validate_final_initialized_data_relocation_envelope(&encoded, &encoded, &misaligned,)
                .is_err()
        );

        let mut overlapping = RelocationPlan::with_target(NativeTarget::host());
        overlapping.push_record(data_relocation(8));
        overlapping.push_record(data_relocation(8));
        assert!(
            validate_final_initialized_data_relocation_envelope(&encoded, &encoded, &overlapping,)
                .is_err()
        );

        let mut out_of_bounds = RelocationPlan::with_target(NativeTarget::host());
        out_of_bounds.push_record(data_relocation(24));
        assert!(
            validate_final_initialized_data_relocation_envelope(
                &encoded,
                &encoded,
                &out_of_bounds,
            )
            .is_err()
        );
    }
}
