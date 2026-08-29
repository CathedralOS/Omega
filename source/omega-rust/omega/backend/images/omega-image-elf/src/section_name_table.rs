//! Completed ELF section-name table contents and address-free descriptor.
//!
//! The generic System V ABI defines the [section-name string table] as the
//! string-table section selected later by `e_shstrndx`, and defines
//! `SHT_STRTAB` metadata in the [section header]. This layer adopts the exact
//! reserved name seed as the complete payload and adds its semantic
//! descriptor. It assigns no numeric section index, `e_shstrndx`, address,
//! placement, or serialized section header.
//!
//! [section-name string table]: https://gabi.xinuos.com/elf/03-sheader.html#special-sections
//! [section header]: https://gabi.xinuos.com/elf/03-sheader.html#section-header

use crate::dynamic_section_descriptors::ElfDynamicSectionKind;
use crate::dynamic_table_descriptor::ValidatedElfDynamicTableSectionDescriptorPlan;
use psi_diagnostics::Diagnostic;

const SHT_STRTAB: u32 = 3;
const UPSTREAM_DESCRIPTOR_COUNT: usize = 10;
const UPSTREAM_NAME_SEED_SIZE: usize = 102;
const SECTION_NAME_TABLE_NAME_OFFSET: u32 = 59;
const COMPLETE_SECTION_NAME_TABLE_SIZE: usize = 102;
const COMPLETE_SECTION_NAME_TABLE: &[u8] = b"\0.interp\0.dynstr\0.dynsym\0.hash\0.gnu.version\0.gnu.version_r\0.shstrtab\0.plt\0.got.plt\0.rela.plt\0.dynamic\0";
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Independently replayed complete section-name table and its address-free
/// semantic descriptor.
///
/// This non-clone carrier retains the exact `.dynamic` descriptor owner. It
/// grants no numeric `sh_name`, `sh_link`, section index, `e_shstrndx`,
/// address, placement, image mutation, publication, or runnable-image
/// authority.
#[derive(Debug)]
#[must_use = "validated ELF section-name table retains the dynamic descriptor owner"]
pub struct ValidatedElfSectionNameTablePlan {
    dynamic_table: ValidatedElfDynamicTableSectionDescriptorPlan,
    contents: ElfSectionNameTableContents,
    non_authoritative_table_compatibility_fingerprint: u64,
}

impl ValidatedElfSectionNameTablePlan {
    pub const fn dynamic_table(&self) -> &ValidatedElfDynamicTableSectionDescriptorPlan {
        &self.dynamic_table
    }

    pub const fn descriptor_count(&self) -> usize {
        UPSTREAM_DESCRIPTOR_COUNT + 1
    }

    pub const fn appended_descriptor_count(&self) -> usize {
        1
    }

    pub fn byte_count(&self) -> usize {
        self.contents.bytes.len()
    }

    /// Compatibility fingerprint of the exact upstream descriptor identity,
    /// complete raw name-table bytes, and semantic descriptor metadata. This
    /// is content identity, not section numbering or image authority.
    pub const fn non_authoritative_table_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_table_compatibility_fingerprint
    }

    pub(crate) const fn contents(&self) -> &ElfSectionNameTableContents {
        &self.contents
    }

    #[allow(dead_code)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidatedElfDynamicTableSectionDescriptorPlan,
        ElfSectionNameTableContents,
    ) {
        (self.dynamic_table, self.contents)
    }
}

/// Rejected section-name-table completion with exact `.dynamic` descriptor
/// custody.
#[derive(Debug)]
#[must_use = "ELF section-name-table rejection retains the dynamic descriptor owner"]
pub struct ElfSectionNameTablePlanningError {
    dynamic_table: ValidatedElfDynamicTableSectionDescriptorPlan,
    diagnostic: Diagnostic,
}

impl ElfSectionNameTablePlanningError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfDynamicTableSectionDescriptorPlan, Diagnostic) {
        (self.dynamic_table, self.diagnostic)
    }
}

