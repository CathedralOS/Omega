//! Canonical ELF64-LSB serialization of the semantic `.dynamic` table.
//!
//! The primary System V ABI defines [`Elf64_Dyn`] as one signed eight-byte tag
//! followed by an eight-byte value/address union, while the generic ELF [data
//! encoding] defines least-significant-byte-first serialization. This module
//! preserves all eight address fields as exact zero placeholders with typed
//! byte-coordinate fixups; it assigns no address or section index.
//!
//! [`Elf64_Dyn`]: https://gabi.xinuos.com/elf/08-dynamic.html#dynamic-section
//! [data encoding]: https://gabi.xinuos.com/elf/02-eheader.html#data-encoding

use target::compatibility_fingerprint::Fnv1a;

use super::super::checked::{self, CandidateValidationError};
use crate::bytes::write_u64;
use crate::constants::{
    ELF64_DYNAMIC_ROW_SIZE, ELF64_DYNAMIC_VALUE_OFFSET, ELF64_DYNAMIC_VALUE_SIZE,
};
use crate::dynamic_executable::checked::{
    checked_product, checked_sum, decode_dynamic_rows, read_u64, require,
};
use crate::dynamic_executable::dynamic_table::dynamic_tags::{
    ElfDynamicAddressTarget, ElfDynamicTagContents, ElfDynamicValue, ValidatedElfDynamicTagPlan,
    encoded_value,
};
use diagnostics::Diagnostic;

/// Independently decoded and replayed ELF64-LSB `.dynamic` bytes.
///
/// The exact semantic tag plan remains owned by this non-clone carrier. Every
/// address union is still zero and named by a typed fixup; these bytes have no
/// descriptor, placement, program header, image mutation, or runnable-image
/// authority.
#[derive(Debug)]
#[must_use = "validated ELF dynamic bytes retain the exact semantic tag plan"]
pub struct ValidatedElfDynamicTablePayload {
    plan: ValidatedElfDynamicTagPlan,
    contents: ElfDynamicTablePayloadContents,
    non_authoritative_payload_compatibility_fingerprint: u64,
}

impl ValidatedElfDynamicTablePayload {
    pub const fn plan(&self) -> &ValidatedElfDynamicTagPlan {
        &self.plan
    }

    pub fn row_count(&self) -> usize {
        self.plan.row_count()
    }

    pub fn byte_count(&self) -> usize {
        self.contents.bytes.len()
    }
    /// Compatibility fingerprint of the exact semantic tag identity,
    /// ELF64-LSB bytes, and typed byte-coordinate fixups. This is not final-
    /// byte, placement, loader, or publication identity.
    pub const fn non_authoritative_payload_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_payload_compatibility_fingerprint
    }

    pub(crate) const fn contents(&self) -> &ElfDynamicTablePayloadContents {
        &self.contents
    }

    pub(crate) fn into_parts(self) -> (ValidatedElfDynamicTagPlan, ElfDynamicTablePayloadContents) {
        (self.plan, self.contents)
    }
}

/// Rejected `.dynamic` serialization with exact semantic-plan custody.
#[derive(Debug)]
#[must_use = "ELF dynamic serialization rejection retains the semantic plan"]
pub struct ElfDynamicTableSerializationError {
    plan: ValidatedElfDynamicTagPlan,
    diagnostic: Diagnostic,
}

impl ElfDynamicTableSerializationError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfDynamicTagPlan, Diagnostic) {
        (self.plan, self.diagnostic)
    }
}

diagnostics::diagnostic_display!(ElfDynamicTableSerializationError);

