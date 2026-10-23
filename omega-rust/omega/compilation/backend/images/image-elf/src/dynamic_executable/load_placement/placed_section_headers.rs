//! Applied ELF64-LSB section-header placement fields.
//!
//! This layer consumes the independently validated absolute dynamic-load
//! geometry and copies its retained fourteen-row `Elf64_Shdr` template. It
//! applies only the twenty-five already-resolved `sh_addr` and `sh_offset`
//! fields, then decodes the complete table and replays every field against the
//! retained numeric roster and placement ledger.
//!
//! It does not choose or serialize `e_shoff`, an ELF header, or program
//! headers; resolve payload-internal dynamic/procedure/source relocations;
//! mutate the retained `FinalImage`; or claim a runnable image.

use target::compatibility_fingerprint::Fnv1a;

use super::super::checked::{self, CandidateValidationError};
use crate::constants::{ELF64_SECTION_HEADER_SIZE, SECTION_COUNT};
use crate::dynamic_executable::checked::{
    DecodedElf64SectionHeader, checked_product, checked_sum, decode_section_header_rows, field_mut,
    read_u64, require,
};
use crate::dynamic_executable::load_placement::load_layout::{
    ElfResolvedSectionHeaderPlacement, ElfSectionPlacementResolutionKind,
    ValidatedElfDynamicLoadLayout, public_section_kind,
};
use crate::dynamic_executable::section_headers::section_roster::ElfNumericSectionDescriptor;
use diagnostics::Diagnostic;

const ELF64_SECTION_ADDRESS_OFFSET: usize = 16;
const ELF64_SECTION_FILE_OFFSET: usize = 24;
const ELF64_PLACEMENT_FIELD_SIZE: u8 = 8;
const FILE_OFFSET_PLACEMENT_COUNT: usize = 13;
const VIRTUAL_ADDRESS_PLACEMENT_COUNT: usize = 12;
const PLACEMENT_COUNT: usize = 25;
const SECTION_NAME_TABLE_INDEX: usize = 13;
/// One placement application retained beside the resulting bytes — the
/// resolved placement itself, verbatim.
pub(crate) type ElfAppliedSectionHeaderPlacement = ElfResolvedSectionHeaderPlacement;

/// Independently decoded section-header bytes retaining exact load-layout
/// custody.
///
/// This non-clone carrier proves only that the retained 832-byte template has
/// received its exact twenty-five load-layout placements. It carries no final
/// file placement, mutation, publication, or runnable-image authority.
#[derive(Debug)]
#[must_use = "applied ELF section headers retain the absolute load layout"]
pub struct ValidatedElfPlacedSectionHeaderTable {
    load_layout: ValidatedElfDynamicLoadLayout,
    contents: ElfPlacedSectionHeaderContents,
    non_authoritative_placed_compatibility_fingerprint: u64,
}

impl ValidatedElfPlacedSectionHeaderTable {
    pub const fn load_layout(&self) -> &ValidatedElfDynamicLoadLayout {
        &self.load_layout
    }

    pub fn bytes(&self) -> &[u8] {
        &self.contents.bytes
    }

    pub(crate) fn applied_placements(&self) -> &[ElfAppliedSectionHeaderPlacement] {
        &self.contents.applications
    }

    pub const fn non_authoritative_placed_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_placed_compatibility_fingerprint
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidatedElfDynamicLoadLayout,
        ElfPlacedSectionHeaderContents,
    ) {
        (self.load_layout, self.contents)
    }
}

/// Rejected application retaining exact absolute-load-layout custody.
#[derive(Debug)]
#[must_use = "section-header placement rejection retains the absolute load layout"]
pub struct ElfSectionHeaderPlacementApplicationError {
    load_layout: ValidatedElfDynamicLoadLayout,
    diagnostic: Diagnostic,
}

impl ElfSectionHeaderPlacementApplicationError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfDynamicLoadLayout, Diagnostic) {
        (self.load_layout, self.diagnostic)
    }
}

diagnostics::diagnostic_display!(ElfSectionHeaderPlacementApplicationError);