impl std::fmt::Display for ElfSectionNameTablePlanningError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfSectionNameTablePlanningError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfSectionNameTableContents {
    pub(crate) bytes: Vec<u8>,
    pub(crate) descriptor: ElfSectionNameTableDescriptor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ElfSectionNameTableSectionKind {
    SectionNameTable = 1,
}

impl ElfSectionNameTableSectionKind {
    const fn name(self) -> &'static [u8] {
        match self {
            Self::SectionNameTable => b".shstrtab",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfSectionNameTableDescriptor {
    pub(crate) kind: ElfSectionNameTableSectionKind,
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
    dynamic_table: ValidatedElfDynamicTableSectionDescriptorPlan,
    contents: ElfSectionNameTableContents,
    non_authoritative_table_compatibility_fingerprint: u64,
}

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

/// Consume the exact `.dynamic` descriptor carrier, adopt its already reserved
/// `.shstrtab` name seed as the complete section-name-table payload, and seal
/// one address-free semantic string-table descriptor.
///
/// This does not assign final numeric section indexes, resolve any semantic
/// link, choose `e_shstrndx`, serialize section headers, place bytes, or mutate
/// an image.
pub fn plan_elf_section_name_table(
    dynamic_table: ValidatedElfDynamicTableSectionDescriptorPlan,
) -> Result<ValidatedElfSectionNameTablePlan, Box<ElfSectionNameTablePlanningError>> {
    let contents = match derive_contents(&dynamic_table) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(ElfSectionNameTablePlanningError {
                dynamic_table,
                diagnostic,
            }));
        }
    };
    let non_authoritative_table_compatibility_fingerprint =
        non_authoritative_table_compatibility_fingerprint(&dynamic_table, &contents);
    let candidate = Candidate {
        dynamic_table,
        contents,
        non_authoritative_table_compatibility_fingerprint,
    };
    match validate_candidate(candidate) {
        Ok(validated) => Ok(validated),
        Err(error) => Err(Box::new(ElfSectionNameTablePlanningError {
            dynamic_table: error.candidate.dynamic_table,
            diagnostic: error.diagnostic,
        })),
    }
}

fn derive_contents(
    dynamic_table: &ValidatedElfDynamicTableSectionDescriptorPlan,
) -> Result<ElfSectionNameTableContents, Diagnostic> {
    let base_seed = upstream_name_seed(dynamic_table);
    let name_offset = checked_u32(
        SECTION_NAME_TABLE_NAME_OFFSET as usize,
        "section-name-table name offset",
    )?;
    let bytes = base_seed.to_vec();
    let payload_size = u64::try_from(bytes.len())
        .map_err(|_| Diagnostic::error("section-name-table payload exceeds Elf64_Xword"))?;
    Ok(ElfSectionNameTableContents {
        bytes,
        descriptor: ElfSectionNameTableDescriptor {
            kind: ElfSectionNameTableSectionKind::SectionNameTable,
            name_offset,
            section_type: SHT_STRTAB,
            flags: 0,
            payload_size,
            alignment: 1,
            entry_size: 0,
            link: None,
            info: None,
        },
    })
}

fn upstream_name_seed(dynamic_table: &ValidatedElfDynamicTableSectionDescriptorPlan) -> &[u8] {
    &dynamic_table.contents().section_name_table_seed
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfSectionNameTablePlan, CandidateValidationError> {
    if let Err(diagnostic) = validate_contents(&candidate.dynamic_table, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    if candidate.non_authoritative_table_compatibility_fingerprint
        != non_authoritative_table_compatibility_fingerprint(
            &candidate.dynamic_table,
            &candidate.contents,
        )
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error(
                "ELF section-name-table compatibility fingerprint does not replay",
            ),
        });
    }
    Ok(ValidatedElfSectionNameTablePlan {
        dynamic_table: candidate.dynamic_table,
        contents: candidate.contents,
        non_authoritative_table_compatibility_fingerprint: candidate
            .non_authoritative_table_compatibility_fingerprint,
    })
}

