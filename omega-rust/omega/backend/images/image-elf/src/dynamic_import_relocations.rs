//! Address-free procedure-linkage relocation requirements for dynamic imports.
//!
//! The System V ABI defines the dynamic [`DT_JMPREL`], [`DT_PLTRELSZ`], and
//! [`DT_PLTREL`] relationship. The target relocation numbers and procedure
//! linkage details come from the [x86-64 psABI] and [AArch64 ELF ABI]. This
//! module stops at semantic PLT/GOT slots and RELA `JUMP_SLOT` requirements;
//! it does not create addresses, final section indexes, GOT/PLT bytes,
//! `Elf64_Rela` bytes, placement, or mutation authority.
//!
//! [`DT_JMPREL`]: https://gabi.xinuos.com/elf/08-dynamic.html#dynamic-section
//! [`DT_PLTRELSZ`]: https://gabi.xinuos.com/elf/08-dynamic.html#dynamic-section
//! [`DT_PLTREL`]: https://gabi.xinuos.com/elf/08-dynamic.html#dynamic-section
//! [x86-64 psABI]: https://gitlab.com/x86-psABIs/x86-64-ABI
//! [AArch64 ELF ABI]: https://github.com/ARM-software/abi-aa/blob/main/aaelf64/aaelf64.rst

use crate::dynamic_section_descriptors::ValidatedElfDynamicSectionDescriptorPlan;
use crate::imports::{ElfImportLocator, ElfImportRequest};
use diagnostics::Diagnostic;
use image::{FinalImageRelocation, FinalImageSection};
use object_file::RelocationKind;
use target::TargetProfile;

const R_X86_64_JUMP_SLOT: u32 = 7;
const R_AARCH64_JUMP_SLOT: u32 = 1026;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Independently validated semantic procedure-linkage relocations for all
/// referenced dynamic imports.
///
/// Each logical ordinal names one future PLT entry and its corresponding GOT
/// slot without claiming either section's physical numbering or placement.
/// All unresolved import uses are proven to be direct calls represented by
/// these rows, so this carrier requires no general `.rela.dyn` entries.
#[derive(Debug)]
#[must_use = "validated procedure linkage retains the exact descriptor plan"]
pub struct ValidatedElfProcedureLinkageRelocationPlan {
    descriptors: ValidatedElfDynamicSectionDescriptorPlan,
    contents: ElfProcedureLinkageRelocationContents,
    non_authoritative_linkage_compatibility_fingerprint: u64,
}

impl ValidatedElfProcedureLinkageRelocationPlan {
    pub const fn descriptors(&self) -> &ValidatedElfDynamicSectionDescriptorPlan {
        &self.descriptors
    }

    pub fn logical_slot_count(&self) -> usize {
        self.contents.slots.len()
    }

    pub fn procedure_relocation_count(&self) -> usize {
        self.contents.jump_slot_relocations.len()
    }

    pub fn direct_call_site_count(&self) -> usize {
        self.contents
            .slots
            .iter()
            .map(|slot| slot.call_sites.len())
            .sum()
    }

    /// All unresolved import relocations were admitted as procedure calls, so
    /// no general dynamic relocation row is required by this plan.
    pub const fn general_dynamic_relocation_count(&self) -> usize {
        0
    }

    /// Compatibility fingerprint of the exact descriptor identity, target,
    /// logical PLT/GOT slots, semantic JUMP_SLOT rows, and canonical call-site
    /// mapping. This is not an address, layout, or runnable-image identity.
    pub const fn non_authoritative_linkage_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_linkage_compatibility_fingerprint
    }

    pub(crate) const fn contents(&self) -> &ElfProcedureLinkageRelocationContents {
        &self.contents
    }

    #[allow(dead_code)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidatedElfDynamicSectionDescriptorPlan,
        ElfProcedureLinkageRelocationContents,
    ) {
        (self.descriptors, self.contents)
    }
}

/// Rejected procedure-linkage planning with exact descriptor custody.
#[derive(Debug)]
#[must_use = "ELF procedure-linkage rejection retains the descriptor plan"]
pub struct ElfProcedureLinkageRelocationPlanningError {
    descriptors: ValidatedElfDynamicSectionDescriptorPlan,
    diagnostic: Diagnostic,
}

