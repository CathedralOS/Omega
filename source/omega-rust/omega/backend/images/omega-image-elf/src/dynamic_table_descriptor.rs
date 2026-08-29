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

use crate::dynamic_section_descriptors::ElfDynamicSectionKind;
use crate::dynamic_tag_bytes::ValidatedElfDynamicTablePayload;
use psi_diagnostics::Diagnostic;

const SHT_DYNAMIC: u32 = 6;
const SHF_WRITE: u64 = 0x1;
const SHF_ALLOC: u64 = 0x2;
const ELF64_DYNAMIC_ALIGNMENT: u64 = 8;
const ELF64_DYNAMIC_ENTRY_SIZE: u64 = 16;
const UPSTREAM_DESCRIPTOR_COUNT: usize = 9;
const UPSTREAM_NAME_SEED_SIZE: usize = 93;
const DYNAMIC_NAME_OFFSET: u32 = 93;
const DYNAMIC_NAME_SUFFIX: &[u8] = b".dynamic\0";
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

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

    #[allow(dead_code)]
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

impl ElfDynamicTableSectionDescriptorPlanningError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfDynamicTablePayload, Diagnostic) {
        (self.payload, self.diagnostic)
    }
}

impl std::fmt::Display for ElfDynamicTableSectionDescriptorPlanningError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfDynamicTableSectionDescriptorPlanningError {}

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

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

/// Consume the exact serialized `.dynamic` payload into one address-free
/// semantic section descriptor and append its raw name to the owning seed.
///
/// This does not complete `.shstrtab`, assign final numeric section indexes or
/// addresses, resolve the semantic `.dynstr` link, place bytes, serialize
/// headers, or mutate an image.
pub fn plan_elf_dynamic_table_section_descriptor(
    payload: ValidatedElfDynamicTablePayload,
) -> Result<
    ValidatedElfDynamicTableSectionDescriptorPlan,
    Box<ElfDynamicTableSectionDescriptorPlanningError>,
> {
    let contents = match derive_contents(&payload) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(ElfDynamicTableSectionDescriptorPlanningError {
                payload,
                diagnostic,
            }));
        }
    };
    let non_authoritative_descriptor_compatibility_fingerprint =
        non_authoritative_descriptor_compatibility_fingerprint(&payload, &contents);
    let candidate = Candidate {
        payload,
        contents,
        non_authoritative_descriptor_compatibility_fingerprint,
    };
    match validate_candidate(candidate) {
        Ok(validated) => Ok(validated),
        Err(error) => Err(Box::new(ElfDynamicTableSectionDescriptorPlanningError {
            payload: error.candidate.payload,
            diagnostic: error.diagnostic,
        })),
    }
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
) -> Result<ValidatedElfDynamicTableSectionDescriptorPlan, CandidateValidationError> {
    if let Err(diagnostic) = validate_contents(&candidate.payload, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    if candidate.non_authoritative_descriptor_compatibility_fingerprint
        != non_authoritative_descriptor_compatibility_fingerprint(
            &candidate.payload,
            &candidate.contents,
        )
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error(
                "ELF dynamic-table descriptor compatibility fingerprint does not replay",
            ),
        });
    }
    Ok(ValidatedElfDynamicTableSectionDescriptorPlan {
        payload: candidate.payload,
        contents: candidate.contents,
        non_authoritative_descriptor_compatibility_fingerprint: candidate
            .non_authoritative_descriptor_compatibility_fingerprint,
    })
}

