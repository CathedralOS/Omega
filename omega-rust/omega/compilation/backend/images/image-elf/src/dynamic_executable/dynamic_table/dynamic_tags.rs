//! Address-free semantic planning for the ELF `.dynamic` array.
//!
//! The generic [System V ABI dynamic section] defines the required tag/value
//! relationships and the significant relative order of `DT_NEEDED` rows. The
//! [LSB symbol-version ABI] defines `DT_VERSYM`, `DT_VERNEED`, and
//! `DT_VERNEEDNUM`. This module retains those meanings as typed rows and exact
//! semantic address obligations; it emits no `Elf64_Dyn` bytes or addresses.
//! The [original GNU implementation] defines `DT_GNU_HASH` custody for the
//! companion `.gnu.hash` payload.
//!
//! [System V ABI dynamic section]: https://gabi.xinuos.com/elf/08-dynamic.html
//! [LSB symbol-version ABI]: https://refspecs.linuxfoundation.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/symversion.html
//! [original GNU implementation]: https://sourceware.org/pipermail/binutils/2006-July/048074.html

use target::compatibility_fingerprint::Fnv1a;

use super::super::checked::{self, CandidateValidationError};
use crate::constants::ELF64_RELA_SIZE;
use crate::dynamic_executable::checked::{checked_product, checked_sum, checked_u32, require};
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_descriptors::{
    ElfProcedureLinkageSectionKind, ValidatedElfProcedureLinkageSectionDescriptorPlan,
};
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::{
    ElfProcedureLinkageFixupKind, ElfProcedureLinkageFixupStorage,
    ElfProcedureLinkageSemanticTarget,
};
use diagnostics::Diagnostic;
use target::TargetProfile;

const ELF64_DYNAMIC_SYMBOL_SIZE: u64 = 24;
const ELF64_DYN_VALUE_SIZE: u8 = 8;
const UNVERSIONED_FIXED_ROW_COUNT: usize = 14;
const UNVERSIONED_ADDRESS_COUNT: usize = 7;
/// Independently validated semantic `.dynamic` rows and address obligations.
///
/// The exact eleven-section descriptor carrier remains owned by this non-clone
/// plan. No row contains a placed pointer, final section index, serialized
/// `Elf64_Dyn`, program header, image mutation, or runnable-image authority.
#[derive(Debug)]
#[must_use = "validated ELF dynamic tags retain the exact descriptor carrier"]
pub struct ValidatedElfDynamicTagPlan {
    descriptors: ValidatedElfProcedureLinkageSectionDescriptorPlan,
    contents: ElfDynamicTagContents,
    non_authoritative_tag_compatibility_fingerprint: u64,
}

impl ValidatedElfDynamicTagPlan {
    pub const fn descriptors(&self) -> &ValidatedElfProcedureLinkageSectionDescriptorPlan {
        &self.descriptors
    }

    pub(crate) fn has_version_requirements(&self) -> bool {
        !structural_contents(&self.descriptors).verneed.is_empty()
    }

    pub fn row_count(&self) -> usize {
        self.contents.rows.len()
    }

    pub(crate) fn needed_row_count(&self) -> usize {
        self.descriptors
            .templates()
            .linkage()
            .descriptors()
            .payloads()
            .plan()
            .needed_object_count()
    }

    pub(crate) fn address_obligation_count(&self) -> usize {
        self.contents.address_obligations.len()
    }

    /// Compatibility fingerprint of the owning eleven-section descriptor
    /// identity, exact typed row sequence, and retained semantic address
    /// obligations. This is not final-byte or loader identity.
    pub const fn non_authoritative_tag_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_tag_compatibility_fingerprint
    }

    pub(crate) const fn contents(&self) -> &ElfDynamicTagContents {
        &self.contents
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidatedElfProcedureLinkageSectionDescriptorPlan,
        ElfDynamicTagContents,
    ) {
        (self.descriptors, self.contents)
    }
}

/// Rejected semantic dynamic-tag planning with exact descriptor custody.
#[derive(Debug)]
#[must_use = "ELF dynamic-tag rejection retains the eleven-section carrier"]
pub struct ElfDynamicTagPlanningError {
    descriptors: ValidatedElfProcedureLinkageSectionDescriptorPlan,
    diagnostic: Diagnostic,
}

