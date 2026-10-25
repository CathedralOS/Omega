//! Address-free ELF section descriptor for the serialized `.dynamic` table.
//!
//! The generic [System V ABI section header] defines `SHT_DYNAMIC`, its
//! writable/allocated flags, and its `sh_link` relationship to the associated
//! string table. The [dynamic section] defines the fixed `Elf64_Dyn` entry
//! size. This layer extends the existing append-only name seed and retains the
//! exact serialized payload without assigning numeric section indexes,
//! addresses, placement, or final `.shstrtab` contents.
//!
//! [System V ABI section header]: https://gabi.xinuos.com/elf/03-sheader.html
//! [dynamic section]: https://gabi.xinuos.com/elf/08-dynamic.html#dynamic-section

use target::compatibility_fingerprint::Fnv1a;

use super::super::checked::{self, CandidateValidationError};
use crate::dynamic_executable::checked::{checked_product, checked_sum, checked_u32, require};
use crate::dynamic_executable::dynamic_table::dynamic_tag_bytes::ValidatedElfDynamicTablePayload;
use crate::dynamic_executable::import_sections::dynamic_section_descriptors::ElfDynamicSectionKind;
use diagnostics::Diagnostic;

use crate::constants::{SHF_ALLOC, SHF_WRITE};
const SHT_DYNAMIC: u32 = 6;
const ELF64_DYNAMIC_ALIGNMENT: u64 = 8;
const ELF64_DYNAMIC_ENTRY_SIZE: u64 = 16;
const UPSTREAM_DESCRIPTOR_COUNT: usize = 11;
const UPSTREAM_NAME_SEED_SIZE: usize = 113;
const DYNAMIC_NAME_OFFSET: u32 = 113;
const DYNAMIC_NAME_SUFFIX: &[u8] = b".dynamic\0";
/// Independently validated address-free metadata for the serialized
/// `.dynamic` payload.
///
/// The extended name seed remains append-only and incomplete: it is not a
/// final `.shstrtab`. This non-clone carrier grants no final `sh_name`,
/// `sh_link`, section index, address, placement, image mutation, publication,
/// or runnable-image authority.
#[derive(Debug)]
#[must_use = "validated ELF dynamic-table descriptor retains the exact payload"]
pub struct ValidatedElfDynamicTableSectionDescriptorPlan {
    payload: ValidatedElfDynamicTablePayload,
    contents: ElfDynamicTableSectionDescriptorContents,
    non_authoritative_descriptor_compatibility_fingerprint: u64,
}

impl ValidatedElfDynamicTableSectionDescriptorPlan {
    pub const fn payload(&self) -> &ValidatedElfDynamicTablePayload {
        &self.payload
    }

    pub const fn descriptor_count(&self) -> usize {
        UPSTREAM_DESCRIPTOR_COUNT + 1
    }

    pub const fn appended_descriptor_count(&self) -> usize {
        1
    }

    pub fn section_name_seed_byte_count(&self) -> usize {
        self.contents.section_name_table_seed.len()
    }

    /// Compatibility fingerprint of the exact serialized payload identity,
    /// append-only name seed, typed link/info semantics, and ABI metadata.
    /// This is a content compatibility coordinate, not layout or image authority.
    pub const fn non_authoritative_descriptor_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_descriptor_compatibility_fingerprint
    }

    pub(crate) const fn contents(&self) -> &ElfDynamicTableSectionDescriptorContents {
        &self.contents
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidatedElfDynamicTablePayload,
        ElfDynamicTableSectionDescriptorContents,
    ) {
        (self.payload, self.contents)
    }
}

/// Rejected `.dynamic` descriptor planning with exact serialized-payload
/// custody.
#[derive(Debug)]
#[must_use = "ELF dynamic-table descriptor rejection retains the serialized payload"]
pub struct ElfDynamicTableSectionDescriptorPlanningError {
    payload: ValidatedElfDynamicTablePayload,
    diagnostic: Diagnostic,
}

