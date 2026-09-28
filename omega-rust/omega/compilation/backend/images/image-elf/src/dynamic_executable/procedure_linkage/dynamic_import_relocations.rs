//! Address-free procedure-linkage relocation requirements for dynamic imports.
//!
//! The System V ABI defines the dynamic [`DT_JMPREL`], [`DT_PLTRELSZ`],
//! [`DT_PLTREL`], [`DT_RELA`], and [`DT_RELASZ`] relationships. The target
//! relocation numbers and procedure linkage details come from the [x86-64
//! psABI] and [AArch64 ELF ABI]. This module stops at semantic PLT/GOT slots,
//! RELA `JUMP_SLOT` requirements, and general RELA rows; it does not create
//! addresses, final section indexes, GOT/PLT bytes, `Elf64_Rela` bytes,
//! placement, or mutation authority.
//!
//! [`DT_JMPREL`]: https://gabi.xinuos.com/elf/08-dynamic.html#dynamic-section
//! [`DT_PLTRELSZ`]: https://gabi.xinuos.com/elf/08-dynamic.html#dynamic-section
//! [`DT_PLTREL`]: https://gabi.xinuos.com/elf/08-dynamic.html#dynamic-section
//! [`DT_RELA`]: https://gabi.xinuos.com/elf/08-dynamic.html#dynamic-section
//! [`DT_RELASZ`]: https://gabi.xinuos.com/elf/08-dynamic.html#dynamic-section
//! [x86-64 psABI]: https://gitlab.com/x86-psABIs/x86-64-ABI
//! [AArch64 ELF ABI]: https://github.com/ARM-software/abi-aa/blob/main/aaelf64/aaelf64.rst

use crate::dynamic_executable::checked::{checked_u32, require};
use crate::dynamic_executable::import_sections::dynamic_section_descriptors::ValidatedElfDynamicSectionDescriptorPlan;
use crate::dynamic_executable::import_sections::dynamic_sections::ElfDynamicImportBinding;
use crate::imports::{ElfImportLocator, ElfImportRequest};
use diagnostics::Diagnostic;
use image::{FinalImageRelocation, FinalImageSection};
use object_file::RelocationKind;
use target::TargetProfile;

const R_X86_64_JUMP_SLOT: u32 = 7;
const R_AARCH64_JUMP_SLOT: u32 = 1026;
const R_X86_64_64: u32 = 1;
const R_AARCH64_ABS64: u32 = 257;
const ELF64_GENERAL_SLOT_WIDTH: usize = 8;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Independently validated semantic procedure-linkage relocations for all
/// referenced dynamic imports.
///
/// Each logical ordinal names one future PLT entry and its corresponding GOT
/// slot without claiming either section's physical numbering or placement.
/// Unresolved import uses are either direct calls represented by the slot
/// rows or eight-byte `.data` slots bound through general `.rela.dyn` rows.
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

    /// Number of semantic general `.rela.dyn` rows this plan emits, one per
    /// admitted data-slot import reference.
    pub fn general_dynamic_relocation_count(&self) -> usize {
        self.contents.general_relocations.len()
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
    pub(crate) general_relocations: Vec<ElfSemanticGeneralRelocation>,
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

/// One semantic `.rela.dyn` requirement: an exact eight-byte slot inside the
/// retained `.data` image section bound to an import's dynamic symbol at load
/// time. The source offset stays address-free until load placement resolves
/// the future `r_offset` fixup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfSemanticGeneralRelocation {
    pub(crate) request_index: usize,
    pub(crate) dynamic_symbol_index: u32,
    pub(crate) relocation_type: u32,
    pub(crate) source_section: FinalImageSection,
    pub(crate) source_offset: usize,
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
/// relocation shape or by eight-byte aligned `.data` slots. Success creates
/// one logical PLT/GOT slot and one semantic RELA `JUMP_SLOT` requirement per
/// imported dynamic symbol plus one semantic general RELA row per data slot.
/// It deliberately emits no bytes and assigns no address or final section
/// index.
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
    let (source_kind, relocation_type, general_type) =
        target_relocation_spec(inputs.interpreter().target())?;
    let mut slots = Vec::with_capacity(bindings.len());
    let mut jump_slot_relocations = Vec::with_capacity(bindings.len());
    let mut general_relocations = Vec::new();
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
        let (call_sites, mut general) = canonical_relocation_classes(
            inputs.interpreter().target(),
            &inputs.image().memory.text,
            inputs.image().memory.data.len(),
            source_kind,
            general_type,
            binding,
            request,
        )?;
        general_relocations.append(&mut general);
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
    general_relocations.sort_unstable_by_key(|relocation| relocation.source_offset);
    let contents = ElfProcedureLinkageRelocationContents {
        slots,
        jump_slot_relocations,
        general_relocations,
    };
    validate_nonoverlapping_call_sites(&contents)?;
    validate_nonoverlapping_general_slots(&contents)?;
    Ok(contents)
}