fn validate_contents(
    payload: &ValidatedElfDynamicTablePayload,
    contents: &ElfDynamicTableSectionDescriptorContents,
) -> Result<(), Diagnostic> {
    let descriptors = payload.plan().descriptors();
    require(
        descriptors.descriptor_count() == UPSTREAM_DESCRIPTOR_COUNT,
        "dynamic-table descriptor requires the exact sealed nine-row base",
    )?;
    let base_seed = base_name_seed(payload);
    require(
        base_seed.len() == UPSTREAM_NAME_SEED_SIZE,
        "dynamic-table descriptor requires the exact 93-byte upstream name seed",
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

fn checked_sum(left: usize, right: usize, context: &'static str) -> Result<usize, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
}

fn checked_product(left: usize, right: usize, context: &'static str) -> Result<usize, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
}

fn checked_u32(value: usize, context: &'static str) -> Result<u32, Diagnostic> {
    u32::try_from(value).map_err(|_| Diagnostic::error(format!("{context} exceeds Elf64_Word")))
}

fn require(condition: bool, message: &'static str) -> Result<(), Diagnostic> {
    condition
        .then_some(())
        .ok_or_else(|| Diagnostic::error(message))
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
    match row.link {
        Some(kind) => {
            hash.byte(1);
            hash.byte(kind as u8);
        }
        None => hash.byte(0),
    }
    match row.info {
        Some(kind) => {
            hash.byte(1);
            hash.byte(kind as u8);
        }
        None => hash.byte(0),
    }
    hash.finish()
}

struct Fnv1a(u64);

impl Fnv1a {
    const fn new() -> Self {
        Self(FNV_OFFSET_BASIS)
    }

    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(FNV_PRIME);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            self.byte(byte);
        }
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        plan_elf_dynamic_link_inputs, plan_elf_dynamic_section_descriptors,
        plan_elf_dynamic_sections, plan_elf_dynamic_tags, plan_elf_procedure_linkage_relocations,
        plan_elf_procedure_linkage_section_descriptors, plan_elf_procedure_linkage_templates,
        serialize_elf_dynamic_sections, serialize_elf_dynamic_table,
    };
    use omega_image::{
        FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
        FinalImageSection, FinalImageSymbol,
    };
    use omega_object_file::{RelocationKind, SymbolKind};
    use omega_target::{
        ForeignLocatorCandidate, TargetProfile, normalize_elf_interpreter_plan,
        normalize_foreign_locator,
    };
    use psi_arena::Handle;

    #[derive(Clone, Copy)]
    struct ImportFixture {
        object: &'static [u8],
        symbol: &'static [u8],
        version: &'static [u8],
        instruction_offsets: &'static [usize],
    }

    const FIRST_SITES: &[usize] = &[0, 32];
    const SECOND_SITES: &[usize] = &[16];
    const IMPORTS: [ImportFixture; 2] = [
        ImportFixture {
            object: b"liba\xff.so",
            symbol: b"alpha\xfe",
            version: b"V1\xfd",
            instruction_offsets: FIRST_SITES,
        },
        ImportFixture {
            object: b"libb.so",
            symbol: b"beta",
            version: b"V2",
            instruction_offsets: SECOND_SITES,
        },
    ];

    fn interpreter_path(target: TargetProfile) -> &'static [u8] {
        match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2",
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1",
            _ => unreachable!("dynamic descriptor fixture uses a Linux target"),
        }
    }

    fn payload(
        target: TargetProfile,
        imports: &[ImportFixture],
    ) -> ValidatedElfDynamicTablePayload {
        let relocation_count = imports
            .iter()
            .map(|fixture| fixture.instruction_offsets.len())
            .sum();
        let mut image = FinalImage::with_capacity(
            target.native_target(),
            FinalImageMemory {
                text: vec![0; 64],
                ..FinalImageMemory::default()
            },
            Handle::invalid(),
            imports.len(),
            imports.len(),
            relocation_count,
        );
        for (index, fixture) in imports.iter().enumerate() {
            let symbol_handle = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: format!("__omega_dynamic_descriptor_import_{index}"),
                section: FinalImageSection::None,
                offset: 0,
                size: 0,
                kind: SymbolKind::Import,
            });
            image.symbol_table.imports.insert(FinalImageImport {
                symbol_handle,
                import: FinalImageImportPlan::Normalized(
                    normalize_foreign_locator(
                        ForeignLocatorCandidate::ElfVersioned {
                            object: fixture.object.to_vec(),
                            symbol: fixture.symbol.to_vec(),
                            version: fixture.version.to_vec(),
                        },
                        target,
                    )
                    .expect("valid dynamic-descriptor locator"),
                ),
            });
            for instruction_offset in fixture.instruction_offsets {
                let (relocation_offset, kind) = match target {
                    TargetProfile::LinuxX64 => {
                        image.memory.text[*instruction_offset] = 0xe8;
                        (instruction_offset + 1, RelocationKind::X86_64Relative32)
                    }
                    TargetProfile::LinuxArm64 => {
                        image.memory.text[*instruction_offset..*instruction_offset + 4]
                            .copy_from_slice(&[0, 0, 0, 0x94]);
                        (*instruction_offset, RelocationKind::Aarch64Branch26)
                    }
                    _ => unreachable!(),
                };
                image
                    .relocation_table
                    .relocations
                    .insert(FinalImageRelocation {
                        section: FinalImageSection::Text,
                        offset: relocation_offset,
                        byte_width: 4,
                        symbol_handle,
                        addend: 0,
                        kind,
                    });
            }
        }
        let interpreter = normalize_elf_interpreter_plan(interpreter_path(target).to_vec(), target)
            .expect("valid dynamic-descriptor interpreter");
        let inputs = plan_elf_dynamic_link_inputs(image, interpreter).expect("valid link inputs");
        let sections = plan_elf_dynamic_sections(inputs).expect("valid dynamic sections");
        let payloads = serialize_elf_dynamic_sections(sections).expect("valid payloads");
        let descriptors =
            plan_elf_dynamic_section_descriptors(payloads).expect("valid base descriptors");
        let linkage =
            plan_elf_procedure_linkage_relocations(descriptors).expect("valid linkage plan");
        let templates =
            plan_elf_procedure_linkage_templates(linkage).expect("valid linkage templates");
        let descriptors = plan_elf_procedure_linkage_section_descriptors(templates)
            .expect("valid linkage descriptors");
        let tags = plan_elf_dynamic_tags(descriptors).expect("valid dynamic tags");
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
            assert_eq!(plan.payload.plan().descriptors().descriptor_count(), 9);
            assert_eq!(plan.descriptor_count(), 10);
            assert_eq!(plan.appended_descriptor_count(), 1);
            assert_eq!(plan.section_name_seed_byte_count(), 102);
            assert_eq!(
                &plan.contents.section_name_table_seed[93..],
                DYNAMIC_NAME_SUFFIX,
            );
            assert_eq!(
                plan.contents.descriptor,
                ElfDynamicTableSectionDescriptor {
                    kind: ElfDynamicTableSectionKind::DynamicTable,
                    name_offset: 93,
                    section_type: SHT_DYNAMIC,
                    flags: SHF_WRITE | SHF_ALLOC,
                    payload_size: 240,
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
    fn name_seed_is_exactly_append_only_and_preserves_raw_non_utf8_inputs() {
        let plan =
            plan_elf_dynamic_table_section_descriptor(payload(TargetProfile::LinuxX64, &IMPORTS))
                .unwrap();
        let base = base_name_seed(plan.payload());
        assert_eq!(&plan.contents.section_name_table_seed[..93], base);
        assert_eq!(
            &plan.contents.section_name_table_seed[59..69],
            b".shstrtab\0"
        );
        assert_eq!(
            &plan.contents.section_name_table_seed[83..93],
            b".rela.plt\0"
        );
        validate_name(
            &plan.contents.section_name_table_seed,
            &plan.contents.descriptor,
        )
        .unwrap();

        let dynstr = &plan
            .payload
            .plan()
            .descriptors()
            .templates()
            .linkage()
            .descriptors()
            .payloads()
            .payloads()
            .dynstr;
        assert!(dynstr.windows(8).any(|window| window == b"liba\xff.so"));
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
    fn every_appended_name_byte_replays_and_rejection_retains_payload() {
        for offset in UPSTREAM_NAME_SEED_SIZE..UPSTREAM_NAME_SEED_SIZE + DYNAMIC_NAME_SUFFIX.len() {
            let mut candidate = candidate(TargetProfile::LinuxX64);
            let expected_identity = candidate
                .payload
                .non_authoritative_payload_compatibility_fingerprint();
            candidate.contents.section_name_table_seed[offset] ^= 1;
            let error = validate_candidate(candidate)
                .expect_err("mutated dynamic-table name seed must reject");
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
    fn independent_replay_rejects_every_descriptor_field_link_info_and_identity_corruption() {
        let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
            Box::new(|candidate| candidate.contents.section_name_table_seed[0] ^= 1),
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