impl ElfProcedureLinkageRelocationPlanningError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfDynamicSectionDescriptorPlan, Diagnostic) {
        (self.descriptors, self.diagnostic)
    }
}

impl std::fmt::Display for ElfProcedureLinkageRelocationPlanningError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfProcedureLinkageRelocationPlanningError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfProcedureLinkageRelocationContents {
    pub(crate) slots: Vec<ElfLogicalProcedureLinkageSlot>,
    pub(crate) jump_slot_relocations: Vec<ElfSemanticJumpSlotRelocation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfLogicalProcedureLinkageSlot {
    pub(crate) logical_ordinal: u32,
    pub(crate) request_index: usize,
    pub(crate) compatibility_report_identity: u64,
    pub(crate) dynamic_symbol_index: u32,
    pub(crate) version_index: u16,
    pub(crate) call_sites: Vec<ElfDirectImportCallSite>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfDirectImportCallSite {
    pub(crate) instruction_offset: usize,
    pub(crate) relocation_offset: usize,
    pub(crate) byte_width: usize,
    pub(crate) kind: RelocationKind,
    pub(crate) addend: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfSemanticJumpSlotRelocation {
    pub(crate) logical_got_slot_ordinal: u32,
    pub(crate) dynamic_symbol_index: u32,
    pub(crate) relocation_type: u32,
    pub(crate) addend: i64,
}

struct Candidate {
    descriptors: ValidatedElfDynamicSectionDescriptorPlan,
    contents: ElfProcedureLinkageRelocationContents,
    non_authoritative_linkage_compatibility_fingerprint: u64,
}

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

/// Consume exact dynamic-section descriptors into one canonical, address-free
/// procedure-linkage relocation plan.
///
/// Every referenced import must be used only by the target's exact direct-call
/// relocation shape. Success creates one logical PLT/GOT slot and one semantic
/// RELA `JUMP_SLOT` requirement per imported dynamic symbol. It deliberately
/// emits no bytes and assigns no address or final section index.
pub fn plan_elf_procedure_linkage_relocations(
    descriptors: ValidatedElfDynamicSectionDescriptorPlan,
) -> Result<
    ValidatedElfProcedureLinkageRelocationPlan,
    Box<ElfProcedureLinkageRelocationPlanningError>,
> {
    let contents = match derive_contents(&descriptors) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(ElfProcedureLinkageRelocationPlanningError {
                descriptors,
                diagnostic,
            }));
        }
    };
    let non_authoritative_linkage_compatibility_fingerprint =
        non_authoritative_linkage_compatibility_fingerprint(&descriptors, &contents);
    let candidate = Candidate {
        descriptors,
        contents,
        non_authoritative_linkage_compatibility_fingerprint,
    };
    match validate_candidate(candidate) {
        Ok(validated) => Ok(validated),
        Err(error) => Err(Box::new(ElfProcedureLinkageRelocationPlanningError {
            descriptors: error.candidate.descriptors,
            diagnostic: error.diagnostic,
        })),
    }
}

fn derive_contents(
    descriptors: &ValidatedElfDynamicSectionDescriptorPlan,
) -> Result<ElfProcedureLinkageRelocationContents, Diagnostic> {
    let plan = descriptors.payloads().plan();
    let bindings = &plan.contents().bindings;
    let inputs = plan.inputs();
    require(
        bindings.len() == inputs.imports().len(),
        "ELF dynamic bindings do not cover every canonical import request",
    )?;
    let (source_kind, relocation_type) = target_relocation_spec(inputs.interpreter().target())?;
    let mut slots = Vec::with_capacity(bindings.len());
    let mut jump_slot_relocations = Vec::with_capacity(bindings.len());
    for (index, binding) in bindings.iter().enumerate() {
        let logical_ordinal = checked_u32(index, "logical procedure-linkage slot ordinal")?;
        let request = inputs.imports().get(binding.request_index).ok_or_else(|| {
            Diagnostic::error("ELF dynamic binding request index exceeds canonical imports")
        })?;
        let compatibility_report_identity = request_compatibility_report_identity(request)?;
        require(
            compatibility_report_identity == binding.compatibility_report_identity,
            "ELF dynamic binding report identity does not match its canonical import request",
        )?;
        let call_sites = canonical_call_sites(
            inputs.interpreter().target(),
            &inputs.image().memory.text,
            source_kind,
            request,
        )?;
        slots.push(ElfLogicalProcedureLinkageSlot {
            logical_ordinal,
            request_index: binding.request_index,
            compatibility_report_identity,
            dynamic_symbol_index: binding.dynamic_symbol_index,
            version_index: binding.version_index,
            call_sites,
        });
        jump_slot_relocations.push(ElfSemanticJumpSlotRelocation {
            logical_got_slot_ordinal: logical_ordinal,
            dynamic_symbol_index: binding.dynamic_symbol_index,
            relocation_type,
            addend: 0,
        });
    }
    let contents = ElfProcedureLinkageRelocationContents {
        slots,
        jump_slot_relocations,
    };
    validate_nonoverlapping_call_sites(&contents)?;
    Ok(contents)
}