fn validate_contents(
    dynamic_table: &ValidatedElfDynamicTableSectionDescriptorPlan,
    contents: &ElfSectionNameTableContents,
) -> Result<(), Diagnostic> {
    require(
        dynamic_table.descriptor_count() == UPSTREAM_DESCRIPTOR_COUNT,
        "section-name table requires the exact sealed ten-row base",
    )?;
    let base_seed = upstream_name_seed(dynamic_table);
    require(
        base_seed.len() == UPSTREAM_NAME_SEED_SIZE,
        "section-name table requires the exact 102-byte upstream name seed",
    )?;
    require(
        base_seed.len() == COMPLETE_SECTION_NAME_TABLE_SIZE
            && COMPLETE_SECTION_NAME_TABLE.len() == COMPLETE_SECTION_NAME_TABLE_SIZE,
        "section-name-table fixed size does not replay",
    )?;
    require(
        base_seed == COMPLETE_SECTION_NAME_TABLE
            && contents.bytes.len() == COMPLETE_SECTION_NAME_TABLE_SIZE
            && contents.bytes == base_seed
            && contents.bytes == COMPLETE_SECTION_NAME_TABLE,
        "section-name-table payload is not the exact bounded upstream seed",
    )?;
    validate_complete_names(&contents.bytes)?;

    let row = &contents.descriptor;
    validate_name(&contents.bytes, row)?;
    require(
        row.kind == ElfSectionNameTableSectionKind::SectionNameTable
            && row.name_offset == SECTION_NAME_TABLE_NAME_OFFSET
            && row.section_type == SHT_STRTAB
            && row.flags == 0
            && row.payload_size
                == u64::try_from(contents.bytes.len()).map_err(|_| {
                    Diagnostic::error("validated section-name-table size exceeds Elf64_Xword")
                })?
            && row.payload_size == COMPLETE_SECTION_NAME_TABLE_SIZE as u64
            && row.alignment == 1
            && row.entry_size == 0
            && row.link.is_none()
            && row.info.is_none(),
        "section-name-table descriptor metadata drifted from its exact payload",
    )
}

fn validate_complete_names(bytes: &[u8]) -> Result<(), Diagnostic> {
    const EXPECTED: &[(usize, &[u8])] = &[
        (0, b""),
        (1, b".interp"),
        (9, b".dynstr"),
        (17, b".dynsym"),
        (25, b".hash"),
        (31, b".gnu.version"),
        (44, b".gnu.version_r"),
        (59, b".shstrtab"),
        (69, b".plt"),
        (74, b".got.plt"),
        (83, b".rela.plt"),
        (93, b".dynamic"),
    ];
    let mut next_offset = 0usize;
    for (offset, expected) in EXPECTED {
        require(
            *offset == next_offset,
            "section-name-table expected offsets are not contiguous",
        )?;
        let tail = bytes
            .get(*offset..)
            .ok_or_else(|| Diagnostic::error("section name offset is outside the table"))?;
        let terminator = tail
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(|| Diagnostic::error("section name is not NUL-terminated"))?;
        require(
            tail.get(..terminator) == Some(*expected),
            "section-name-table entry does not replay",
        )?;
        next_offset = checked_sum(*offset, terminator + 1, "section-name-table traversal")?;
    }
    require(
        next_offset == bytes.len(),
        "section-name-table traversal leaves trailing bytes",
    )
}

fn validate_name(bytes: &[u8], row: &ElfSectionNameTableDescriptor) -> Result<(), Diagnostic> {
    let offset = usize::try_from(row.name_offset)
        .map_err(|_| Diagnostic::error("section-name-table sh_name exceeds usize"))?;
    let tail = bytes
        .get(offset..)
        .ok_or_else(|| Diagnostic::error("section-name-table sh_name is outside its payload"))?;
    let terminator = tail.iter().position(|byte| *byte == 0).ok_or_else(|| {
        Diagnostic::error("section-name-table descriptor name is not NUL-terminated")
    })?;
    require(
        tail.get(..terminator) == Some(row.kind.name()),
        "section-name-table sh_name does not select the reserved semantic name",
    )
}

