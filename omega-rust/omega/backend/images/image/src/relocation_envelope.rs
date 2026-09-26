//! Proof after the fact that patching touched only relocation fields, by masking
//! every slot the plan declared and comparing everything that is left.

use crate::{
    CompilerTextDerivationDigest, CompilerTextRelocationEnvelopeDigest,
    CompilerTextValidationEvidence, EncodedCompilerTextDigest, FinalCompilerTextDigest,
};
use diagnostics::Diagnostic;
use object_file::{RelocationKind, RelocationPlan, SectionKind};
use sha2::{Digest, Sha256};

/// Prove that final initialized data preserves every encoded byte except the
/// exact eight-byte slots named by checked absolute data relocations.
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

/// Prove that final `.text` preserves every encoded bit except the exact
/// immediate fields named by checked relocation records.
pub fn validate_final_text_relocation_envelope(
    encoded_text_bytes: &[u8],
    final_text_bytes: &[u8],
    relocations: &RelocationPlan,
) -> Result<CompilerTextValidationEvidence, Diagnostic> {
    if final_text_bytes.len() < encoded_text_bytes.len() {
        return Err(Diagnostic::error(format!(
            "relocated .text truncated compiler code from {} to {} byte(s)",
            encoded_text_bytes.len(),
            final_text_bytes.len()
        )));
    }
    let final_compiler_text = &final_text_bytes[..encoded_text_bytes.len()];
    let mut mutable_bits = vec![0u8; encoded_text_bytes.len()];
    let mut text_relocations = Vec::new();
    for (_, relocation) in relocations.records() {
        if relocation.section != SectionKind::Text {
            continue;
        }
        let (expected_width, masks): (usize, &[u8]) = match relocation.kind {
            RelocationKind::X86_64Relative32 => (4, &[0xff; 4]),
            RelocationKind::Absolute64 => (8, &[0xff; 8]),
            RelocationKind::Aarch64Page21 => (4, &[0xe0, 0xff, 0xff, 0x60]),
            RelocationKind::Aarch64PageOffset12 => (4, &[0x00, 0xfc, 0x3f, 0x00]),
            RelocationKind::Aarch64Branch26 => (4, &[0xff, 0xff, 0xff, 0x03]),
        };
        if relocation.byte_width != expected_width {
            return Err(Diagnostic::error(format!(
                "text relocation at byte {} has width {}, expected {} for {:?}",
                relocation.offset, relocation.byte_width, expected_width, relocation.kind
            )));
        }
        let end = relocation
            .offset
            .checked_add(expected_width)
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
        text_relocations.push((
            relocation.offset,
            relocation.byte_width,
            relocation_kind_tag(relocation.kind),
            relocation.addend,
        ));
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

    text_relocations.sort_unstable();
    let encoded_text_digest = EncodedCompilerTextDigest::from_digest(digest_bytes(
        b"omega.encoded-compiler-text.sha256.v1\0",
        encoded_text_bytes,
    ));
    let final_compiler_text_digest = FinalCompilerTextDigest::from_digest(digest_bytes(
        b"omega.final-compiler-text.sha256.v1\0",
        final_compiler_text,
    ));
    let encoded_text_report_fingerprint = report_fingerprint_bytes(encoded_text_bytes);
    let final_compiler_text_report_fingerprint = report_fingerprint_bytes(final_compiler_text);
    let mut relocation_digest = Sha256::new();
    relocation_digest.update(b"omega.compiler-text-relocation-envelope.sha256.v1\0");
    relocation_digest.update((text_relocations.len() as u64).to_le_bytes());
    let mut relocation_envelope_report_fingerprint = FNV_OFFSET;
    for (offset, width, kind, addend) in &text_relocations {
        relocation_digest.update((*offset as u64).to_le_bytes());
        relocation_digest.update((*width as u64).to_le_bytes());
        relocation_digest.update([*kind]);
        relocation_digest.update(addend.to_le_bytes());
        fingerprint_into(
            &mut relocation_envelope_report_fingerprint,
            &(*offset as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut relocation_envelope_report_fingerprint,
            &(*width as u64).to_le_bytes(),
        );
        fingerprint_into(&mut relocation_envelope_report_fingerprint, &[*kind]);
        fingerprint_into(
            &mut relocation_envelope_report_fingerprint,
            &addend.to_le_bytes(),
        );
    }
    let mut derivation_report_fingerprint = FNV_OFFSET;
    fingerprint_into(
        &mut derivation_report_fingerprint,
        &encoded_text_report_fingerprint.to_le_bytes(),
    );
    fingerprint_into(
        &mut derivation_report_fingerprint,
        &final_compiler_text_report_fingerprint.to_le_bytes(),
    );
    fingerprint_into(
        &mut derivation_report_fingerprint,
        &relocation_envelope_report_fingerprint.to_le_bytes(),
    );
    fingerprint_into(
        &mut derivation_report_fingerprint,
        &(text_relocations.len() as u64).to_le_bytes(),
    );
    let mut evidence = CompilerTextValidationEvidence {
        encoded_text_digest,
        final_compiler_text_digest,
        relocation_envelope_digest: CompilerTextRelocationEnvelopeDigest::from_digest(
            relocation_digest.finalize().into(),
        ),
        derivation_digest: CompilerTextDerivationDigest::from_digest([0; 32]),
        encoded_text_report_fingerprint,
        final_compiler_text_report_fingerprint,
        relocation_envelope_report_fingerprint,
        checked_instruction_validation_report_fingerprint: 0,
        checked_instruction_footprint_report_fingerprint: 0,
        derivation_report_fingerprint,
        text_relocation_count: text_relocations.len(),
        checked_instruction_validation_count: 0,
    };
    evidence.derivation_digest = evidence.recomputed_derivation_digest();
    Ok(evidence)
}

fn relocation_kind_tag(kind: RelocationKind) -> u8 {
    match kind {
        RelocationKind::Aarch64Page21 => 1,
        RelocationKind::Aarch64PageOffset12 => 2,
        RelocationKind::Aarch64Branch26 => 3,
        RelocationKind::Absolute64 => 4,
        RelocationKind::X86_64Relative32 => 5,
    }
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

fn report_fingerprint_bytes(bytes: &[u8]) -> u64 {
    let mut fingerprint = FNV_OFFSET;
    fingerprint_into(&mut fingerprint, bytes);
    fingerprint
}

fn fingerprint_into(fingerprint: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *fingerprint ^= u64::from(*byte);
        *fingerprint = fingerprint.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn digest_bytes(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
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
        final_data[0..8].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
        final_data[16..24].copy_from_slice(&0x8877_6655_4433_2211u64.to_le_bytes());
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
    fn initialized_data_envelope_preserves_empty_data_behavior_and_exact_length() {
        let relocations = RelocationPlan::with_target(NativeTarget::host());
        validate_final_initialized_data_relocation_envelope(&[], &[], &relocations)
            .expect("empty initialized data remains valid");
        assert!(
            validate_final_initialized_data_relocation_envelope(&[], &[0], &relocations).is_err()
        );
        assert!(
            validate_final_initialized_data_relocation_envelope(&[0], &[], &relocations).is_err()
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

    #[test]
    fn initialized_data_envelope_ignores_text_relocation_fields() {
        let encoded = vec![0x5a; 8];
        let mut relocations = RelocationPlan::with_target(NativeTarget::host());
        let mut relocation = data_relocation(0);
        relocation.section = SectionKind::Text;
        relocation.kind = RelocationKind::X86_64Relative32;
        relocation.byte_width = 4;
        relocations.push_record(relocation);

        validate_final_initialized_data_relocation_envelope(&encoded, &encoded, &relocations)
            .expect("text relocation fields belong to the text envelope");
    }
}