fn target_relocation_spec(target: TargetProfile) -> Result<(RelocationKind, u32), Diagnostic> {
    match target {
        TargetProfile::LinuxX64 => Ok((RelocationKind::X86_64Relative32, R_X86_64_JUMP_SLOT)),
        TargetProfile::LinuxArm64 => Ok((RelocationKind::Aarch64Branch26, R_AARCH64_JUMP_SLOT)),
        _ => Err(Diagnostic::error(
            "ELF procedure linkage requires an exact Linux x86-64 or AArch64 profile",
        )),
    }
}

fn request_compatibility_report_identity(request: &ElfImportRequest) -> Result<u64, Diagnostic> {
    match &request.locator {
        ElfImportLocator::Versioned {
            compatibility_report_identity,
            ..
        } => Ok(*compatibility_report_identity),
        ElfImportLocator::StringBackedBootstrap { .. } => Err(Diagnostic::error(
            "string-backed import reached normalized ELF procedure linkage",
        )),
    }
}

fn canonical_call_sites(
    target: TargetProfile,
    text: &[u8],
    expected_kind: RelocationKind,
    request: &ElfImportRequest,
) -> Result<Vec<ElfDirectImportCallSite>, Diagnostic> {
    require(
        !request.relocations.is_empty(),
        "ELF procedure-linkage import has no retained direct call site",
    )?;
    let mut sites = request
        .relocations
        .iter()
        .map(|relocation| call_site(target, text, expected_kind, relocation))
        .collect::<Result<Vec<_>, _>>()?;
    sites.sort_unstable_by_key(|site| (site.instruction_offset, site.relocation_offset));
    validate_site_spans(&sites)?;
    Ok(sites)
}

fn call_site(
    target: TargetProfile,
    text: &[u8],
    expected_kind: RelocationKind,
    relocation: &FinalImageRelocation,
) -> Result<ElfDirectImportCallSite, Diagnostic> {
    require(
        relocation.section == FinalImageSection::Text
            && relocation.kind == expected_kind
            && relocation.byte_width == 4
            && relocation.addend == 0,
        "ELF dynamic import requires a non-procedure or malformed source relocation",
    )?;
    let end = relocation
        .offset
        .checked_add(relocation.byte_width)
        .filter(|end| *end <= text.len())
        .ok_or_else(|| Diagnostic::error("ELF dynamic import call relocation exceeds .text"))?;
    let instruction_offset = match target {
        TargetProfile::LinuxX64 => {
            let instruction_offset = relocation.offset.checked_sub(1).ok_or_else(|| {
                Diagnostic::error("x86-64 import call relocation lacks its opcode byte")
            })?;
            require(
                text.get(instruction_offset) == Some(&0xe8)
                    && text.get(relocation.offset..end) == Some(&[0, 0, 0, 0]),
                "x86-64 import call does not retain the exact unresolved CALL rel32 placeholder",
            )?;
            instruction_offset
        }
        TargetProfile::LinuxArm64 => {
            require(
                relocation.offset.is_multiple_of(4)
                    && text.get(relocation.offset..end) == Some(&[0, 0, 0, 0x94]),
                "AArch64 import call does not retain the exact unresolved BL placeholder",
            )?;
            relocation.offset
        }
        _ => {
            return Err(Diagnostic::error(
                "ELF import call uses a non-Linux target profile",
            ));
        }
    };
    Ok(ElfDirectImportCallSite {
        instruction_offset,
        relocation_offset: relocation.offset,
        byte_width: relocation.byte_width,
        kind: relocation.kind,
        addend: relocation.addend,
    })
}