impl std::error::Error for ElfSectionHeaderPlacementApplicationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfPlacedSectionHeaderContents {
    pub(crate) bytes: Vec<u8>,
    pub(crate) applications: Vec<ElfAppliedSectionHeaderPlacement>,
}

struct Candidate {
    load_layout: ValidatedElfDynamicLoadLayout,
    contents: ElfPlacedSectionHeaderContents,
    non_authoritative_placed_compatibility_fingerprint: u64,
}

/// Copy the retained section-header template and apply its exact resolved
/// address and file-offset values in little-endian form.
pub(crate) fn apply_elf_section_header_placements(
    load_layout: ValidatedElfDynamicLoadLayout,
) -> Result<ValidatedElfPlacedSectionHeaderTable, Box<ElfSectionHeaderPlacementApplicationError>> {
    checked::plan_candidate(
        load_layout,
        derive_contents,
        |load_layout, contents| {
            non_authoritative_placed_compatibility_fingerprint(&load_layout, &contents)
        },
        |load_layout, contents, fingerprint| Candidate {
            load_layout: load_layout,
            contents: contents,
            non_authoritative_placed_compatibility_fingerprint: fingerprint,
        },
        validate_candidate,
        |candidate| candidate.load_layout,
        |load_layout, diagnostic| {
            Box::new(ElfSectionHeaderPlacementApplicationError {
                load_layout: load_layout,
                diagnostic,
            })
        },
    )
}