image::rejection_carrier!(
    ElfDynamicTableSectionDescriptorPlanningError;
    payload: ValidatedElfDynamicTablePayload,
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfDynamicTableSectionDescriptorContents {
    pub(crate) section_name_table_seed: Vec<u8>,
    pub(crate) descriptor: ElfDynamicTableSectionDescriptor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ElfDynamicTableSectionKind {
    DynamicTable = 1,
}

impl ElfDynamicTableSectionKind {
    const fn name(self) -> &'static [u8] {
        match self {
            Self::DynamicTable => b".dynamic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfDynamicTableSectionDescriptor {
    pub(crate) kind: ElfDynamicTableSectionKind,
    pub(crate) name_offset: u32,
    pub(crate) section_type: u32,
    pub(crate) flags: u64,
    pub(crate) payload_size: u64,
    pub(crate) alignment: u64,
    pub(crate) entry_size: u64,
    pub(crate) link: Option<ElfDynamicSectionKind>,
    pub(crate) info: Option<ElfDynamicSectionKind>,
}

struct Candidate {
    payload: ValidatedElfDynamicTablePayload,
    contents: ElfDynamicTableSectionDescriptorContents,
    non_authoritative_descriptor_compatibility_fingerprint: u64,
}

/// Consume the exact serialized `.dynamic` payload into one address-free
/// semantic section descriptor and append its raw name to the owning seed.
///
/// This does not complete `.shstrtab`, assign final numeric section indexes or
/// addresses, resolve the semantic `.dynstr` link, place bytes, serialize
/// headers, or mutate an image.
pub(crate) fn plan_elf_dynamic_table_section_descriptor(
    payload: ValidatedElfDynamicTablePayload,
) -> Result<
    ValidatedElfDynamicTableSectionDescriptorPlan,
    Box<ElfDynamicTableSectionDescriptorPlanningError>,
> {
    checked::plan_candidate(
        payload,
        derive_contents,
        |payload, contents| {
            non_authoritative_descriptor_compatibility_fingerprint(&payload, &contents)
        },
        |payload, contents, fingerprint| Candidate {
            payload: payload,
            contents: contents,
            non_authoritative_descriptor_compatibility_fingerprint: fingerprint,
        },
        validate_candidate,
        |candidate| candidate.payload,
        |payload, diagnostic| {
            Box::new(ElfDynamicTableSectionDescriptorPlanningError {
                payload: payload,
                diagnostic,
            })
        },
    )
}

fn derive_contents(
    payload: &ValidatedElfDynamicTablePayload,
) -> Result<ElfDynamicTableSectionDescriptorContents, Diagnostic> {
    let base_seed = base_name_seed(payload);
    let name_offset = checked_u32(base_seed.len(), "dynamic-table name offset")?;
    let mut section_name_table_seed = Vec::with_capacity(checked_sum(
        base_seed.len(),
        DYNAMIC_NAME_SUFFIX.len(),
        "dynamic-table name-seed size",
    )?);
    section_name_table_seed.extend_from_slice(base_seed);
    section_name_table_seed.extend_from_slice(DYNAMIC_NAME_SUFFIX);
    let payload_size = u64::try_from(payload.byte_count())
        .map_err(|_| Diagnostic::error("dynamic-table payload size exceeds Elf64_Xword"))?;
    Ok(ElfDynamicTableSectionDescriptorContents {
        section_name_table_seed,
        descriptor: ElfDynamicTableSectionDescriptor {
            kind: ElfDynamicTableSectionKind::DynamicTable,
            name_offset,
            section_type: SHT_DYNAMIC,
            flags: SHF_WRITE | SHF_ALLOC,
            payload_size,
            alignment: ELF64_DYNAMIC_ALIGNMENT,
            entry_size: ELF64_DYNAMIC_ENTRY_SIZE,
            link: Some(ElfDynamicSectionKind::DynamicString),
            info: None,
        },
    })
}

fn base_name_seed(payload: &ValidatedElfDynamicTablePayload) -> &[u8] {
    &payload
        .plan()
        .descriptors()
        .contents()
        .section_name_table_seed
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfDynamicTableSectionDescriptorPlan, CandidateValidationError<Candidate>> {
    checked::validate_candidate(
        candidate,
        |candidate| validate_contents(&candidate.payload, &candidate.contents),
        |candidate| {
            candidate.non_authoritative_descriptor_compatibility_fingerprint
                == non_authoritative_descriptor_compatibility_fingerprint(
                    &candidate.payload,
                    &candidate.contents,
                )
        },
        "ELF dynamic-table descriptor compatibility fingerprint does not replay",
        |candidate| ValidatedElfDynamicTableSectionDescriptorPlan {
            payload: candidate.payload,
            contents: candidate.contents,
            non_authoritative_descriptor_compatibility_fingerprint: candidate
                .non_authoritative_descriptor_compatibility_fingerprint,
        },
    )
}

fn validate_contents(
    payload: &ValidatedElfDynamicTablePayload,
    contents: &ElfDynamicTableSectionDescriptorContents,
) -> Result<(), Diagnostic> {
    let descriptors = payload.plan().descriptors();
    require(
        descriptors.descriptor_count() == UPSTREAM_DESCRIPTOR_COUNT,
        "dynamic-table descriptor requires the exact sealed eleven-row base",
    )?;
    let base_seed = base_name_seed(payload);
    require(
        base_seed.len() == UPSTREAM_NAME_SEED_SIZE,
        "dynamic-table descriptor requires the exact 113-byte upstream name seed",
    )?;
    let expected_seed_len = checked_sum(
        base_seed.len(),
        DYNAMIC_NAME_SUFFIX.len(),
        "validated dynamic-table name-seed size",
    )?;
    require(
        contents.section_name_table_seed.len() == expected_seed_len
            && contents.section_name_table_seed.get(..base_seed.len()) == Some(base_seed)
            && contents.section_name_table_seed.get(base_seed.len()..) == Some(DYNAMIC_NAME_SUFFIX),
        "dynamic-table section-name seed is not an exact append-only extension",
    )?;

    let dynamic_string_count = descriptors
        .templates()
        .linkage()
        .descriptors()
        .contents()
        .descriptors
        .iter()
        .filter(|row| row.kind == ElfDynamicSectionKind::DynamicString)
        .count();
    require(
        dynamic_string_count == 1,
        "dynamic-table descriptor requires exactly one semantic .dynstr target",
    )?;

    let expected_payload_size = checked_product(
        payload.row_count(),
        usize::try_from(ELF64_DYNAMIC_ENTRY_SIZE)
            .map_err(|_| Diagnostic::error("Elf64_Dyn entry size exceeds usize"))?,
        "validated dynamic-table payload size",
    )?;
    require(
        payload.byte_count() == expected_payload_size,
        "dynamic-table payload size is not an exact Elf64_Dyn row multiple",
    )?;

    let row = &contents.descriptor;
    validate_name(&contents.section_name_table_seed, row)?;
    require(
        row.kind == ElfDynamicTableSectionKind::DynamicTable
            && row.name_offset == DYNAMIC_NAME_OFFSET
            && row.section_type == SHT_DYNAMIC
            && row.flags == SHF_WRITE | SHF_ALLOC
            && row.payload_size
                == u64::try_from(payload.byte_count()).map_err(|_| {
                    Diagnostic::error("validated dynamic-table payload exceeds Elf64_Xword")
                })?
            && row.alignment == ELF64_DYNAMIC_ALIGNMENT
            && row.entry_size == ELF64_DYNAMIC_ENTRY_SIZE
            && row.link == Some(ElfDynamicSectionKind::DynamicString)
            && row.info.is_none(),
        "dynamic-table descriptor metadata drifted from its serialized payload",
    )
}

fn validate_name(seed: &[u8], row: &ElfDynamicTableSectionDescriptor) -> Result<(), Diagnostic> {
    let offset = usize::try_from(row.name_offset)
        .map_err(|_| Diagnostic::error("dynamic-table sh_name exceeds usize"))?;
    let tail = seed
        .get(offset..)
        .ok_or_else(|| Diagnostic::error("dynamic-table sh_name is outside the name seed"))?;
    let terminator = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| Diagnostic::error("dynamic-table section name is not NUL-terminated"))?;
    require(
        tail.get(..terminator) == Some(row.kind.name()),
        "dynamic-table sh_name does not select the exact semantic name",
    )
}

fn non_authoritative_descriptor_compatibility_fingerprint(
    payload: &ValidatedElfDynamicTablePayload,
    contents: &ElfDynamicTableSectionDescriptorContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-dynamic-table-section-descriptor.v1");
    hash.bytes(
        &payload
            .non_authoritative_payload_compatibility_fingerprint()
            .to_le_bytes(),
    );
    hash.bytes(&contents.section_name_table_seed);
    let row = &contents.descriptor;
    hash.byte(row.kind as u8);
    hash.bytes(&row.name_offset.to_le_bytes());
    hash.bytes(&row.section_type.to_le_bytes());
    hash.bytes(&row.flags.to_le_bytes());
    hash.bytes(&row.payload_size.to_le_bytes());
    hash.bytes(&row.alignment.to_le_bytes());
    hash.bytes(&row.entry_size.to_le_bytes());
    for kind in [row.link, row.info] {
        match kind {
            Some(kind) => {
                hash.byte(1);
                hash.byte(kind as u8);
            }
            None => hash.byte(0),
        }
    }
    hash.finish()
}

#[cfg(test)]
mod tests {
    use super::{
        Candidate, DYNAMIC_NAME_SUFFIX, ElfDynamicSectionKind, ElfDynamicTableSectionDescriptor,
        ElfDynamicTableSectionKind, SHF_ALLOC, SHF_WRITE, SHT_DYNAMIC, UPSTREAM_NAME_SEED_SIZE,
        ValidatedElfDynamicTablePayload, base_name_seed, checked_product, checked_sum, checked_u32,
        derive_contents, non_authoritative_descriptor_compatibility_fingerprint,
        plan_elf_dynamic_table_section_descriptor, validate_candidate, validate_contents,
        validate_name,
    };
    use crate::serialize_elf_dynamic_table;
    use target::TargetProfile;

    const IMPORTS: [crate::test_support::DynamicImportFixture; 2] =
        crate::test_support::PAIR_IMPORTS;

    fn payload(
        target: TargetProfile,
        imports: &[crate::test_support::DynamicImportFixture],
    ) -> ValidatedElfDynamicTablePayload {
        let tags =
            crate::test_support::dynamic_tag_plan(crate::test_support::dynamic_import_inputs(
                target,
                imports,
                "__omega_dynamic_descriptor_import_",
                "valid dynamic-descriptor locator",
                "valid dynamic-descriptor interpreter",
            ));
        serialize_elf_dynamic_table(tags).expect("valid dynamic payload")
    }

    fn candidate(target: TargetProfile) -> Candidate {
        let payload = payload(target, &IMPORTS);
        let contents = derive_contents(&payload).expect("derived dynamic-table descriptor");
        let non_authoritative_descriptor_compatibility_fingerprint =
            non_authoritative_descriptor_compatibility_fingerprint(&payload, &contents);
        Candidate {
            payload,
            contents,
            non_authoritative_descriptor_compatibility_fingerprint,
        }
    }

    #[test]
    fn both_targets_append_exact_name_and_address_free_dynamic_metadata() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let plan = plan_elf_dynamic_table_section_descriptor(payload(target, &IMPORTS))
                .expect("validated dynamic-table descriptor");
            assert_eq!(plan.payload.plan().descriptors().descriptor_count(), 11);
            assert_eq!(plan.descriptor_count(), 12);
            assert_eq!(plan.appended_descriptor_count(), 1);
            assert_eq!(plan.section_name_seed_byte_count(), 122);
            assert_eq!(
                &plan.contents.section_name_table_seed[..UPSTREAM_NAME_SEED_SIZE],
                base_name_seed(plan.payload()),
            );
            let dynstr = &plan
                .payload()
                .plan()
                .descriptors()
                .templates()
                .linkage()
                .descriptors()
                .payloads()
                .payloads()
                .dynstr;
            assert!(dynstr.windows(8).any(|window| window == b"liba\xff.so"));
            assert_eq!(
                &plan.contents.section_name_table_seed[113..],
                DYNAMIC_NAME_SUFFIX,
            );
            assert_eq!(
                plan.contents.descriptor,
                ElfDynamicTableSectionDescriptor {
                    kind: ElfDynamicTableSectionKind::DynamicTable,
                    name_offset: 113,
                    section_type: SHT_DYNAMIC,
                    flags: SHF_WRITE | SHF_ALLOC,
                    payload_size: 304,
                    alignment: 8,
                    entry_size: 16,
                    link: Some(ElfDynamicSectionKind::DynamicString),
                    info: None,
                },
            );
            assert_ne!(
                plan.non_authoritative_descriptor_compatibility_fingerprint(),
                0
            );
            validate_contents(plan.payload(), &plan.contents)
                .expect("independent dynamic-table descriptor replay");
        }
    }

    #[test]
    fn import_permutation_preserves_descriptor_identity_and_target_is_bound() {
        let forward =
            plan_elf_dynamic_table_section_descriptor(payload(TargetProfile::LinuxX64, &IMPORTS))
                .unwrap();
        let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
        let reverse = plan_elf_dynamic_table_section_descriptor(payload(
            TargetProfile::LinuxX64,
            &reverse_imports,
        ))
        .unwrap();
        let arm =
            plan_elf_dynamic_table_section_descriptor(payload(TargetProfile::LinuxArm64, &IMPORTS))
                .unwrap();
        assert_eq!(forward.contents, reverse.contents);
        assert_eq!(
            forward.non_authoritative_descriptor_compatibility_fingerprint(),
            reverse.non_authoritative_descriptor_compatibility_fingerprint()
        );
        assert_ne!(
            forward.non_authoritative_descriptor_compatibility_fingerprint(),
            arm.non_authoritative_descriptor_compatibility_fingerprint()
        );
    }

    #[test]
    fn independent_replay_rejects_every_descriptor_field_link_info_and_identity_corruption() {
        let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
            Box::new(|candidate| candidate.contents.section_name_table_seed[0] ^= 1),
            Box::new(|candidate| {
                candidate.contents.section_name_table_seed[UPSTREAM_NAME_SEED_SIZE] ^= 1
            }),
            Box::new(|candidate| {
                *candidate
                    .contents
                    .section_name_table_seed
                    .last_mut()
                    .unwrap() ^= 1
            }),
            Box::new(|candidate| {
                candidate.contents.section_name_table_seed.pop();
            }),
            Box::new(|candidate| candidate.contents.section_name_table_seed.push(0)),
            Box::new(|candidate| candidate.contents.descriptor.name_offset = u32::MAX),
            Box::new(|candidate| candidate.contents.descriptor.section_type = 1),
            Box::new(|candidate| candidate.contents.descriptor.flags ^= SHF_WRITE),
            Box::new(|candidate| candidate.contents.descriptor.payload_size += 1),
            Box::new(|candidate| candidate.contents.descriptor.alignment = 1),
            Box::new(|candidate| candidate.contents.descriptor.entry_size = 8),
            Box::new(|candidate| candidate.contents.descriptor.link = None),
            Box::new(|candidate| {
                candidate.contents.descriptor.link = Some(ElfDynamicSectionKind::DynamicSymbol)
            }),
            Box::new(|candidate| {
                candidate.contents.descriptor.info = Some(ElfDynamicSectionKind::DynamicString)
            }),
            Box::new(|candidate| {
                candidate.non_authoritative_descriptor_compatibility_fingerprint ^= 1
            }),
        ];
        for corrupt in corruptions {
            let mut candidate = candidate(TargetProfile::LinuxArm64);
            let expected_identity = candidate
                .payload
                .non_authoritative_payload_compatibility_fingerprint();
            corrupt(&mut candidate);
            let error = validate_candidate(candidate)
                .expect_err("corrupt dynamic-table descriptor candidate must reject");
            assert_eq!(
                error
                    .candidate
                    .payload
                    .non_authoritative_payload_compatibility_fingerprint(),
                expected_identity
            );
        }
    }

    #[test]
    fn malformed_name_offsets_and_arithmetic_reject_without_panicking() {
        assert!(checked_sum(usize::MAX, 1, "sum").is_err());
        assert!(checked_product(usize::MAX, 16, "product").is_err());
        assert!(checked_u32(usize::MAX, "word").is_err());
        let mut descriptor = ElfDynamicTableSectionDescriptor {
            kind: ElfDynamicTableSectionKind::DynamicTable,
            name_offset: u32::MAX,
            section_type: SHT_DYNAMIC,
            flags: SHF_WRITE | SHF_ALLOC,
            payload_size: 16,
            alignment: 8,
            entry_size: 16,
            link: Some(ElfDynamicSectionKind::DynamicString),
            info: None,
        };
        assert!(validate_name(b"\0.dynamic\0", &descriptor).is_err());
        descriptor.name_offset = 1;
        assert!(validate_name(b"\0.dynamic", &descriptor).is_err());
        descriptor.name_offset = 2;
        assert!(validate_name(b"\0.dynamic\0", &descriptor).is_err());
    }
}