fn target_relocation_spec(target: TargetProfile) -> Result<(RelocationKind, u32, u32), Diagnostic> {
    match target {
        TargetProfile::LinuxX64 => Ok((
            RelocationKind::X86_64Relative32,
            R_X86_64_JUMP_SLOT,
            R_X86_64_64,
        )),
        TargetProfile::LinuxArm64 => Ok((
            RelocationKind::Aarch64Branch26,
            R_AARCH64_JUMP_SLOT,
            R_AARCH64_ABS64,
        )),
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

/// Partition one import's retained relocations into the target's exact
/// direct-call sites and eight-byte aligned `.data` slot rows. Any other
/// shape still rejects: there is no third admitted import relocation class.
fn canonical_relocation_classes(
    target: TargetProfile,
    text: &[u8],
    data_len: usize,
    expected_kind: RelocationKind,
    general_type: u32,
    binding: &ElfDynamicImportBinding,
    request: &ElfImportRequest,
) -> Result<
    (
        Vec<ElfDirectImportCallSite>,
        Vec<ElfSemanticGeneralRelocation>,
    ),
    Diagnostic,
> {
    require(
        !request.relocations.is_empty(),
        "ELF procedure-linkage import has no retained relocations",
    )?;
    let mut sites = Vec::new();
    let mut general = Vec::new();
    for relocation in &request.relocations {
        if is_direct_call_shape(relocation, expected_kind) {
            sites.push(call_site(target, text, expected_kind, relocation)?);
        } else {
            general.push(general_slot(data_len, general_type, binding, relocation)?);
        }
    }
    sites.sort_unstable_by_key(|site| (site.instruction_offset, site.relocation_offset));
    validate_site_spans(&sites)?;
    Ok((sites, general))
}

fn is_direct_call_shape(relocation: &FinalImageRelocation, expected_kind: RelocationKind) -> bool {
    relocation.section == FinalImageSection::Text
        && relocation.kind == expected_kind
        && relocation.byte_width == 4
        && relocation.addend == 0
}

/// Admit one `.data` import slot as a semantic general RELA row. The admitted
/// shape is exactly the object envelope's data relocation contract: an
/// eight-byte-aligned `Absolute64` slot that stays inside `.data`.
fn general_slot(
    data_len: usize,
    general_type: u32,
    binding: &ElfDynamicImportBinding,
    relocation: &FinalImageRelocation,
) -> Result<ElfSemanticGeneralRelocation, Diagnostic> {
    require(
        relocation.section == FinalImageSection::Data
            && relocation.kind == RelocationKind::Absolute64
            && relocation.byte_width == ELF64_GENERAL_SLOT_WIDTH
            && relocation.offset.is_multiple_of(ELF64_GENERAL_SLOT_WIDTH),
        "ELF dynamic import requires a non-procedure or malformed source relocation",
    )?;
    relocation
        .offset
        .checked_add(ELF64_GENERAL_SLOT_WIDTH)
        .filter(|end| *end <= data_len)
        .ok_or_else(|| Diagnostic::error("ELF dynamic import data slot exceeds .data"))?;
    Ok(ElfSemanticGeneralRelocation {
        request_index: binding.request_index,
        dynamic_symbol_index: binding.dynamic_symbol_index,
        relocation_type: general_type,
        source_section: relocation.section,
        source_offset: relocation.offset,
        addend: relocation.addend,
    })
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

fn validate_nonoverlapping_general_slots(
    contents: &ElfProcedureLinkageRelocationContents,
) -> Result<(), Diagnostic> {
    for pair in contents.general_relocations.windows(2) {
        require(
            pair[0].source_offset != pair[1].source_offset,
            "ELF import data slots overlap or repeat one eight-byte span",
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
    let (source_kind, relocation_type, general_type) =
        target_relocation_spec(inputs.interpreter().target())?;
    let mut expected_general = Vec::new();
    for (index, binding) in bindings.iter().enumerate() {
        let logical_ordinal = checked_u32(index, "validated logical PLT/GOT slot ordinal")?;
        let request = inputs.imports().get(binding.request_index).ok_or_else(|| {
            Diagnostic::error("validated ELF binding exceeds canonical import requests")
        })?;
        let compatibility_report_identity = request_compatibility_report_identity(request)?;
        let (expected_sites, mut binding_general) = canonical_relocation_classes(
            inputs.interpreter().target(),
            &inputs.image().memory.text,
            inputs.image().memory.data.len(),
            source_kind,
            general_type,
            binding,
            request,
        )?;
        expected_general.append(&mut binding_general);
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
    expected_general.sort_unstable_by_key(|relocation| relocation.source_offset);
    require(
        contents.general_relocations == expected_general,
        "ELF general RELA rows drifted from their exact import bindings",
    )?;
    require_unique_semantic_rows(contents)?;
    validate_nonoverlapping_call_sites(contents)?;
    validate_nonoverlapping_general_slots(contents)
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
    hash.bytes(&(contents.general_relocations.len() as u64).to_le_bytes());
    for relocation in &contents.general_relocations {
        hash.bytes(&(relocation.request_index as u64).to_le_bytes());
        hash.bytes(&relocation.dynamic_symbol_index.to_le_bytes());
        hash.bytes(&relocation.relocation_type.to_le_bytes());
        hash.byte(section_tag(relocation.source_section));
        hash.bytes(&(relocation.source_offset as u64).to_le_bytes());
        hash.bytes(&relocation.addend.to_le_bytes());
    }
    hash.finish()
}

const fn section_tag(section: FinalImageSection) -> u8 {
    match section {
        FinalImageSection::Text => 1,
        FinalImageSection::Data => 2,
        FinalImageSection::Bss => 3,
        FinalImageSection::None => 4,
    }
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
mod tests;