fn validate_site_spans(sites: &[ElfDirectImportCallSite]) -> Result<(), Diagnostic> {
    for pair in sites.windows(2) {
        let left_end = site_end(&pair[0])?;
        require(
            left_end <= pair[1].instruction_offset,
            "ELF import call sites overlap or repeat one instruction span",
        )?;
    }
    Ok(())
}

fn validate_nonoverlapping_call_sites(
    contents: &ElfProcedureLinkageRelocationContents,
) -> Result<(), Diagnostic> {
    let mut spans = contents
        .slots
        .iter()
        .flat_map(|slot| &slot.call_sites)
        .map(|site| Ok((site.instruction_offset, site_end(site)?)))
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    spans.sort_unstable();
    for pair in spans.windows(2) {
        require(
            pair[0].1 <= pair[1].0,
            "ELF import call mapping contains overlapping instruction spans",
        )?;
    }
    Ok(())
}

fn site_end(site: &ElfDirectImportCallSite) -> Result<usize, Diagnostic> {
    site.relocation_offset
        .checked_add(site.byte_width)
        .ok_or_else(|| Diagnostic::error("ELF import call-site span overflows usize"))
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfProcedureLinkageRelocationPlan, CandidateValidationError> {
    if let Err(diagnostic) = validate_contents(&candidate.descriptors, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    if candidate.non_authoritative_linkage_compatibility_fingerprint
        != non_authoritative_linkage_compatibility_fingerprint(
            &candidate.descriptors,
            &candidate.contents,
        )
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error(
                "ELF procedure-linkage compatibility fingerprint does not replay",
            ),
        });
    }
    Ok(ValidatedElfProcedureLinkageRelocationPlan {
        descriptors: candidate.descriptors,
        contents: candidate.contents,
        non_authoritative_linkage_compatibility_fingerprint: candidate
            .non_authoritative_linkage_compatibility_fingerprint,
    })
}

fn validate_contents(
    descriptors: &ValidatedElfDynamicSectionDescriptorPlan,
    contents: &ElfProcedureLinkageRelocationContents,
) -> Result<(), Diagnostic> {
    let plan = descriptors.payloads().plan();
    let bindings = &plan.contents().bindings;
    let inputs = plan.inputs();
    require(
        contents.slots.len() == bindings.len()
            && contents.jump_slot_relocations.len() == bindings.len()
            && bindings.len() == inputs.imports().len(),
        "ELF procedure-linkage slots, relocations, bindings, and imports do not correspond",
    )?;
    let (source_kind, relocation_type) = target_relocation_spec(inputs.interpreter().target())?;
    for (index, binding) in bindings.iter().enumerate() {
        let logical_ordinal = checked_u32(index, "validated logical PLT/GOT slot ordinal")?;
        let request = inputs.imports().get(binding.request_index).ok_or_else(|| {
            Diagnostic::error("validated ELF binding exceeds canonical import requests")
        })?;
        let compatibility_report_identity = request_compatibility_report_identity(request)?;
        let expected_sites = canonical_call_sites(
            inputs.interpreter().target(),
            &inputs.image().memory.text,
            source_kind,
            request,
        )?;
        let expected_slot = ElfLogicalProcedureLinkageSlot {
            logical_ordinal,
            request_index: binding.request_index,
            compatibility_report_identity,
            dynamic_symbol_index: binding.dynamic_symbol_index,
            version_index: binding.version_index,
            call_sites: expected_sites,
        };
        let expected_relocation = ElfSemanticJumpSlotRelocation {
            logical_got_slot_ordinal: logical_ordinal,
            dynamic_symbol_index: binding.dynamic_symbol_index,
            relocation_type,
            addend: 0,
        };
        require(
            compatibility_report_identity == binding.compatibility_report_identity
                && contents.slots.get(index) == Some(&expected_slot)
                && contents.jump_slot_relocations.get(index) == Some(&expected_relocation),
            "ELF procedure-linkage slot or JUMP_SLOT row drifted from its exact import binding",
        )?;
    }
    require_unique_semantic_rows(contents)?;
    validate_nonoverlapping_call_sites(contents)
}