impl std::error::Error for ElfDynamicTableSerializationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfDynamicTablePayloadContents {
    pub(crate) bytes: Vec<u8>,
    pub(crate) address_fixups: Vec<ElfDynamicPayloadAddressFixup>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ElfDynamicPayloadFixupKind {
    Elf64AbsoluteAddress = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfDynamicPayloadAddressFixup {
    pub(crate) row_ordinal: u32,
    pub(crate) byte_offset: usize,
    pub(crate) byte_width: u8,
    pub(crate) kind: ElfDynamicPayloadFixupKind,
    pub(crate) target: ElfDynamicAddressTarget,
}

struct Candidate {
    plan: ValidatedElfDynamicTagPlan,
    contents: ElfDynamicTablePayloadContents,
    non_authoritative_payload_compatibility_fingerprint: u64,
}

/// Serialize the exact semantic tag plan as ELF64-LSB `Elf64_Dyn` rows and
/// independently decode every row and address-fixup coordinate before sealing
/// success.
///
/// This does not append a `.dynamic` name, create its descriptor, assign a
/// section index/address, resolve a pointer, place bytes, or mutate the image.
pub(crate) fn serialize_elf_dynamic_table(
    plan: ValidatedElfDynamicTagPlan,
) -> Result<ValidatedElfDynamicTablePayload, Box<ElfDynamicTableSerializationError>> {
    checked::plan_candidate(
        plan,
        encode_contents,
        |plan, contents| non_authoritative_payload_compatibility_fingerprint(&plan, &contents),
        |plan, contents, fingerprint| Candidate {
            plan: plan,
            contents: contents,
            non_authoritative_payload_compatibility_fingerprint: fingerprint,
        },
        validate_candidate,
        |candidate| candidate.plan,
        |plan, diagnostic| {
            Box::new(ElfDynamicTableSerializationError {
                plan: plan,
                diagnostic,
            })
        },
    )
}

fn encode_contents(
    plan: &ValidatedElfDynamicTagPlan,
) -> Result<ElfDynamicTablePayloadContents, Diagnostic> {
    let semantic = plan.contents();
    let mut bytes = Vec::with_capacity(checked_product(
        semantic.rows.len(),
        ELF64_DYNAMIC_ROW_SIZE,
        "ELF64 dynamic payload size",
    )?);
    for row in &semantic.rows {
        write_u64(&mut bytes, (row.tag as i64) as u64);
        write_u64(&mut bytes, encoded_value(row.value));
    }
    let address_fixups = semantic
        .address_obligations
        .iter()
        .map(|obligation| {
            let row_offset = checked_product(
                obligation.row_ordinal as usize,
                ELF64_DYNAMIC_ROW_SIZE,
                "ELF64 dynamic fixup row offset",
            )?;
            Ok(ElfDynamicPayloadAddressFixup {
                row_ordinal: obligation.row_ordinal,
                byte_offset: checked_sum(
                    row_offset,
                    ELF64_DYNAMIC_VALUE_OFFSET,
                    "ELF64 dynamic fixup value offset",
                )?,
                byte_width: obligation.byte_width,
                kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                target: obligation.target,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok(ElfDynamicTablePayloadContents {
        bytes,
        address_fixups,
    })
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfDynamicTablePayload, CandidateValidationError<Candidate>> {
    checked::validate_candidate(
        candidate,
        |candidate| validate_contents(&candidate.plan, &candidate.contents),
        |candidate| {
            candidate.non_authoritative_payload_compatibility_fingerprint
                == non_authoritative_payload_compatibility_fingerprint(
                    &candidate.plan,
                    &candidate.contents,
                )
        },
        "ELF dynamic payload compatibility fingerprint does not replay",
        |candidate| ValidatedElfDynamicTablePayload {
            plan: candidate.plan,
            contents: candidate.contents,
            non_authoritative_payload_compatibility_fingerprint: candidate
                .non_authoritative_payload_compatibility_fingerprint,
        },
    )
}

fn validate_contents(
    plan: &ValidatedElfDynamicTagPlan,
    contents: &ElfDynamicTablePayloadContents,
) -> Result<(), Diagnostic> {
    let semantic = plan.contents();
    let decoded = decode_dynamic_rows(&contents.bytes, semantic.rows.len())?;
    require(
        decoded.len() == semantic.rows.len(),
        "decoded Elf64_Dyn row count drifted from the semantic plan",
    )?;
    for (decoded, semantic) in decoded.iter().zip(&semantic.rows) {
        require(
            decoded.tag == semantic.tag as i64 && decoded.value == encoded_value(semantic.value),
            "decoded Elf64_Dyn tag or value drifted from the semantic row",
        )?;
    }
    validate_fixups(&contents.bytes, semantic, &contents.address_fixups)
}

fn validate_fixups(
    bytes: &[u8],
    semantic: &ElfDynamicTagContents,
    fixups: &[ElfDynamicPayloadAddressFixup],
) -> Result<(), Diagnostic> {
    require(
        fixups.len() == semantic.address_obligations.len(),
        "serialized ELF dynamic address-fixup count is not exact",
    )?;
    for (fixup, obligation) in fixups.iter().zip(&semantic.address_obligations) {
        let row_offset = checked_product(
            obligation.row_ordinal as usize,
            ELF64_DYNAMIC_ROW_SIZE,
            "replayed Elf64_Dyn fixup row",
        )?;
        let expected_offset = checked_sum(
            row_offset,
            ELF64_DYNAMIC_VALUE_OFFSET,
            "replayed Elf64_Dyn fixup value",
        )?;
        require(
            fixup.row_ordinal == obligation.row_ordinal
                && fixup.byte_offset == expected_offset
                && fixup.byte_width == ELF64_DYNAMIC_VALUE_SIZE
                && fixup.byte_width == obligation.byte_width
                && fixup.kind == ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress
                && fixup.target == obligation.target,
            "serialized ELF dynamic address fixup drifted from its semantic obligation",
        )?;
        require(
            read_u64(bytes, fixup.byte_offset, "Elf64_Dyn address placeholder")? == 0,
            "serialized Elf64_Dyn address field is not an exact zero placeholder",
        )?;
    }
    for (index, row) in semantic.rows.iter().enumerate() {
        let fixup_count = fixups
            .iter()
            .filter(|fixup| fixup.row_ordinal as usize == index)
            .count();
        require(
            fixup_count == usize::from(matches!(row.value, ElfDynamicValue::AddressPlaceholder)),
            "serialized Elf64_Dyn row has a missing, duplicate, or orphan address fixup",
        )?;
    }
    validate_fixup_coverage(fixups)
}

fn validate_fixup_coverage(fixups: &[ElfDynamicPayloadAddressFixup]) -> Result<(), Diagnostic> {
    for (index, fixup) in fixups.iter().enumerate() {
        let end = checked_sum(
            fixup.byte_offset,
            usize::from(fixup.byte_width),
            "Elf64_Dyn fixup end",
        )?;
        for other in &fixups[index + 1..] {
            let other_end = checked_sum(
                other.byte_offset,
                usize::from(other.byte_width),
                "Elf64_Dyn fixup end",
            )?;
            require(
                end <= other.byte_offset || other_end <= fixup.byte_offset,
                "serialized Elf64_Dyn address fixups overlap or duplicate one field",
            )?;
        }
    }
    Ok(())
}

fn non_authoritative_payload_compatibility_fingerprint(
    plan: &ValidatedElfDynamicTagPlan,
    contents: &ElfDynamicTablePayloadContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-dynamic-table-payload.v1");
    hash.bytes(
        &plan
            .non_authoritative_tag_compatibility_fingerprint()
            .to_le_bytes(),
    );
    hash.bytes(&contents.bytes);
    hash.bytes(&(contents.address_fixups.len() as u64).to_le_bytes());
    for fixup in &contents.address_fixups {
        hash.bytes(&fixup.row_ordinal.to_le_bytes());
        hash.bytes(&(fixup.byte_offset as u64).to_le_bytes());
        hash.byte(fixup.byte_width);
        hash.byte(fixup.kind as u8);
        hash.byte(fixup.target as u8);
    }
    hash.finish()
}

#[cfg(test)]
mod tests {
    use super::{
        Candidate, ELF64_DYNAMIC_ROW_SIZE, ELF64_DYNAMIC_VALUE_OFFSET, ElfDynamicAddressTarget,
        ElfDynamicPayloadAddressFixup, ElfDynamicPayloadFixupKind, ElfDynamicValue,
        ValidatedElfDynamicTagPlan, checked_product, checked_sum, decode_dynamic_rows,
        encode_contents, non_authoritative_payload_compatibility_fingerprint, read_u64,
        serialize_elf_dynamic_table, validate_candidate, validate_contents,
        validate_fixup_coverage,
    };
    use crate::dynamic_executable::checked::read_i64;
    use target::TargetProfile;

    const IMPORTS: [crate::test_support::DynamicImportFixture; 2] =
        crate::test_support::PAIR_IMPORTS;

    fn tag_plan(
        target: TargetProfile,
        imports: &[crate::test_support::DynamicImportFixture],
    ) -> ValidatedElfDynamicTagPlan {
        crate::test_support::dynamic_tag_plan(crate::test_support::dynamic_import_inputs(
            target,
            imports,
            "__omega_dynamic_byte_import_",
            "valid dynamic-byte locator",
            "valid dynamic-byte interpreter",
        ))
    }

    fn candidate(target: TargetProfile) -> Candidate {
        let plan = tag_plan(target, &IMPORTS);
        let contents = encode_contents(&plan).expect("encoded dynamic bytes");
        let non_authoritative_payload_compatibility_fingerprint =
            non_authoritative_payload_compatibility_fingerprint(&plan, &contents);
        Candidate {
            plan,
            contents,
            non_authoritative_payload_compatibility_fingerprint,
        }
    }

    fn row(bytes: &[u8], ordinal: usize) -> &[u8] {
        let start = ordinal * ELF64_DYNAMIC_ROW_SIZE;
        &bytes[start..start + ELF64_DYNAMIC_ROW_SIZE]
    }

    #[test]
    fn both_targets_serialize_exact_elf64_lsb_rows_and_fixup_coordinates() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let payload = serialize_elf_dynamic_table(tag_plan(target, &IMPORTS))
                .expect("validated dynamic payload");
            assert_eq!(payload.row_count(), 19);
            assert_eq!(payload.byte_count(), 304);
            assert_eq!(payload.contents().address_fixups.len(), 9);
            assert_ne!(
                payload.non_authoritative_payload_compatibility_fingerprint(),
                0
            );

            let bytes = &payload.contents.bytes;
            let first_needed = match payload.plan.contents().rows[0].value {
                ElfDynamicValue::NeededStringOffset(offset) => offset,
                _ => panic!("first row must be DT_NEEDED"),
            };
            assert_eq!(&row(bytes, 0)[..8], &[1, 0, 0, 0, 0, 0, 0, 0]);
            assert_eq!(
                &row(bytes, 0)[8..],
                &(u64::from(first_needed)).to_le_bytes(),
            );
            assert_eq!(&row(bytes, 5)[..8], &[0xf5, 0xfe, 0xff, 0x6f, 0, 0, 0, 0]);
            assert_eq!(&row(bytes, 10)[..8], &[20, 0, 0, 0, 0, 0, 0, 0]);
            assert_eq!(&row(bytes, 10)[8..], &[7, 0, 0, 0, 0, 0, 0, 0]);
            assert_eq!(&row(bytes, 12)[..8], &[7, 0, 0, 0, 0, 0, 0, 0]);
            assert_eq!(
                row(bytes, 13),
                &[8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            );
            assert_eq!(
                row(bytes, 14),
                &[9, 0, 0, 0, 0, 0, 0, 0, 24, 0, 0, 0, 0, 0, 0, 0]
            );
            assert_eq!(&row(bytes, 15)[..8], &[0xf0, 0xff, 0xff, 0x6f, 0, 0, 0, 0],);
            assert_eq!(row(bytes, 18), &[0; 16]);
            assert_eq!(
                payload.contents.address_fixups,
                [
                    ElfDynamicPayloadAddressFixup {
                        row_ordinal: 3,
                        byte_offset: 56,
                        byte_width: 8,
                        kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                        target: ElfDynamicAddressTarget::ProcedureGot,
                    },
                    ElfDynamicPayloadAddressFixup {
                        row_ordinal: 4,
                        byte_offset: 72,
                        byte_width: 8,
                        kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                        target: ElfDynamicAddressTarget::SystemVHash,
                    },
                    ElfDynamicPayloadAddressFixup {
                        row_ordinal: 5,
                        byte_offset: 88,
                        byte_width: 8,
                        kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                        target: ElfDynamicAddressTarget::GnuHash,
                    },
                    ElfDynamicPayloadAddressFixup {
                        row_ordinal: 6,
                        byte_offset: 104,
                        byte_width: 8,
                        kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                        target: ElfDynamicAddressTarget::DynamicString,
                    },
                    ElfDynamicPayloadAddressFixup {
                        row_ordinal: 7,
                        byte_offset: 120,
                        byte_width: 8,
                        kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                        target: ElfDynamicAddressTarget::DynamicSymbol,
                    },
                    ElfDynamicPayloadAddressFixup {
                        row_ordinal: 11,
                        byte_offset: 184,
                        byte_width: 8,
                        kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                        target: ElfDynamicAddressTarget::ProcedureRelocation,
                    },
                    ElfDynamicPayloadAddressFixup {
                        row_ordinal: 12,
                        byte_offset: 200,
                        byte_width: 8,
                        kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                        target: ElfDynamicAddressTarget::GeneralRelocation,
                    },
                    ElfDynamicPayloadAddressFixup {
                        row_ordinal: 15,
                        byte_offset: 248,
                        byte_width: 8,
                        kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                        target: ElfDynamicAddressTarget::GnuSymbolVersion,
                    },
                    ElfDynamicPayloadAddressFixup {
                        row_ordinal: 16,
                        byte_offset: 264,
                        byte_width: 8,
                        kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                        target: ElfDynamicAddressTarget::GnuVersionRequirement,
                    },
                ],
            );
            validate_contents(payload.plan(), &payload.contents)
                .expect("independent dynamic-byte replay");
        }
    }

    #[test]
    fn encoded_needed_offsets_still_select_exact_raw_non_utf8_names() {
        let payload =
            serialize_elf_dynamic_table(tag_plan(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
        let dynstr = &payload
            .plan
            .descriptors()
            .templates()
            .linkage()
            .descriptors()
            .payloads()
            .payloads()
            .dynstr;
        let names = (0..payload.plan.needed_row_count())
            .map(|ordinal| {
                let offset = read_u64(
                    &payload.contents.bytes,
                    ordinal * ELF64_DYNAMIC_ROW_SIZE + ELF64_DYNAMIC_VALUE_OFFSET,
                    "DT_NEEDED offset",
                )
                .unwrap() as usize;
                let tail = &dynstr[offset..];
                &tail[..tail.iter().position(|byte| *byte == 0).unwrap()]
            })
            .collect::<Vec<_>>();
        assert_eq!(names, [b"liba\xff.so".as_slice(), b"libb.so".as_slice()]);
    }

    #[test]
    fn import_permutation_preserves_bytes_while_target_remains_identity_bound() {
        let forward =
            serialize_elf_dynamic_table(tag_plan(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
        let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
        let reverse =
            serialize_elf_dynamic_table(tag_plan(TargetProfile::LinuxX64, &reverse_imports))
                .unwrap();
        let arm =
            serialize_elf_dynamic_table(tag_plan(TargetProfile::LinuxArm64, &IMPORTS)).unwrap();
        assert_eq!(forward.contents, reverse.contents);
        assert_eq!(
            forward.non_authoritative_payload_compatibility_fingerprint(),
            reverse.non_authoritative_payload_compatibility_fingerprint()
        );
        assert_eq!(forward.contents.bytes, arm.contents.bytes);
        assert_ne!(
            forward.non_authoritative_payload_compatibility_fingerprint(),
            arm.non_authoritative_payload_compatibility_fingerprint()
        );
    }

    #[test]
    fn independent_decoder_rejects_every_tag_value_placeholder_and_boundary_corruption() {
        let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
            Box::new(|candidate| candidate.contents.bytes[0] ^= 1),
            Box::new(|candidate| candidate.contents.bytes[8] ^= 1),
            Box::new(|candidate| candidate.contents.bytes[2 * 16 + 8] ^= 1),
            Box::new(|candidate| candidate.contents.bytes[3 * 16 + 8] = 1),
            Box::new(|candidate| candidate.contents.bytes[8 * 16 + 8] ^= 1),
            Box::new(|candidate| candidate.contents.bytes[9 * 16 + 8] ^= 1),
            Box::new(|candidate| candidate.contents.bytes[10 * 16 + 8] ^= 1),
            Box::new(|candidate| candidate.contents.bytes[14 * 16 + 8] ^= 1),
            Box::new(|candidate| candidate.contents.bytes[17 * 16 + 8] ^= 1),
            Box::new(|candidate| candidate.contents.bytes[18 * 16] = 1),
            Box::new(|candidate| candidate.contents.bytes[18 * 16 + 8] = 1),
            Box::new(|candidate| candidate.contents.bytes[12 * 16..12 * 16 + 8].reverse()),
            Box::new(|candidate| {
                candidate.contents.bytes.pop();
            }),
            Box::new(|candidate| candidate.contents.bytes.push(0)),
            Box::new(|candidate| {
                candidate.non_authoritative_payload_compatibility_fingerprint ^= 1
            }),
        ];
        for corrupt in corruptions {
            let mut candidate = candidate(TargetProfile::LinuxX64);
            let expected_identity = candidate
                .plan
                .non_authoritative_tag_compatibility_fingerprint();
            corrupt(&mut candidate);
            let error =
                validate_candidate(candidate).expect_err("corrupt Elf64_Dyn payload must reject");
            assert_eq!(
                error
                    .candidate
                    .plan
                    .non_authoritative_tag_compatibility_fingerprint(),
                expected_identity,
                "dynamic-byte rejection retains exact semantic-plan custody",
            );
        }
    }

    #[test]
    fn independent_replay_rejects_every_fixup_coordinate_kind_and_target_corruption() {
        let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
            Box::new(|candidate| {
                candidate.contents.address_fixups.pop();
            }),
            Box::new(|candidate| {
                candidate
                    .contents
                    .address_fixups
                    .push(candidate.contents.address_fixups[0])
            }),
            Box::new(|candidate| candidate.contents.address_fixups.swap(0, 1)),
            Box::new(|candidate| candidate.contents.address_fixups[0].row_ordinal = u32::MAX),
            Box::new(|candidate| candidate.contents.address_fixups[0].byte_offset += 1),
            Box::new(|candidate| candidate.contents.address_fixups[0].byte_width = 4),
            Box::new(|candidate| {
                candidate.contents.address_fixups[0].target = ElfDynamicAddressTarget::DynamicString
            }),
        ];
        for corrupt in corruptions {
            let mut candidate = candidate(TargetProfile::LinuxArm64);
            let expected_identity = candidate
                .plan
                .non_authoritative_tag_compatibility_fingerprint();
            corrupt(&mut candidate);
            let error = validate_candidate(candidate)
                .expect_err("corrupt Elf64_Dyn address fixup must reject");
            assert_eq!(
                error
                    .candidate
                    .plan
                    .non_authoritative_tag_compatibility_fingerprint(),
                expected_identity
            );
        }
    }

    #[test]
    fn decoder_and_fixup_checks_reject_overflow_and_malformed_inputs_without_panicking() {
        assert!(checked_product(usize::MAX, 16, "product").is_err());
        assert!(checked_sum(usize::MAX, 8, "sum").is_err());
        assert!(decode_dynamic_rows(&[], usize::MAX).is_err());
        assert!(decode_dynamic_rows(&[0; 15], 1).is_err());
        assert!(decode_dynamic_rows(&[0; 17], 1).is_err());
        assert!(read_i64(&[], usize::MAX, "tag").is_err());
        assert!(read_u64(&[0; 7], 0, "value").is_err());

        let overflow = ElfDynamicPayloadAddressFixup {
            row_ordinal: u32::MAX,
            byte_offset: usize::MAX,
            byte_width: 8,
            kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
            target: ElfDynamicAddressTarget::ProcedureGot,
        };
        assert!(validate_fixup_coverage(&[overflow, overflow]).is_err());
    }
}