image::rejection_carrier!(
    ElfDynamicTagPlanningError;
    descriptors: ValidatedElfProcedureLinkageSectionDescriptorPlan,
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfDynamicTagContents {
    pub(crate) rows: Vec<ElfDynamicSemanticRow>,
    pub(crate) address_obligations: Vec<ElfDynamicAddressObligation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub(crate) enum ElfDynamicTag {
    Null = 0,
    Needed = 1,
    ProcedureRelocationSize = 2,
    ProcedureGot = 3,
    SystemVHash = 4,
    DynamicString = 5,
    DynamicSymbol = 6,
    Rela = 7,
    GeneralRelocationSize = 8,
    GeneralRelocationEntrySize = 9,
    DynamicStringSize = 10,
    DynamicSymbolEntrySize = 11,
    ProcedureRelocationKind = 20,
    ProcedureRelocation = 23,
    GnuSymbolVersion = 0x6fff_fff0,
    GnuHash = 0x6fff_fef5,
    GnuVersionRequirement = 0x6fff_fffe,
    GnuVersionRequirementCount = 0x6fff_ffff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ElfDynamicValue {
    NeededStringOffset(u32),
    ProcedureRelocationByteCount(u64),
    GeneralRelocationByteCount(u64),
    GeneralRelocationEntryByteCount(u64),
    AddressPlaceholder,
    DynamicStringByteCount(u64),
    DynamicSymbolEntryByteCount(u64),
    RelocationTag(ElfDynamicTag),
    VersionRequirementRecordCount(u64),
    Null,
}

pub(crate) const fn encoded_value(value: ElfDynamicValue) -> u64 {
    match value {
        ElfDynamicValue::NeededStringOffset(offset) => offset as u64,
        ElfDynamicValue::ProcedureRelocationByteCount(count)
        | ElfDynamicValue::GeneralRelocationByteCount(count)
        | ElfDynamicValue::GeneralRelocationEntryByteCount(count)
        | ElfDynamicValue::DynamicStringByteCount(count)
        | ElfDynamicValue::DynamicSymbolEntryByteCount(count)
        | ElfDynamicValue::VersionRequirementRecordCount(count) => count,
        ElfDynamicValue::RelocationTag(tag) => (tag as i64) as u64,
        ElfDynamicValue::AddressPlaceholder | ElfDynamicValue::Null => 0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfDynamicSemanticRow {
    pub(crate) tag: ElfDynamicTag,
    pub(crate) value: ElfDynamicValue,
}

/// The tag retains the applied address target verbatim.
pub(crate) type ElfDynamicAddressTarget = crate::ElfDynamicAddressApplicationTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfDynamicAddressObligation {
    pub(crate) row_ordinal: u32,
    pub(crate) byte_width: u8,
    pub(crate) target: ElfDynamicAddressTarget,
}

struct Candidate {
    descriptors: ValidatedElfProcedureLinkageSectionDescriptorPlan,
    contents: ElfDynamicTagContents,
    non_authoritative_tag_compatibility_fingerprint: u64,
}

/// Consume the exact eleven-section address-free carrier into a complete
/// semantic `.dynamic` tag sequence and typed future-address obligations.
///
/// The planner deliberately stops before `Elf64_Dyn` serialization, `.dynamic`
/// section descriptors, final section indexes, placement, or byte mutation.
pub(crate) fn plan_elf_dynamic_tags(
    descriptors: ValidatedElfProcedureLinkageSectionDescriptorPlan,
) -> Result<ValidatedElfDynamicTagPlan, Box<ElfDynamicTagPlanningError>> {
    checked::plan_candidate(
        descriptors,
        derive_contents,
        |descriptors, contents| {
            non_authoritative_tag_compatibility_fingerprint(&descriptors, &contents)
        },
        |descriptors, contents, fingerprint| Candidate {
            descriptors: descriptors,
            contents: contents,
            non_authoritative_tag_compatibility_fingerprint: fingerprint,
        },
        validate_candidate,
        |candidate| candidate.descriptors,
        |descriptors, diagnostic| {
            Box::new(ElfDynamicTagPlanningError {
                descriptors: descriptors,
                diagnostic,
            })
        },
    )
}

fn derive_contents(
    descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
) -> Result<ElfDynamicTagContents, Diagnostic> {
    let structural = structural_contents(descriptors);
    let row_capacity = checked_sum(
        structural.needed.len(),
        UNVERSIONED_FIXED_ROW_COUNT + 3 * usize::from(!structural.verneed.is_empty()),
        "ELF dynamic row count",
    )?;
    let mut rows = Vec::with_capacity(row_capacity);
    let mut address_obligations = Vec::with_capacity(
        UNVERSIONED_ADDRESS_COUNT + 2 * usize::from(!structural.verneed.is_empty()),
    );
    for needed in &structural.needed {
        rows.push(ElfDynamicSemanticRow {
            tag: ElfDynamicTag::Needed,
            value: ElfDynamicValue::NeededStringOffset(*needed),
        });
    }
    rows.push(ElfDynamicSemanticRow {
        tag: ElfDynamicTag::ProcedureRelocationSize,
        value: ElfDynamicValue::ProcedureRelocationByteCount(super::super::checked_u64(
            descriptors.templates().procedure_relocation_byte_count(),
            "procedure relocation byte count",
        )?),
    });
    push_address_row(
        &mut rows,
        &mut address_obligations,
        ElfDynamicTag::ProcedureGot,
        ElfDynamicAddressTarget::ProcedureGot,
    )?;
    push_address_row(
        &mut rows,
        &mut address_obligations,
        ElfDynamicTag::SystemVHash,
        ElfDynamicAddressTarget::SystemVHash,
    )?;
    push_address_row(
        &mut rows,
        &mut address_obligations,
        ElfDynamicTag::GnuHash,
        ElfDynamicAddressTarget::GnuHash,
    )?;
    push_address_row(
        &mut rows,
        &mut address_obligations,
        ElfDynamicTag::DynamicString,
        ElfDynamicAddressTarget::DynamicString,
    )?;
    push_address_row(
        &mut rows,
        &mut address_obligations,
        ElfDynamicTag::DynamicSymbol,
        ElfDynamicAddressTarget::DynamicSymbol,
    )?;
    rows.push(ElfDynamicSemanticRow {
        tag: ElfDynamicTag::DynamicStringSize,
        value: ElfDynamicValue::DynamicStringByteCount(super::super::checked_u64(
            dynamic_payloads(descriptors).dynamic_string_byte_count(),
            "dynamic string byte count",
        )?),
    });
    rows.push(ElfDynamicSemanticRow {
        tag: ElfDynamicTag::DynamicSymbolEntrySize,
        value: ElfDynamicValue::DynamicSymbolEntryByteCount(ELF64_DYNAMIC_SYMBOL_SIZE),
    });
    rows.push(ElfDynamicSemanticRow {
        tag: ElfDynamicTag::ProcedureRelocationKind,
        value: ElfDynamicValue::RelocationTag(ElfDynamicTag::Rela),
    });
    push_address_row(
        &mut rows,
        &mut address_obligations,
        ElfDynamicTag::ProcedureRelocation,
        ElfDynamicAddressTarget::ProcedureRelocation,
    )?;
    push_address_row(
        &mut rows,
        &mut address_obligations,
        ElfDynamicTag::Rela,
        ElfDynamicAddressTarget::GeneralRelocation,
    )?;
    rows.push(ElfDynamicSemanticRow {
        tag: ElfDynamicTag::GeneralRelocationSize,
        value: ElfDynamicValue::GeneralRelocationByteCount(super::super::checked_u64(
            descriptors.templates().general_relocation_byte_count(),
            "general relocation byte count",
        )?),
    });
    rows.push(ElfDynamicSemanticRow {
        tag: ElfDynamicTag::GeneralRelocationEntrySize,
        value: ElfDynamicValue::GeneralRelocationEntryByteCount(super::super::checked_u64(
            ELF64_RELA_SIZE,
            "general relocation entry size",
        )?),
    });
    // DT_VERSYM directs lazy symbol resolution through the loader's version
    // table even for a global index of one. An unversioned-only executable has
    // no Verneed/Verdef records from which that table could be established.
    // Likewise a present DT_VERNEED must point at an actual first record.
    if !structural.verneed.is_empty() {
        push_address_row(
            &mut rows,
            &mut address_obligations,
            ElfDynamicTag::GnuSymbolVersion,
            ElfDynamicAddressTarget::GnuSymbolVersion,
        )?;
        push_address_row(
            &mut rows,
            &mut address_obligations,
            ElfDynamicTag::GnuVersionRequirement,
            ElfDynamicAddressTarget::GnuVersionRequirement,
        )?;
        rows.push(ElfDynamicSemanticRow {
            tag: ElfDynamicTag::GnuVersionRequirementCount,
            value: ElfDynamicValue::VersionRequirementRecordCount(super::super::checked_u64(
                structural.verneed.len(),
                "GNU version-requirement record count",
            )?),
        });
    }
    rows.push(ElfDynamicSemanticRow {
        tag: ElfDynamicTag::Null,
        value: ElfDynamicValue::Null,
    });
    Ok(ElfDynamicTagContents {
        rows,
        address_obligations,
    })
}

fn push_address_row(
    rows: &mut Vec<ElfDynamicSemanticRow>,
    obligations: &mut Vec<ElfDynamicAddressObligation>,
    tag: ElfDynamicTag,
    target: ElfDynamicAddressTarget,
) -> Result<(), Diagnostic> {
    let row_ordinal = checked_u32(rows.len(), "ELF dynamic address-row ordinal")?;
    rows.push(ElfDynamicSemanticRow {
        tag,
        value: ElfDynamicValue::AddressPlaceholder,
    });
    obligations.push(ElfDynamicAddressObligation {
        row_ordinal,
        byte_width: ELF64_DYN_VALUE_SIZE,
        target,
    });
    Ok(())
}

fn dynamic_payloads(
    descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
) -> &crate::dynamic_executable::import_sections::dynamic_section_bytes::ValidatedElfDynamicSectionPayloads{
    descriptors.templates().linkage().descriptors().payloads()
}

fn structural_contents(
    descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
) -> &crate::dynamic_executable::import_sections::dynamic_sections::ElfDynamicSectionContents {
    dynamic_payloads(descriptors).plan().contents()
}

fn target(descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan) -> TargetProfile {
    dynamic_payloads(descriptors)
        .plan()
        .inputs()
        .interpreter()
        .target()
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfDynamicTagPlan, CandidateValidationError<Candidate>> {
    checked::validate_candidate(
        candidate,
        |candidate| validate_contents(&candidate.descriptors, &candidate.contents),
        |candidate| {
            candidate.non_authoritative_tag_compatibility_fingerprint
                == non_authoritative_tag_compatibility_fingerprint(
                    &candidate.descriptors,
                    &candidate.contents,
                )
        },
        "ELF dynamic-tag compatibility fingerprint does not replay",
        |candidate| ValidatedElfDynamicTagPlan {
            descriptors: candidate.descriptors,
            contents: candidate.contents,
            non_authoritative_tag_compatibility_fingerprint: candidate
                .non_authoritative_tag_compatibility_fingerprint,
        },
    )
}

fn validate_contents(
    descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
    contents: &ElfDynamicTagContents,
) -> Result<(), Diagnostic> {
    require(
        descriptors.descriptor_count() == 11,
        "ELF dynamic tags require the exact eleven-section descriptor carrier",
    )?;
    require(
        matches!(
            target(descriptors),
            TargetProfile::LinuxX64 | TargetProfile::LinuxArm64
        ),
        "ELF dynamic tags require an exact supported Linux profile",
    )?;
    let structural = structural_contents(descriptors);
    require(
        !structural.needed.is_empty(),
        "ELF dynamic tags require at least one owned DT_NEEDED row",
    )?;
    let expected_row_count = checked_sum(
        structural.needed.len(),
        UNVERSIONED_FIXED_ROW_COUNT + 3 * usize::from(!structural.verneed.is_empty()),
        "validated ELF dynamic row count",
    )?;
    require(
        contents.rows.len() == expected_row_count,
        "ELF dynamic tag row count is not exact",
    )?;
    validate_needed_rows(descriptors, contents, structural.needed.len())?;
    validate_fixed_rows(descriptors, contents, structural.needed.len())?;
    validate_descriptor_targets(descriptors)?;
    validate_address_obligations(contents, !structural.verneed.is_empty())?;
    validate_relocation_closure(descriptors)?;
    Ok(())
}

fn validate_descriptor_targets(
    descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
) -> Result<(), Diagnostic> {
    let appended = &descriptors.contents().descriptors;
    for kind in [
        ElfProcedureLinkageSectionKind::ProcedureGot,
        ElfProcedureLinkageSectionKind::ProcedureRelocation,
    ] {
        require(
            appended.iter().filter(|row| row.kind == kind).count() == 1,
            "ELF dynamic address target is not uniquely owned by the linkage descriptors",
        )?;
    }
    Ok(())
}

fn validate_needed_rows(
    descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
    contents: &ElfDynamicTagContents,
    needed_count: usize,
) -> Result<(), Diagnostic> {
    let structural = structural_contents(descriptors);
    let dynstr = &dynamic_payloads(descriptors).payloads().dynstr;
    let mut prior = None;
    for (index, expected_offset) in structural.needed.iter().enumerate() {
        let row = contents
            .rows
            .get(index)
            .ok_or_else(|| Diagnostic::error("missing DT_NEEDED row"))?;
        require(
            *row == (ElfDynamicSemanticRow {
                tag: ElfDynamicTag::Needed,
                value: ElfDynamicValue::NeededStringOffset(*expected_offset),
            }),
            "DT_NEEDED row drifted from the exact significant roster",
        )?;
        let object = dynamic_string(dynstr, *expected_offset)?;
        require(
            !object.is_empty(),
            "DT_NEEDED references an empty object name",
        )?;
        if let Some(prior_object) = prior {
            require(
                prior_object < object,
                "DT_NEEDED rows are duplicated or not in canonical significant order",
            )?;
        }
        prior = Some(object);
    }
    require(
        contents
            .rows
            .get(needed_count)
            .is_some_and(|row| row.tag != ElfDynamicTag::Needed),
        "DT_NEEDED rows are not one contiguous significant prefix",
    )
}

fn validate_fixed_rows(
    descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
    contents: &ElfDynamicTagContents,
    base: usize,
) -> Result<(), Diagnostic> {
    let structural = structural_contents(descriptors);
    let mut fixed = vec![
        ElfDynamicSemanticRow {
            tag: ElfDynamicTag::ProcedureRelocationSize,
            value: ElfDynamicValue::ProcedureRelocationByteCount(super::super::checked_u64(
                descriptors.templates().procedure_relocation_byte_count(),
                "validated procedure relocation size",
            )?),
        },
        address_row(ElfDynamicTag::ProcedureGot),
        address_row(ElfDynamicTag::SystemVHash),
        address_row(ElfDynamicTag::GnuHash),
        address_row(ElfDynamicTag::DynamicString),
        address_row(ElfDynamicTag::DynamicSymbol),
        ElfDynamicSemanticRow {
            tag: ElfDynamicTag::DynamicStringSize,
            value: ElfDynamicValue::DynamicStringByteCount(super::super::checked_u64(
                dynamic_payloads(descriptors).dynamic_string_byte_count(),
                "validated dynamic string size",
            )?),
        },
        ElfDynamicSemanticRow {
            tag: ElfDynamicTag::DynamicSymbolEntrySize,
            value: ElfDynamicValue::DynamicSymbolEntryByteCount(ELF64_DYNAMIC_SYMBOL_SIZE),
        },
        ElfDynamicSemanticRow {
            tag: ElfDynamicTag::ProcedureRelocationKind,
            value: ElfDynamicValue::RelocationTag(ElfDynamicTag::Rela),
        },
        address_row(ElfDynamicTag::ProcedureRelocation),
        address_row(ElfDynamicTag::Rela),
        ElfDynamicSemanticRow {
            tag: ElfDynamicTag::GeneralRelocationSize,
            value: ElfDynamicValue::GeneralRelocationByteCount(super::super::checked_u64(
                descriptors.templates().general_relocation_byte_count(),
                "validated general relocation size",
            )?),
        },
        ElfDynamicSemanticRow {
            tag: ElfDynamicTag::GeneralRelocationEntrySize,
            value: ElfDynamicValue::GeneralRelocationEntryByteCount(super::super::checked_u64(
                ELF64_RELA_SIZE,
                "validated general relocation entry size",
            )?),
        },
    ];
    if !structural.verneed.is_empty() {
        fixed.extend([
            address_row(ElfDynamicTag::GnuSymbolVersion),
            address_row(ElfDynamicTag::GnuVersionRequirement),
            ElfDynamicSemanticRow {
                tag: ElfDynamicTag::GnuVersionRequirementCount,
                value: ElfDynamicValue::VersionRequirementRecordCount(super::super::checked_u64(
                    structural.verneed.len(),
                    "validated version-requirement count",
                )?),
            },
        ]);
    }
    fixed.push(ElfDynamicSemanticRow {
        tag: ElfDynamicTag::Null,
        value: ElfDynamicValue::Null,
    });
    require(
        contents.rows.get(base..) == Some(fixed.as_slice()),
        "fixed ELF dynamic tags are missing, duplicated, reordered, or malformed",
    )?;
    let relocation_bytes = descriptors.templates().procedure_relocation_byte_count();
    let relocation_count = descriptors
        .templates()
        .linkage()
        .procedure_relocation_count();
    require(
        relocation_bytes > 0
            && relocation_bytes.is_multiple_of(ELF64_RELA_SIZE)
            && relocation_bytes
                == checked_product(
                    relocation_count,
                    ELF64_RELA_SIZE,
                    "validated PLT relocation byte count",
                )?,
        "DT_PLTRELSZ does not match exact Elf64_Rela JUMP_SLOT rows",
    )?;
    require(
        structural.verneed.len()
            == dynamic_payloads(descriptors)
                .plan()
                .required_version_object_count(),
        "DT_VERNEEDNUM does not cover the exact versioned-object roster",
    )
}

const fn address_row(tag: ElfDynamicTag) -> ElfDynamicSemanticRow {
    ElfDynamicSemanticRow {
        tag,
        value: ElfDynamicValue::AddressPlaceholder,
    }
}

fn validate_address_obligations(
    contents: &ElfDynamicTagContents,
    has_version_requirements: bool,
) -> Result<(), Diagnostic> {
    let expected = [
        (
            ElfDynamicTag::ProcedureGot,
            ElfDynamicAddressTarget::ProcedureGot,
        ),
        (
            ElfDynamicTag::SystemVHash,
            ElfDynamicAddressTarget::SystemVHash,
        ),
        (ElfDynamicTag::GnuHash, ElfDynamicAddressTarget::GnuHash),
        (
            ElfDynamicTag::DynamicString,
            ElfDynamicAddressTarget::DynamicString,
        ),
        (
            ElfDynamicTag::DynamicSymbol,
            ElfDynamicAddressTarget::DynamicSymbol,
        ),
        (
            ElfDynamicTag::ProcedureRelocation,
            ElfDynamicAddressTarget::ProcedureRelocation,
        ),
        (
            ElfDynamicTag::Rela,
            ElfDynamicAddressTarget::GeneralRelocation,
        ),
        (
            ElfDynamicTag::GnuSymbolVersion,
            ElfDynamicAddressTarget::GnuSymbolVersion,
        ),
        (
            ElfDynamicTag::GnuVersionRequirement,
            ElfDynamicAddressTarget::GnuVersionRequirement,
        ),
    ];
    let expected_count = UNVERSIONED_ADDRESS_COUNT + 2 * usize::from(has_version_requirements);
    let expected = &expected[..expected_count];
    require(
        contents.address_obligations.len() == expected.len(),
        "ELF dynamic address-obligation count is not exact",
    )?;
    for (obligation, (tag, target)) in contents
        .address_obligations
        .iter()
        .zip(expected.iter().copied())
    {
        let row_index = usize::try_from(obligation.row_ordinal)
            .map_err(|_| Diagnostic::error("ELF dynamic row ordinal exceeds usize"))?;
        require(
            obligation.byte_width == ELF64_DYN_VALUE_SIZE
                && obligation.target == target
                && contents.rows.get(row_index) == Some(&address_row(tag)),
            "ELF dynamic address obligation drifted from its exact semantic row",
        )?;
    }
    for (index, row) in contents.rows.iter().enumerate() {
        let obligation_count = contents
            .address_obligations
            .iter()
            .filter(|obligation| obligation.row_ordinal as usize == index)
            .count();
        require(
            obligation_count
                == usize::from(matches!(row.value, ElfDynamicValue::AddressPlaceholder)),
            "ELF dynamic row has a missing, duplicate, or orphan address obligation",
        )?;
    }
    Ok(())
}

fn validate_relocation_closure(
    descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
) -> Result<(), Diagnostic> {
    let linkage = descriptors.templates().linkage();
    require(
        checked_product(
            linkage.general_dynamic_relocation_count(),
            ELF64_RELA_SIZE,
            "general relocation table byte count",
        )? == descriptors.templates().general_relocation_byte_count(),
        "general relocation table drifts from its semantic relocation rows",
    )?;
    let future_dynamic = descriptors
        .templates()
        .contents()
        .fixups
        .iter()
        .filter(|fixup| {
            fixup.storage == ElfProcedureLinkageFixupStorage::GotPlt
                && fixup.kind == ElfProcedureLinkageFixupKind::Absolute64
                && fixup.target == ElfProcedureLinkageSemanticTarget::FutureDynamicSection
        })
        .count();
    require(
        future_dynamic
            == match target(descriptors) {
                TargetProfile::LinuxX64 => 1,
                TargetProfile::LinuxArm64 => 0,
                _ => return Err(Diagnostic::error("unsupported relocation-closure target")),
            },
        "target GOT policy drifted from the future .dynamic section obligation",
    )
}

fn dynamic_string(bytes: &[u8], offset: u32) -> Result<&[u8], Diagnostic> {
    let offset = usize::try_from(offset)
        .map_err(|_| Diagnostic::error("DT_NEEDED string offset exceeds usize"))?;
    let tail = bytes
        .get(offset..)
        .ok_or_else(|| Diagnostic::error("DT_NEEDED string offset is outside .dynstr"))?;
    let terminator = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| Diagnostic::error("DT_NEEDED object name is not NUL-terminated"))?;
    Ok(&tail[..terminator])
}

fn non_authoritative_tag_compatibility_fingerprint(
    descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
    contents: &ElfDynamicTagContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-dynamic-tags.v2");
    hash.bytes(
        &descriptors
            .non_authoritative_descriptor_compatibility_fingerprint()
            .to_le_bytes(),
    );
    hash.bytes(&(contents.rows.len() as u64).to_le_bytes());
    for row in &contents.rows {
        hash.bytes(&(row.tag as i64).to_le_bytes());
        match row.value {
            ElfDynamicValue::NeededStringOffset(offset) => {
                hash.byte(1);
                hash.bytes(&offset.to_le_bytes());
            }
            ElfDynamicValue::ProcedureRelocationByteCount(count) => tagged_u64(&mut hash, 2, count),
            ElfDynamicValue::GeneralRelocationByteCount(count) => tagged_u64(&mut hash, 9, count),
            ElfDynamicValue::GeneralRelocationEntryByteCount(count) => {
                tagged_u64(&mut hash, 10, count)
            }
            ElfDynamicValue::AddressPlaceholder => hash.byte(3),
            ElfDynamicValue::DynamicStringByteCount(count) => tagged_u64(&mut hash, 4, count),
            ElfDynamicValue::DynamicSymbolEntryByteCount(count) => tagged_u64(&mut hash, 5, count),
            ElfDynamicValue::RelocationTag(tag) => {
                hash.byte(6);
                hash.bytes(&(tag as i64).to_le_bytes());
            }
            ElfDynamicValue::VersionRequirementRecordCount(count) => {
                tagged_u64(&mut hash, 7, count)
            }
            ElfDynamicValue::Null => hash.byte(8),
        }
    }
    hash.bytes(&(contents.address_obligations.len() as u64).to_le_bytes());
    for obligation in &contents.address_obligations {
        hash.bytes(&obligation.row_ordinal.to_le_bytes());
        hash.byte(obligation.byte_width);
        hash.byte(obligation.target as u8);
    }
    hash.finish()
}

fn tagged_u64(hash: &mut Fnv1a, tag: u8, value: u64) {
    hash.byte(tag);
    hash.bytes(&value.to_le_bytes());
}

#[cfg(test)]
mod tests;