fn checked_sum(left: usize, right: usize, context: &'static str) -> Result<usize, Diagnostic> {
    left.checked_add(right)
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

fn non_authoritative_table_compatibility_fingerprint(
    dynamic_table: &ValidatedElfDynamicTableSectionDescriptorPlan,
    contents: &ElfSectionNameTableContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-section-name-table.v1");
    hash.bytes(
        &dynamic_table
            .non_authoritative_descriptor_compatibility_fingerprint()
            .to_le_bytes(),
    );
    hash.bytes(&contents.bytes);
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
        plan_elf_dynamic_sections, plan_elf_dynamic_table_section_descriptor,
        plan_elf_dynamic_tags, plan_elf_procedure_linkage_relocations,
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
            _ => unreachable!("section-name fixture uses a Linux target"),
        }
    }

    fn dynamic_table(
        target: TargetProfile,
        imports: &[ImportFixture],
    ) -> ValidatedElfDynamicTableSectionDescriptorPlan {
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
                name: format!("__omega_section_name_import_{index}"),
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
                    .expect("valid section-name locator"),
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
            .expect("valid section-name interpreter");
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
        let payload = serialize_elf_dynamic_table(tags).expect("valid dynamic payload");
        plan_elf_dynamic_table_section_descriptor(payload).expect("valid dynamic descriptor")
    }

    fn candidate(target: TargetProfile) -> Candidate {
        let dynamic_table = dynamic_table(target, &IMPORTS);
        let contents = derive_contents(&dynamic_table).expect("derived section-name table");
        let non_authoritative_table_compatibility_fingerprint =
            non_authoritative_table_compatibility_fingerprint(&dynamic_table, &contents);
        Candidate {
            dynamic_table,
            contents,
            non_authoritative_table_compatibility_fingerprint,
        }
    }

    #[test]
    fn both_targets_adopt_exact_payload_and_address_free_descriptor() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let plan = plan_elf_section_name_table(dynamic_table(target, &IMPORTS))
                .expect("validated section-name table");
            assert_eq!(plan.dynamic_table().descriptor_count(), 10);
            assert_eq!(plan.descriptor_count(), 11);
            assert_eq!(plan.appended_descriptor_count(), 1);
            assert_eq!(plan.byte_count(), 102);
            assert_eq!(plan.contents.bytes, COMPLETE_SECTION_NAME_TABLE);
            assert_eq!(
                plan.contents.descriptor,
                ElfSectionNameTableDescriptor {
                    kind: ElfSectionNameTableSectionKind::SectionNameTable,
                    name_offset: 59,
                    section_type: SHT_STRTAB,
                    flags: 0,
                    payload_size: 102,
                    alignment: 1,
                    entry_size: 0,
                    link: None,
                    info: None,
                },
            );
            assert_ne!(plan.non_authoritative_table_compatibility_fingerprint(), 0);
            validate_contents(plan.dynamic_table(), &plan.contents)
                .expect("independent section-name-table replay");
        }
    }

    #[test]
    fn complete_payload_preserves_single_reserved_shstrtab_and_raw_non_utf8_ancestry() {
        let plan =
            plan_elf_section_name_table(dynamic_table(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
        let upstream = upstream_name_seed(plan.dynamic_table());
        assert_eq!(plan.contents.bytes, upstream);
        assert_eq!(&plan.contents.bytes[59..69], b".shstrtab\0");
        assert_eq!(
            plan.contents
                .bytes
                .windows(b".shstrtab\0".len())
                .filter(|window| *window == b".shstrtab\0")
                .count(),
            1,
        );
        assert_eq!(plan.contents.bytes.last(), Some(&0));
        assert_eq!(&plan.contents.bytes[93..], b".dynamic\0");
        validate_complete_names(&plan.contents.bytes).unwrap();

        let dynstr = &plan
            .dynamic_table
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
    }

    #[test]
    fn import_permutation_preserves_table_identity_and_target_remains_bound() {
        let forward =
            plan_elf_section_name_table(dynamic_table(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
        let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
        let reverse =
            plan_elf_section_name_table(dynamic_table(TargetProfile::LinuxX64, &reverse_imports))
                .unwrap();
        let arm = plan_elf_section_name_table(dynamic_table(TargetProfile::LinuxArm64, &IMPORTS))
            .unwrap();
        assert_eq!(forward.contents, reverse.contents);
        assert_eq!(
            forward.non_authoritative_table_compatibility_fingerprint(),
            reverse.non_authoritative_table_compatibility_fingerprint()
        );
        assert_ne!(
            forward.non_authoritative_table_compatibility_fingerprint(),
            arm.non_authoritative_table_compatibility_fingerprint()
        );
    }

    #[test]
    fn every_payload_byte_replays_and_rejection_retains_dynamic_descriptor() {
        for offset in 0..COMPLETE_SECTION_NAME_TABLE_SIZE {
            let mut candidate = candidate(TargetProfile::LinuxX64);
            let expected_identity = candidate
                .dynamic_table
                .non_authoritative_descriptor_compatibility_fingerprint();
            candidate.contents.bytes[offset] ^= 1;
            let error = validate_candidate(candidate)
                .expect_err("mutated section-name-table byte must reject");
            assert_eq!(
                error
                    .candidate
                    .dynamic_table
                    .non_authoritative_descriptor_compatibility_fingerprint(),
                expected_identity,
            );
        }
    }

    #[test]
    fn independent_replay_rejects_length_metadata_link_info_and_identity_corruption() {
        let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
            Box::new(|candidate| {
                candidate.contents.bytes.pop();
            }),
            Box::new(|candidate| candidate.contents.bytes.push(0)),
            Box::new(|candidate| candidate.contents.descriptor.name_offset = u32::MAX),
            Box::new(|candidate| candidate.contents.descriptor.section_type = 1),
            Box::new(|candidate| candidate.contents.descriptor.flags = 1),
            Box::new(|candidate| candidate.contents.descriptor.payload_size += 1),
            Box::new(|candidate| candidate.contents.descriptor.alignment = 8),
            Box::new(|candidate| candidate.contents.descriptor.entry_size = 1),
            Box::new(|candidate| {
                candidate.contents.descriptor.link = Some(ElfDynamicSectionKind::DynamicString)
            }),
            Box::new(|candidate| {
                candidate.contents.descriptor.info = Some(ElfDynamicSectionKind::DynamicSymbol)
            }),
            Box::new(|candidate| candidate.non_authoritative_table_compatibility_fingerprint ^= 1),
        ];
        for corrupt in corruptions {
            let mut candidate = candidate(TargetProfile::LinuxArm64);
            let expected_identity = candidate
                .dynamic_table
                .non_authoritative_descriptor_compatibility_fingerprint();
            corrupt(&mut candidate);
            let error = validate_candidate(candidate)
                .expect_err("corrupt section-name-table candidate must reject");
            assert_eq!(
                error
                    .candidate
                    .dynamic_table
                    .non_authoritative_descriptor_compatibility_fingerprint(),
                expected_identity,
            );
        }
    }

    #[test]
    fn bounded_name_traversal_and_arithmetic_reject_without_panicking() {
        assert!(checked_sum(usize::MAX, 1, "sum").is_err());
        assert!(checked_u32(usize::MAX, "word").is_err());
        assert!(validate_complete_names(&[]).is_err());
        assert!(validate_complete_names(&COMPLETE_SECTION_NAME_TABLE[..101]).is_err());
        let mut trailing = COMPLETE_SECTION_NAME_TABLE.to_vec();
        trailing.push(0);
        assert!(validate_complete_names(&trailing).is_err());

        let mut descriptor = ElfSectionNameTableDescriptor {
            kind: ElfSectionNameTableSectionKind::SectionNameTable,
            name_offset: u32::MAX,
            section_type: SHT_STRTAB,
            flags: 0,
            payload_size: 102,
            alignment: 1,
            entry_size: 0,
            link: None,
            info: None,
        };
        assert!(validate_name(b"\0.shstrtab\0", &descriptor).is_err());
        descriptor.name_offset = 1;
        assert!(validate_name(b"\0.shstrtab", &descriptor).is_err());
        descriptor.name_offset = 2;
        assert!(validate_name(b"\0.shstrtab\0", &descriptor).is_err());
    }
}