fn require_unique_semantic_rows(
    contents: &ElfProcedureLinkageRelocationContents,
) -> Result<(), Diagnostic> {
    for (index, slot) in contents.slots.iter().enumerate() {
        require(
            contents
                .slots
                .iter()
                .enumerate()
                .all(|(other_index, other)| {
                    other_index == index
                        || (other.logical_ordinal != slot.logical_ordinal
                            && other.compatibility_report_identity
                                != slot.compatibility_report_identity
                            && other.dynamic_symbol_index != slot.dynamic_symbol_index)
                }),
            "ELF procedure-linkage slots duplicate an ordinal, import identity, or dynamic symbol",
        )?;
    }
    Ok(())
}

fn checked_u32(value: usize, context: &'static str) -> Result<u32, Diagnostic> {
    u32::try_from(value).map_err(|_| Diagnostic::error(format!("{context} exceeds Elf64_Word")))
}

fn require(condition: bool, message: &'static str) -> Result<(), Diagnostic> {
    condition
        .then_some(())
        .ok_or_else(|| Diagnostic::error(message))
}

fn non_authoritative_linkage_compatibility_fingerprint(
    descriptors: &ValidatedElfDynamicSectionDescriptorPlan,
    contents: &ElfProcedureLinkageRelocationContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-procedure-linkage-relocations.v1");
    hash.bytes(
        &descriptors
            .non_authoritative_descriptor_compatibility_fingerprint()
            .to_le_bytes(),
    );
    hash.bytes(
        descriptors
            .payloads()
            .plan()
            .inputs()
            .interpreter()
            .target()
            .target_name()
            .as_bytes(),
    );
    hash.bytes(&(contents.slots.len() as u64).to_le_bytes());
    for slot in &contents.slots {
        hash.bytes(&slot.logical_ordinal.to_le_bytes());
        hash.bytes(&slot.compatibility_report_identity.to_le_bytes());
        hash.bytes(&slot.dynamic_symbol_index.to_le_bytes());
        hash.bytes(&slot.version_index.to_le_bytes());
        hash.bytes(&(slot.call_sites.len() as u64).to_le_bytes());
        for site in &slot.call_sites {
            hash.bytes(&(site.instruction_offset as u64).to_le_bytes());
            hash.bytes(&(site.relocation_offset as u64).to_le_bytes());
            hash.bytes(&(site.byte_width as u64).to_le_bytes());
            hash.byte(relocation_kind_tag(site.kind));
            hash.bytes(&site.addend.to_le_bytes());
        }
    }
    hash.bytes(&(contents.jump_slot_relocations.len() as u64).to_le_bytes());
    for relocation in &contents.jump_slot_relocations {
        hash.bytes(&relocation.logical_got_slot_ordinal.to_le_bytes());
        hash.bytes(&relocation.dynamic_symbol_index.to_le_bytes());
        hash.bytes(&relocation.relocation_type.to_le_bytes());
        hash.bytes(&relocation.addend.to_le_bytes());
    }
    hash.finish()
}