fn derive_contents(
    load_layout: &ValidatedElfDynamicLoadLayout,
) -> Result<ElfPlacedSectionHeaderContents, Diagnostic> {
    let template = load_layout
        .relative()
        .payloads()
        .section_headers()
        .contents();
    require(
        template.bytes.len() == SECTION_COUNT * ELF64_SECTION_HEADER_SIZE,
        "placed ELF section-header input is not the exact 832-byte template",
    )?;
    require(
        template.placement_fixups.len() == PLACEMENT_COUNT
            && load_layout.section_header_resolutions().len() == PLACEMENT_COUNT,
        "placed ELF section-header input does not have exactly twenty-five fixups and resolutions",
    )?;

    let mut bytes = template.bytes.clone();
    let mut applications = Vec::with_capacity(PLACEMENT_COUNT);
    for (fixup, resolution) in template
        .placement_fixups
        .iter()
        .zip(load_layout.section_header_resolutions())
    {
        let expected_kind = fixup.kind;
        require(
            fixup.row_index == resolution.row_index()
                && public_section_kind(fixup.section_kind) == resolution.section_kind()
                && fixup.byte_offset == resolution.byte_offset()
                && fixup.byte_width == resolution.byte_width()
                && expected_kind == resolution.kind(),
            "ELF section-header resolution does not match its upstream typed fixup",
        )?;
        require(
            fixup.byte_width == ELF64_PLACEMENT_FIELD_SIZE,
            "ELF section-header placement width is not eight bytes",
        )?;
        let field = field_mut(
            &mut bytes,
            fixup.byte_offset,
            fixup.byte_width,
            "section-header placement field",
        )?;
        require(
            field.iter().all(|byte| *byte == 0),
            "ELF section-header placement input is not an exact zero placeholder",
        )?;
        field.copy_from_slice(&resolution.value().to_le_bytes());
        applications.push(*resolution);
    }
    Ok(ElfPlacedSectionHeaderContents {
        bytes,
        applications,
    })
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfPlacedSectionHeaderTable, CandidateValidationError<Candidate>> {
    checked::validate_candidate(
        candidate,
        |candidate| validate_contents(&candidate.load_layout, &candidate.contents),
        |candidate| {
            candidate.non_authoritative_placed_compatibility_fingerprint != 0
                && candidate.non_authoritative_placed_compatibility_fingerprint
                    == non_authoritative_placed_compatibility_fingerprint(
                        &candidate.load_layout,
                        &candidate.contents,
                    )
        },
        "placed ELF section-header compatibility fingerprint does not replay",
        |candidate| ValidatedElfPlacedSectionHeaderTable {
            load_layout: candidate.load_layout,
            contents: candidate.contents,
            non_authoritative_placed_compatibility_fingerprint: candidate
                .non_authoritative_placed_compatibility_fingerprint,
        },
    )
}

fn validate_contents(
    load_layout: &ValidatedElfDynamicLoadLayout,
    contents: &ElfPlacedSectionHeaderContents,
) -> Result<(), Diagnostic> {
    let template = load_layout
        .relative()
        .payloads()
        .section_headers()
        .contents();
    let roster = &load_layout
        .relative()
        .payloads()
        .section_headers()
        .roster()
        .contents()
        .rows;
    let resolutions = load_layout.section_header_resolutions();
    require(
        roster.len() == SECTION_COUNT
            && template.bytes.len() == SECTION_COUNT * ELF64_SECTION_HEADER_SIZE
            && contents.bytes.len() == SECTION_COUNT * ELF64_SECTION_HEADER_SIZE,
        "placed ELF section-header table does not retain exactly fourteen complete rows",
    )?;
    require(
        template.placement_fixups.len() == PLACEMENT_COUNT
            && resolutions.len() == PLACEMENT_COUNT
            && contents.applications.len() == PLACEMENT_COUNT,
        "placed ELF section-header ledger does not cover exactly twenty-five fields",
    )?;

    for ((fixup, resolution), application) in template
        .placement_fixups
        .iter()
        .zip(resolutions)
        .zip(&contents.applications)
    {
        let expected_kind = fixup.kind;
        require(
            fixup.row_index == resolution.row_index()
                && public_section_kind(fixup.section_kind) == resolution.section_kind()
                && fixup.byte_offset == resolution.byte_offset()
                && fixup.byte_width == resolution.byte_width()
                && expected_kind == resolution.kind(),
            "retained ELF section-header resolution drifted from its typed fixup",
        )?;
        require(
            application.row_index == resolution.row_index()
                && application.section_kind == resolution.section_kind()
                && application.byte_offset == resolution.byte_offset()
                && application.byte_width == resolution.byte_width()
                && application.kind == resolution.kind()
                && application.value == resolution.value(),
            "applied ELF section-header placement drifted from its retained resolution",
        )?;
        require(
            application.byte_width == ELF64_PLACEMENT_FIELD_SIZE,
            "applied ELF section-header placement width is not eight bytes",
        )?;
        let expected_field_offset = match application.kind {
            ElfSectionPlacementResolutionKind::VirtualAddress => ELF64_SECTION_ADDRESS_OFFSET,
            ElfSectionPlacementResolutionKind::FileOffset => ELF64_SECTION_FILE_OFFSET,
        };
        let expected_offset = checked_sum(
            checked_product(
                application.row_index as usize,
                ELF64_SECTION_HEADER_SIZE,
                "placed section-header row offset",
            )?,
            expected_field_offset,
            "placed section-header field offset",
        )?;
        require(
            application.byte_offset == expected_offset,
            "applied ELF section-header placement has a noncanonical coordinate",
        )?;
        require(
            read_u64(
                &contents.bytes,
                application.byte_offset,
                "applied section-header placement value",
            )? == application.value,
            "applied ELF section-header bytes do not contain their resolved value",
        )?;
    }
    require(
        contents
            .applications
            .iter()
            .filter(|application| application.kind == ElfSectionPlacementResolutionKind::FileOffset)
            .count()
            == FILE_OFFSET_PLACEMENT_COUNT,
        "placed ELF section-header ledger does not contain exactly twelve file offsets",
    )?;
    require(
        contents
            .applications
            .iter()
            .filter(|application| {
                application.kind == ElfSectionPlacementResolutionKind::VirtualAddress
            })
            .count()
            == VIRTUAL_ADDRESS_PLACEMENT_COUNT,
        "placed ELF section-header ledger does not contain exactly eleven virtual addresses",
    )?;
    validate_application_coverage(contents.bytes.len(), &contents.applications)?;

    let decoded = decode_section_header_rows(&contents.bytes, SECTION_COUNT)?;
    let mut address_by_row = [None; SECTION_COUNT];
    let mut offset_by_row = [None; SECTION_COUNT];
    for application in &contents.applications {
        let row = usize::try_from(application.row_index)
            .map_err(|_| Diagnostic::error("ELF section-header row index exceeds usize"))?;
        let slot = match application.kind {
            ElfSectionPlacementResolutionKind::VirtualAddress => &mut address_by_row[row],
            ElfSectionPlacementResolutionKind::FileOffset => &mut offset_by_row[row],
        };
        require(
            slot.replace(application.value).is_none(),
            "applied ELF section-header ledger duplicates a row field",
        )?;
    }
    require(
        address_by_row[0].is_none()
            && offset_by_row[0].is_none()
            && address_by_row[SECTION_NAME_TABLE_INDEX].is_none()
            && offset_by_row[SECTION_NAME_TABLE_INDEX].is_some(),
        "null or section-name-table placement semantics drifted",
    )?;
    for (ordinal, (decoded, descriptor)) in decoded.iter().zip(roster).enumerate() {
        validate_decoded_row(
            decoded,
            descriptor,
            address_by_row[ordinal].unwrap_or(0),
            offset_by_row[ordinal].unwrap_or(0),
        )?;
    }
    require(
        contents.bytes[..ELF64_SECTION_HEADER_SIZE] == [0; ELF64_SECTION_HEADER_SIZE],
        "placed ELF null section header is not exactly zero",
    )
}

fn validate_decoded_row(
    decoded: &DecodedElf64SectionHeader,
    descriptor: &ElfNumericSectionDescriptor,
    expected_address: u64,
    expected_offset: u64,
) -> Result<(), Diagnostic> {
    require(
        decoded.name_offset == descriptor.name_offset
            && decoded.section_type == descriptor.section_type
            && decoded.flags == descriptor.flags
            && decoded.address == expected_address
            && decoded.file_offset == expected_offset
            && decoded.payload_size == descriptor.payload_size
            && decoded.link == descriptor.link
            && decoded.info == descriptor.info
            && decoded.alignment == descriptor.alignment
            && decoded.entry_size == descriptor.entry_size,
        "decoded placed ELF section header drifted from its roster and resolutions",
    )
}

fn validate_application_coverage(
    byte_count: usize,
    applications: &[ElfAppliedSectionHeaderPlacement],
) -> Result<(), Diagnostic> {
    for (ordinal, application) in applications.iter().enumerate() {
        let end = checked_sum(
            application.byte_offset,
            usize::from(application.byte_width),
            "applied ELF section-header placement end",
        )?;
        require(
            end <= byte_count,
            "applied ELF section-header placement exceeds the table",
        )?;
        for other in &applications[ordinal + 1..] {
            let other_end = checked_sum(
                other.byte_offset,
                usize::from(other.byte_width),
                "applied ELF section-header placement end",
            )?;
            require(
                end <= other.byte_offset || other_end <= application.byte_offset,
                "applied ELF section-header placements overlap or duplicate",
            )?;
        }
    }
    Ok(())
}

fn non_authoritative_placed_compatibility_fingerprint(
    load_layout: &ValidatedElfDynamicLoadLayout,
    contents: &ElfPlacedSectionHeaderContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf.placed-section-header-table.v1");
    hash.bytes(
        &load_layout
            .non_authoritative_layout_compatibility_fingerprint()
            .to_le_bytes(),
    );
    hash.bytes(&contents.bytes);
    hash.bytes(&(contents.applications.len() as u64).to_le_bytes());
    for application in &contents.applications {
        hash.bytes(&application.row_index.to_le_bytes());
        hash.byte(application.section_kind as u8);
        hash.bytes(&(application.byte_offset as u64).to_le_bytes());
        hash.byte(application.byte_width);
        hash.byte(application.kind as u8);
        hash.bytes(&application.value.to_le_bytes());
    }
    hash.finish()
}

#[cfg(test)]
mod tests;