const fn relocation_kind_tag(kind: RelocationKind) -> u8 {
    match kind {
        RelocationKind::Aarch64Page21 => 1,
        RelocationKind::Aarch64PageOffset12 => 2,
        RelocationKind::Aarch64Branch26 => 3,
        RelocationKind::Absolute64 => 4,
        RelocationKind::X86_64Relative32 => 5,
    }
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
        plan_elf_dynamic_sections, serialize_elf_dynamic_sections,
    };
    use arena::Handle;
    use image::{
        FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
        FinalImageSymbol,
    };
    use object_file::SymbolKind;
    use target::{
        ForeignLocatorCandidate, normalize_elf_interpreter_plan, normalize_foreign_locator,
    };

    #[derive(Clone, Copy)]
    struct ImportFixture {
        object: &'static [u8],
        symbol: &'static [u8],
        version: &'static [u8],
        instruction_offsets: &'static [usize],
    }

    const FIRST_SITES: &[usize] = &[0, 44];
    const SECOND_SITES: &[usize] = &[16];
    const THIRD_SITES: &[usize] = &[28];
    const IMPORTS: [ImportFixture; 3] = [
        ImportFixture {
            object: b"liba\xff.so",
            symbol: b"alpha\xfe",
            version: b"V1\xfd",
            instruction_offsets: FIRST_SITES,
        },
        ImportFixture {
            object: b"liba\xff.so",
            symbol: b"beta",
            version: b"V2",
            instruction_offsets: SECOND_SITES,
        },
        ImportFixture {
            object: b"libb.so",
            symbol: b"gamma",
            version: b"V1\xfd",
            instruction_offsets: THIRD_SITES,
        },
    ];

    fn interpreter_path(target: TargetProfile) -> &'static [u8] {
        match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2",
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1",
            _ => unreachable!("procedure-linkage fixture uses a Linux target"),
        }
    }

    fn image(
        target: TargetProfile,
        imports: &[ImportFixture],
    ) -> (FinalImage, Vec<Handle<FinalImageRelocation>>) {
        let native_target = target.native_target();
        let relocation_count = imports
            .iter()
            .map(|fixture| fixture.instruction_offsets.len())
            .sum();
        let mut image = FinalImage::with_capacity(
            native_target,
            FinalImageMemory {
                text: vec![0; 64],
                ..FinalImageMemory::default()
            },
            Handle::invalid(),
            imports.len(),
            imports.len(),
            relocation_count,
        );
        let mut relocation_handles = Vec::with_capacity(relocation_count);
        for (index, fixture) in imports.iter().enumerate() {
            let symbol_handle = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: format!("__omega_procedure_import_{index}"),
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
                    .expect("valid procedure-linkage locator"),
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
                    _ => unreachable!("fixture uses a Linux target"),
                };
                relocation_handles.push(image.relocation_table.relocations.insert(
                    FinalImageRelocation {
                        section: FinalImageSection::Text,
                        offset: relocation_offset,
                        byte_width: 4,
                        symbol_handle,
                        addend: 0,
                        kind,
                    },
                ));
            }
        }
        (image, relocation_handles)
    }

    fn descriptors_from_image(
        target: TargetProfile,
        image: FinalImage,
    ) -> ValidatedElfDynamicSectionDescriptorPlan {
        let interpreter = normalize_elf_interpreter_plan(interpreter_path(target).to_vec(), target)
            .expect("valid procedure-linkage interpreter");
        let inputs =
            plan_elf_dynamic_link_inputs(image, interpreter).expect("valid dynamic-link preflight");
        let sections = plan_elf_dynamic_sections(inputs).expect("valid dynamic sections");
        let payloads = serialize_elf_dynamic_sections(sections).expect("valid dynamic payloads");
        plan_elf_dynamic_section_descriptors(payloads).expect("valid dynamic descriptors")
    }

    fn descriptors(
        target: TargetProfile,
        imports: &[ImportFixture],
    ) -> ValidatedElfDynamicSectionDescriptorPlan {
        descriptors_from_image(target, image(target, imports).0)
    }

    fn candidate(target: TargetProfile) -> Candidate {
        let descriptors = descriptors(target, &IMPORTS);
        let contents = derive_contents(&descriptors).expect("derived procedure linkage");
        let non_authoritative_linkage_compatibility_fingerprint =
            non_authoritative_linkage_compatibility_fingerprint(&descriptors, &contents);
        Candidate {
            descriptors,
            contents,
            non_authoritative_linkage_compatibility_fingerprint,
        }
    }

    #[test]
    fn both_linux_targets_plan_exact_slots_jump_relocations_and_call_sites() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let plan = plan_elf_procedure_linkage_relocations(descriptors(target, &IMPORTS))
                .expect("validated procedure-linkage plan");
            assert_eq!(plan.logical_slot_count(), 3);
            assert_eq!(plan.procedure_relocation_count(), 3);
            assert_eq!(plan.direct_call_site_count(), 4);
            assert_eq!(plan.general_dynamic_relocation_count(), 0);
            assert_ne!(
                plan.non_authoritative_linkage_compatibility_fingerprint(),
                0
            );

            let expected_source_kind = match target {
                TargetProfile::LinuxX64 => RelocationKind::X86_64Relative32,
                TargetProfile::LinuxArm64 => RelocationKind::Aarch64Branch26,
                _ => unreachable!(),
            };
            let expected_dynamic_type = match target {
                TargetProfile::LinuxX64 => R_X86_64_JUMP_SLOT,
                TargetProfile::LinuxArm64 => R_AARCH64_JUMP_SLOT,
                _ => unreachable!(),
            };
            assert_eq!(
                plan.contents
                    .slots
                    .iter()
                    .map(|slot| slot.logical_ordinal)
                    .collect::<Vec<_>>(),
                [0, 1, 2],
            );
            assert_eq!(
                plan.contents
                    .slots
                    .iter()
                    .map(|slot| slot.dynamic_symbol_index)
                    .collect::<Vec<_>>(),
                [1, 2, 3],
            );
            assert!(plan.contents.slots.iter().all(|slot| {
                !slot.call_sites.is_empty()
                    && slot.call_sites.iter().all(|site| {
                        site.kind == expected_source_kind
                            && site.byte_width == 4
                            && site.addend == 0
                    })
            }));
            assert!(plan.contents.jump_slot_relocations.iter().enumerate().all(
                |(index, relocation)| {
                    relocation.logical_got_slot_ordinal == index as u32
                        && relocation.dynamic_symbol_index == index as u32 + 1
                        && relocation.relocation_type == expected_dynamic_type
                        && relocation.addend == 0
                }
            ));
            validate_contents(plan.descriptors(), &plan.contents)
                .expect("independent procedure-linkage replay");
        }
    }

    #[test]
    fn import_permutation_preserves_identity_and_multiple_calls_share_one_slot() {
        let forward =
            plan_elf_procedure_linkage_relocations(descriptors(TargetProfile::LinuxX64, &IMPORTS))
                .expect("forward procedure linkage");
        let reversed = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
        let reverse =
            plan_elf_procedure_linkage_relocations(descriptors(TargetProfile::LinuxX64, &reversed))
                .expect("reverse procedure linkage");
        let arm = plan_elf_procedure_linkage_relocations(descriptors(
            TargetProfile::LinuxArm64,
            &IMPORTS,
        ))
        .expect("AArch64 procedure linkage");

        assert_eq!(
            forward.non_authoritative_linkage_compatibility_fingerprint(),
            reverse.non_authoritative_linkage_compatibility_fingerprint()
        );
        assert_ne!(
            forward.non_authoritative_linkage_compatibility_fingerprint(),
            arm.non_authoritative_linkage_compatibility_fingerprint()
        );
        assert_eq!(forward.logical_slot_count(), 3);
        assert_eq!(forward.direct_call_site_count(), 4);
        assert_eq!(
            forward
                .contents
                .slots
                .iter()
                .map(|slot| slot.call_sites.len())
                .collect::<Vec<_>>(),
            [2, 1, 1],
        );
    }

    #[test]
    fn nonprocedure_source_relocations_and_tampered_placeholders_reject_with_custody() {
        type Mutation = Box<dyn Fn(&mut FinalImage, &[Handle<FinalImageRelocation>])>;
        let mutations: Vec<Mutation> = vec![
            Box::new(|image, handles| {
                image
                    .relocation_table
                    .relocations
                    .get_mut(handles[0])
                    .section = FinalImageSection::Data;
            }),
            Box::new(|image, handles| {
                image.relocation_table.relocations.get_mut(handles[0]).kind =
                    RelocationKind::Absolute64;
            }),
            Box::new(|image, handles| {
                image
                    .relocation_table
                    .relocations
                    .get_mut(handles[0])
                    .byte_width = 8;
            }),
            Box::new(|image, handles| {
                image
                    .relocation_table
                    .relocations
                    .get_mut(handles[0])
                    .addend = 1;
            }),
            Box::new(|image, handles| {
                image
                    .relocation_table
                    .relocations
                    .get_mut(handles[0])
                    .offset = usize::MAX;
            }),
            Box::new(|image, _| image.memory.text[0] = 0x90),
            Box::new(|image, handles| {
                let first = image.relocation_table.relocations.get(handles[0]).offset;
                image
                    .relocation_table
                    .relocations
                    .get_mut(handles[2])
                    .offset = first;
            }),
        ];

        for mutate in mutations {
            let (mut image, handles) = image(TargetProfile::LinuxX64, &IMPORTS);
            mutate(&mut image, &handles);
            let descriptors = descriptors_from_image(TargetProfile::LinuxX64, image);
            let expected_identity =
                descriptors.non_authoritative_descriptor_compatibility_fingerprint();
            let error = plan_elf_procedure_linkage_relocations(descriptors)
                .expect_err("invalid imported call must reject before linkage sealing");
            let (returned, _) = error.into_parts();
            assert_eq!(
                returned.non_authoritative_descriptor_compatibility_fingerprint(),
                expected_identity
            );
        }

        let (mut arm_image, arm_handles) = image(TargetProfile::LinuxArm64, &IMPORTS);
        arm_image
            .relocation_table
            .relocations
            .get_mut(arm_handles[0])
            .offset = 1;
        let descriptors = descriptors_from_image(TargetProfile::LinuxArm64, arm_image);
        let expected_identity =
            descriptors.non_authoritative_descriptor_compatibility_fingerprint();
        let error = plan_elf_procedure_linkage_relocations(descriptors)
            .expect_err("misaligned AArch64 BL relocation must reject");
        assert_eq!(
            error
                .into_parts()
                .0
                .non_authoritative_descriptor_compatibility_fingerprint(),
            expected_identity
        );
    }

    #[test]
    fn independent_validation_rejects_every_slot_relocation_site_and_identity_corruption() {
        let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
            Box::new(|candidate| {
                candidate.contents.slots.pop();
            }),
            Box::new(|candidate| {
                candidate.contents.jump_slot_relocations.pop();
            }),
            Box::new(|candidate| candidate.contents.slots.swap(0, 1)),
            Box::new(|candidate| candidate.contents.slots[0].logical_ordinal += 1),
            Box::new(|candidate| candidate.contents.slots[0].request_index = usize::MAX),
            Box::new(|candidate| {
                candidate.contents.slots[0].compatibility_report_identity ^= 1;
            }),
            Box::new(|candidate| candidate.contents.slots[0].dynamic_symbol_index += 1),
            Box::new(|candidate| candidate.contents.slots[0].version_index += 1),
            Box::new(|candidate| {
                candidate.contents.slots[0].call_sites.pop();
            }),
            Box::new(|candidate| candidate.contents.slots[0].call_sites[0].instruction_offset += 1),
            Box::new(|candidate| candidate.contents.slots[0].call_sites[0].relocation_offset += 1),
            Box::new(|candidate| candidate.contents.slots[0].call_sites[0].byte_width += 1),
            Box::new(|candidate| {
                candidate.contents.slots[0].call_sites[0].kind = RelocationKind::Absolute64
            }),
            Box::new(|candidate| candidate.contents.slots[0].call_sites[0].addend += 1),
            Box::new(|candidate| {
                candidate.contents.jump_slot_relocations[0].logical_got_slot_ordinal += 1
            }),
            Box::new(|candidate| {
                candidate.contents.jump_slot_relocations[0].dynamic_symbol_index += 1
            }),
            Box::new(|candidate| candidate.contents.jump_slot_relocations[0].relocation_type += 1),
            Box::new(|candidate| candidate.contents.jump_slot_relocations[0].addend += 1),
            Box::new(|candidate| {
                candidate.non_authoritative_linkage_compatibility_fingerprint ^= 1
            }),
        ];

        for corrupt in corruptions {
            let mut candidate = candidate(TargetProfile::LinuxX64);
            let expected_identity = candidate
                .descriptors
                .non_authoritative_descriptor_compatibility_fingerprint();
            corrupt(&mut candidate);
            let error = validate_candidate(candidate)
                .expect_err("corrupt procedure-linkage candidate must reject");
            assert_eq!(
                error
                    .candidate
                    .descriptors
                    .non_authoritative_descriptor_compatibility_fingerprint(),
                expected_identity,
                "validation failure retains exact descriptor custody",
            );
        }
    }

    #[test]
    fn malformed_offsets_ordinals_and_spans_reject_without_panicking() {
        assert!(checked_u32(usize::MAX, "ordinal").is_err());
        let overflow_site = ElfDirectImportCallSite {
            instruction_offset: usize::MAX,
            relocation_offset: usize::MAX,
            byte_width: 4,
            kind: RelocationKind::X86_64Relative32,
            addend: 0,
        };
        assert!(site_end(&overflow_site).is_err());
        assert!(validate_site_spans(&[overflow_site, overflow_site]).is_err());

        let relocation = FinalImageRelocation {
            section: FinalImageSection::Text,
            offset: usize::MAX,
            byte_width: 4,
            symbol_handle: Handle::invalid(),
            addend: 0,
            kind: RelocationKind::X86_64Relative32,
        };
        assert!(
            call_site(
                TargetProfile::LinuxX64,
                &[0xe8, 0, 0, 0, 0],
                RelocationKind::X86_64Relative32,
                &relocation,
            )
            .is_err()
        );
    }
}
