//! Address-free numeric roster of exact ELF dynamic-section payloads.
//!
//! This layer joins the closed numeric section roster to the already serialized
//! payload bytes and reindexes existing procedure-linkage and `.dynamic`
//! fixups. It does not place bytes, resolve a fixup, emit program headers, or
//! mutate the image.

use crate::dynamic_executable::checked::{checked_sum, checked_u32, require};
use crate::dynamic_executable::dynamic_table::dynamic_tag_bytes::{
    ElfDynamicPayloadFixupKind, ValidatedElfDynamicTablePayload,
};
use crate::dynamic_executable::dynamic_table::dynamic_tags::ElfDynamicAddressTarget;
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::{
    ElfProcedureLinkageFixupKind, ElfProcedureLinkageFixupStorage,
    ElfProcedureLinkagePlacementConstraint, ElfProcedureLinkageSemanticTarget,
    ValidatedElfProcedureLinkageTemplatePlan,
};
use crate::dynamic_executable::section_headers::section_header_bytes::ValidatedElfSectionHeaderTableTemplate;
use crate::dynamic_executable::section_headers::section_roster::ElfDynamicRosterSectionKind;
use diagnostics::Diagnostic;

const SECTION_COUNT: usize = 14;
const DYNAMIC_FIXUP_COUNT: usize = 9;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Independently replayed numeric payload rows and indexed unresolved fixups.
///
/// The exact section-header template remains owned by this non-clone carrier.
/// Numeric association grants no placement, resolved address, image mutation,
/// publication, or runnable-image authority.
#[derive(Debug)]
#[must_use = "validated indexed ELF payloads retain the section-header template"]
pub struct ValidatedElfIndexedSectionPayloadPlan {
    section_headers: ValidatedElfSectionHeaderTableTemplate,
    contents: ElfIndexedSectionPayloadContents,
    non_authoritative_payload_roster_compatibility_fingerprint: u64,
}

impl ValidatedElfIndexedSectionPayloadPlan {
    pub const fn section_headers(&self) -> &ValidatedElfSectionHeaderTableTemplate {
        &self.section_headers
    }

    pub fn row_count(&self) -> usize {
        self.contents.rows.len()
    }

    pub fn payload_byte_count(&self) -> usize {
        self.contents.rows.iter().map(|row| row.bytes.len()).sum()
    }

    pub fn procedure_fixup_count(&self) -> usize {
        self.contents.procedure_fixups.len()
    }

    pub fn dynamic_fixup_count(&self) -> usize {
        self.contents.dynamic_fixups.len()
    }

    pub const fn non_authoritative_payload_roster_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_payload_roster_compatibility_fingerprint
    }

    #[allow(dead_code)]
    pub(crate) const fn contents(&self) -> &ElfIndexedSectionPayloadContents {
        &self.contents
    }

    #[allow(dead_code)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidatedElfSectionHeaderTableTemplate,
        ElfIndexedSectionPayloadContents,
    ) {
        (self.section_headers, self.contents)
    }
}

/// Rejected indexed-payload planning with exact header-template custody.
#[derive(Debug)]
#[must_use = "ELF indexed-payload rejection retains the section-header template"]
pub struct ElfIndexedSectionPayloadPlanningError {
    section_headers: ValidatedElfSectionHeaderTableTemplate,
    diagnostic: Diagnostic,
}

impl ElfIndexedSectionPayloadPlanningError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfSectionHeaderTableTemplate, Diagnostic) {
        (self.section_headers, self.diagnostic)
    }
}

impl std::fmt::Display for ElfIndexedSectionPayloadPlanningError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfIndexedSectionPayloadPlanningError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfIndexedSectionPayloadContents {
    pub(crate) rows: Vec<ElfIndexedSectionPayloadRow>,
    pub(crate) procedure_fixups: Vec<ElfIndexedProcedureFixup>,
    pub(crate) procedure_constraints: Vec<ElfProcedureLinkagePlacementConstraint>,
    pub(crate) dynamic_fixups: Vec<ElfIndexedDynamicFixup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfIndexedSectionPayloadRow {
    pub(crate) index: u32,
    pub(crate) kind: ElfDynamicRosterSectionKind,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ElfIndexedProcedureFixupStorage {
    SourceText,
    Section {
        index: u32,
        kind: ElfDynamicRosterSectionKind,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfIndexedProcedureFixup {
    pub(crate) upstream_ordinal: u32,
    pub(crate) storage: ElfIndexedProcedureFixupStorage,
    pub(crate) byte_offset: usize,
    pub(crate) byte_width: u8,
    pub(crate) mutable_mask: u64,
    pub(crate) kind: ElfProcedureLinkageFixupKind,
    pub(crate) target: ElfProcedureLinkageSemanticTarget,
    pub(crate) target_section_index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfIndexedDynamicFixup {
    pub(crate) row_ordinal: u32,
    pub(crate) storage_section_index: u32,
    pub(crate) byte_offset: usize,
    pub(crate) byte_width: u8,
    pub(crate) kind: ElfDynamicPayloadFixupKind,
    pub(crate) target: ElfDynamicAddressTarget,
    pub(crate) target_section_index: u32,
}

struct Candidate {
    section_headers: ValidatedElfSectionHeaderTableTemplate,
    contents: ElfIndexedSectionPayloadContents,
    non_authoritative_payload_roster_compatibility_fingerprint: u64,
}

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

/// Consume exact section-header templates into a numeric byte roster with
/// indexed but unresolved payload and source-text fixups.
pub fn plan_elf_indexed_section_payloads(
    section_headers: ValidatedElfSectionHeaderTableTemplate,
) -> Result<ValidatedElfIndexedSectionPayloadPlan, Box<ElfIndexedSectionPayloadPlanningError>> {
    let contents = match derive_contents(&section_headers) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(ElfIndexedSectionPayloadPlanningError {
                section_headers,
                diagnostic,
            }));
        }
    };
    let non_authoritative_payload_roster_compatibility_fingerprint =
        non_authoritative_payload_roster_compatibility_fingerprint(&section_headers, &contents);
    let candidate = Candidate {
        section_headers,
        contents,
        non_authoritative_payload_roster_compatibility_fingerprint,
    };
    match validate_candidate(candidate) {
        Ok(validated) => Ok(validated),
        Err(error) => Err(Box::new(ElfIndexedSectionPayloadPlanningError {
            section_headers: error.candidate.section_headers,
            diagnostic: error.diagnostic,
        })),
    }
}

fn derive_contents(
    section_headers: &ValidatedElfSectionHeaderTableTemplate,
) -> Result<ElfIndexedSectionPayloadContents, Diagnostic> {
    let roster = &section_headers.roster().contents().rows;
    let mut rows = Vec::with_capacity(SECTION_COUNT);
    for descriptor in roster {
        rows.push(ElfIndexedSectionPayloadRow {
            index: descriptor.index,
            kind: descriptor.kind,
            bytes: upstream_payload(section_headers, descriptor.kind)?.to_vec(),
        });
    }

    let templates = procedure_templates(section_headers);
    let procedure_fixups = templates
        .contents()
        .fixups
        .iter()
        .enumerate()
        .map(|(ordinal, fixup)| {
            Ok(ElfIndexedProcedureFixup {
                upstream_ordinal: checked_u32(ordinal, "procedure fixup ordinal")?,
                storage: indexed_procedure_storage(fixup.storage),
                byte_offset: fixup.byte_offset,
                byte_width: fixup.byte_width,
                mutable_mask: fixup.mutable_mask,
                kind: fixup.kind,
                target: fixup.target,
                target_section_index: procedure_target_section(fixup.target),
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let dynamic_fixups = dynamic_payload(section_headers)
        .contents()
        .address_fixups
        .iter()
        .map(|fixup| ElfIndexedDynamicFixup {
            row_ordinal: fixup.row_ordinal,
            storage_section_index: 12,
            byte_offset: fixup.byte_offset,
            byte_width: fixup.byte_width,
            kind: fixup.kind,
            target: fixup.target,
            target_section_index: dynamic_target_section(fixup.target),
        })
        .collect();
    Ok(ElfIndexedSectionPayloadContents {
        rows,
        procedure_fixups,
        procedure_constraints: templates.contents().constraints.clone(),
        dynamic_fixups,
    })
}

fn procedure_templates(
    section_headers: &ValidatedElfSectionHeaderTableTemplate,
) -> &ValidatedElfProcedureLinkageTemplatePlan {
    section_headers
        .roster()
        .section_names()
        .dynamic_table()
        .payload()
        .plan()
        .descriptors()
        .templates()
}

fn dynamic_payload(
    section_headers: &ValidatedElfSectionHeaderTableTemplate,
) -> &ValidatedElfDynamicTablePayload {
    section_headers
        .roster()
        .section_names()
        .dynamic_table()
        .payload()
}

fn upstream_payload(
    section_headers: &ValidatedElfSectionHeaderTableTemplate,
    kind: ElfDynamicRosterSectionKind,
) -> Result<&[u8], Diagnostic> {
    let templates = procedure_templates(section_headers);
    let base = templates.linkage().descriptors().payloads().payloads();
    let bytes = &templates.contents().bytes;
    Ok(match kind {
        ElfDynamicRosterSectionKind::Null => &[],
        ElfDynamicRosterSectionKind::Interpreter => &base.interpreter,
        ElfDynamicRosterSectionKind::DynamicString => &base.dynstr,
        ElfDynamicRosterSectionKind::DynamicSymbol => &base.dynsym,
        ElfDynamicRosterSectionKind::SystemVHash => &base.sysv_hash,
        ElfDynamicRosterSectionKind::GnuHash => &base.gnu_hash,
        ElfDynamicRosterSectionKind::GnuSymbolVersion => &base.versym,
        ElfDynamicRosterSectionKind::GnuVersionRequirement => &base.verneed,
        ElfDynamicRosterSectionKind::ProcedureLinkage => &bytes.plt,
        ElfDynamicRosterSectionKind::ProcedureGot => &bytes.got_plt,
        ElfDynamicRosterSectionKind::ProcedureRelocation => &bytes.rela_plt,
        ElfDynamicRosterSectionKind::GeneralRelocation => &bytes.rela_dyn,
        ElfDynamicRosterSectionKind::DynamicTable => {
            &dynamic_payload(section_headers).contents().bytes
        }
        ElfDynamicRosterSectionKind::SectionNameTable => {
            &section_headers.roster().section_names().contents().bytes
        }
    })
}

const fn indexed_procedure_storage(
    storage: ElfProcedureLinkageFixupStorage,
) -> ElfIndexedProcedureFixupStorage {
    match storage {
        ElfProcedureLinkageFixupStorage::SourceText => ElfIndexedProcedureFixupStorage::SourceText,
        ElfProcedureLinkageFixupStorage::Plt => ElfIndexedProcedureFixupStorage::Section {
            index: 8,
            kind: ElfDynamicRosterSectionKind::ProcedureLinkage,
        },
        ElfProcedureLinkageFixupStorage::GotPlt => ElfIndexedProcedureFixupStorage::Section {
            index: 9,
            kind: ElfDynamicRosterSectionKind::ProcedureGot,
        },
        ElfProcedureLinkageFixupStorage::RelaPlt => ElfIndexedProcedureFixupStorage::Section {
            index: 10,
            kind: ElfDynamicRosterSectionKind::ProcedureRelocation,
        },
        ElfProcedureLinkageFixupStorage::RelaDyn => ElfIndexedProcedureFixupStorage::Section {
            index: 11,
            kind: ElfDynamicRosterSectionKind::GeneralRelocation,
        },
    }
}

const fn procedure_target_section(target: ElfProcedureLinkageSemanticTarget) -> u32 {
    match target {
        ElfProcedureLinkageSemanticTarget::FutureDynamicSection => 12,
        ElfProcedureLinkageSemanticTarget::PltHeader
        | ElfProcedureLinkageSemanticTarget::PltEntry { .. }
        | ElfProcedureLinkageSemanticTarget::PltLazyTail { .. } => 8,
        ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { .. }
        | ElfProcedureLinkageSemanticTarget::GotPltSlot { .. } => 9,
        // Retained source sections are not numbered roster rows; the applied
        // layout resolves them through the image-memory placement instead.
        ElfProcedureLinkageSemanticTarget::RelocatedImageSection { .. } => u32::MAX,
    }
}

const fn dynamic_target_section(target: ElfDynamicAddressTarget) -> u32 {
    match target {
        ElfDynamicAddressTarget::ProcedureGot => 9,
        ElfDynamicAddressTarget::SystemVHash => 4,
        ElfDynamicAddressTarget::GnuHash => 7,
        ElfDynamicAddressTarget::DynamicString => 2,
        ElfDynamicAddressTarget::DynamicSymbol => 3,
        ElfDynamicAddressTarget::ProcedureRelocation => 10,
        ElfDynamicAddressTarget::GnuSymbolVersion => 5,
        ElfDynamicAddressTarget::GnuVersionRequirement => 6,
        ElfDynamicAddressTarget::GeneralRelocation => 11,
    }
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfIndexedSectionPayloadPlan, CandidateValidationError> {
    if let Err(diagnostic) = validate_contents(&candidate.section_headers, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    if candidate.non_authoritative_payload_roster_compatibility_fingerprint
        != non_authoritative_payload_roster_compatibility_fingerprint(
            &candidate.section_headers,
            &candidate.contents,
        )
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error(
                "ELF indexed payload-roster compatibility fingerprint does not replay",
            ),
        });
    }
    Ok(ValidatedElfIndexedSectionPayloadPlan {
        section_headers: candidate.section_headers,
        contents: candidate.contents,
        non_authoritative_payload_roster_compatibility_fingerprint: candidate
            .non_authoritative_payload_roster_compatibility_fingerprint,
    })
}

fn validate_contents(
    section_headers: &ValidatedElfSectionHeaderTableTemplate,
    contents: &ElfIndexedSectionPayloadContents,
) -> Result<(), Diagnostic> {
    let roster = &section_headers.roster().contents().rows;
    require(
        roster.len() == SECTION_COUNT && contents.rows.len() == SECTION_COUNT,
        "indexed ELF payload roster must contain exactly fourteen rows",
    )?;
    for (ordinal, (row, descriptor)) in contents.rows.iter().zip(roster).enumerate() {
        require(
            row.index == checked_u32(ordinal, "indexed payload ordinal")?
                && row.index == descriptor.index
                && row.kind == descriptor.kind
                && row.bytes == upstream_payload(section_headers, row.kind)?
                && u64::try_from(row.bytes.len()).ok() == Some(descriptor.payload_size),
            "indexed ELF payload row drifted from its roster or upstream bytes",
        )?;
    }
    validate_procedure_fixups(section_headers, contents)?;
    validate_dynamic_fixups(section_headers, contents)
}

fn validate_procedure_fixups(
    section_headers: &ValidatedElfSectionHeaderTableTemplate,
    contents: &ElfIndexedSectionPayloadContents,
) -> Result<(), Diagnostic> {
    let templates = procedure_templates(section_headers);
    require(
        contents.procedure_fixups.len() == templates.contents().fixups.len()
            && contents.procedure_constraints == templates.contents().constraints,
        "indexed procedure fixups or constraints do not exactly cover the upstream templates",
    )?;
    for (ordinal, (indexed, upstream)) in contents
        .procedure_fixups
        .iter()
        .zip(&templates.contents().fixups)
        .enumerate()
    {
        require(
            indexed.upstream_ordinal == checked_u32(ordinal, "replayed procedure fixup")?
                && indexed.storage == indexed_procedure_storage(upstream.storage)
                && indexed.byte_offset == upstream.byte_offset
                && indexed.byte_width == upstream.byte_width
                && indexed.mutable_mask == upstream.mutable_mask
                && indexed.kind == upstream.kind
                && indexed.target == upstream.target
                && indexed.target_section_index == procedure_target_section(upstream.target),
            "indexed procedure fixup drifted from the upstream template",
        )?;
        let storage = procedure_storage_bytes(section_headers, contents, indexed.storage)?;
        let field = read_field(storage, indexed.byte_offset, indexed.byte_width)?;
        require(
            field & indexed.mutable_mask == 0,
            "indexed procedure fixup does not select an exact zero placeholder",
        )?;
    }
    for constraint in &contents.procedure_constraints {
        require(
            (constraint.fixup_ordinal as usize) < contents.procedure_fixups.len(),
            "indexed procedure constraint references a missing fixup",
        )?;
    }
    Ok(())
}

fn procedure_storage_bytes<'a>(
    section_headers: &'a ValidatedElfSectionHeaderTableTemplate,
    contents: &'a ElfIndexedSectionPayloadContents,
    storage: ElfIndexedProcedureFixupStorage,
) -> Result<&'a [u8], Diagnostic> {
    match storage {
        ElfIndexedProcedureFixupStorage::SourceText => Ok(&procedure_templates(section_headers)
            .linkage()
            .descriptors()
            .payloads()
            .plan()
            .inputs()
            .image()
            .memory
            .text),
        ElfIndexedProcedureFixupStorage::Section { index, kind } => contents
            .rows
            .get(index as usize)
            .filter(|row| row.kind == kind)
            .map(|row| row.bytes.as_slice())
            .ok_or_else(|| Diagnostic::error("indexed procedure-fixup storage is missing")),
    }
}

fn validate_dynamic_fixups(
    section_headers: &ValidatedElfSectionHeaderTableTemplate,
    contents: &ElfIndexedSectionPayloadContents,
) -> Result<(), Diagnostic> {
    let upstream = &dynamic_payload(section_headers).contents().address_fixups;
    require(
        upstream.len() == DYNAMIC_FIXUP_COUNT && contents.dynamic_fixups.len() == upstream.len(),
        "indexed dynamic fixups do not contain exactly nine rows",
    )?;
    let storage = contents
        .rows
        .get(12)
        .ok_or_else(|| Diagnostic::error("indexed .dynamic payload is missing"))?;
    for (indexed, upstream) in contents.dynamic_fixups.iter().zip(upstream) {
        require(
            indexed.row_ordinal == upstream.row_ordinal
                && indexed.storage_section_index == 12
                && indexed.byte_offset == upstream.byte_offset
                && indexed.byte_width == upstream.byte_width
                && indexed.kind == upstream.kind
                && indexed.target == upstream.target
                && indexed.target_section_index == dynamic_target_section(upstream.target),
            "indexed dynamic fixup drifted from the serialized .dynamic payload",
        )?;
        require(
            read_field(&storage.bytes, indexed.byte_offset, indexed.byte_width)? == 0,
            "indexed dynamic fixup does not select an exact zero placeholder",
        )?;
    }
    Ok(())
}

fn read_field(bytes: &[u8], offset: usize, width: u8) -> Result<u64, Diagnostic> {
    let end = checked_sum(offset, usize::from(width), "indexed fixup end")?;
    let field = bytes
        .get(offset..end)
        .ok_or_else(|| Diagnostic::error("indexed fixup exceeds its storage"))?;
    match width {
        4 => Ok(u64::from(u32::from_le_bytes(field.try_into().map_err(
            |_| Diagnostic::error("invalid four-byte indexed fixup"),
        )?))),
        8 => Ok(u64::from_le_bytes(field.try_into().map_err(|_| {
            Diagnostic::error("invalid eight-byte indexed fixup")
        })?)),
        _ => Err(Diagnostic::error("unsupported indexed fixup width")),
    }
}

fn non_authoritative_payload_roster_compatibility_fingerprint(
    section_headers: &ValidatedElfSectionHeaderTableTemplate,
    contents: &ElfIndexedSectionPayloadContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-indexed-section-payloads.v1");
    hash.bytes(
        &section_headers
            .non_authoritative_template_compatibility_fingerprint()
            .to_le_bytes(),
    );
    for row in &contents.rows {
        hash.bytes(&row.index.to_le_bytes());
        hash.byte(row.kind as u8);
        hash.bytes(&row.bytes);
    }
    for fixup in &contents.procedure_fixups {
        hash.bytes(&fixup.upstream_ordinal.to_le_bytes());
        hash_procedure_storage(&mut hash, fixup.storage);
        hash.bytes(&(fixup.byte_offset as u64).to_le_bytes());
        hash.byte(fixup.byte_width);
        hash.bytes(&fixup.mutable_mask.to_le_bytes());
        hash.byte(fixup.kind as u8);
        hash_procedure_target(&mut hash, fixup.target);
        hash.bytes(&fixup.target_section_index.to_le_bytes());
    }
    for constraint in &contents.procedure_constraints {
        hash.bytes(&constraint.fixup_ordinal.to_le_bytes());
        hash.byte(constraint.kind as u8);
    }
    for fixup in &contents.dynamic_fixups {
        hash.bytes(&fixup.row_ordinal.to_le_bytes());
        hash.bytes(&fixup.storage_section_index.to_le_bytes());
        hash.bytes(&(fixup.byte_offset as u64).to_le_bytes());
        hash.byte(fixup.byte_width);
        hash.byte(fixup.kind as u8);
        hash.byte(fixup.target as u8);
        hash.bytes(&fixup.target_section_index.to_le_bytes());
    }
    hash.finish()
}

fn hash_procedure_storage(hash: &mut Fnv1a, storage: ElfIndexedProcedureFixupStorage) {
    match storage {
        ElfIndexedProcedureFixupStorage::SourceText => hash.byte(0),
        ElfIndexedProcedureFixupStorage::Section { index, kind } => {
            hash.byte(1);
            hash.bytes(&index.to_le_bytes());
            hash.byte(kind as u8);
        }
    }
}

fn hash_procedure_target(hash: &mut Fnv1a, target: ElfProcedureLinkageSemanticTarget) {
    match target {
        ElfProcedureLinkageSemanticTarget::FutureDynamicSection => hash.byte(0),
        ElfProcedureLinkageSemanticTarget::PltHeader => hash.byte(1),
        ElfProcedureLinkageSemanticTarget::PltEntry { logical_ordinal } => {
            hash.byte(2);
            hash.bytes(&logical_ordinal.to_le_bytes());
        }
        ElfProcedureLinkageSemanticTarget::PltLazyTail { logical_ordinal } => {
            hash.byte(3);
            hash.bytes(&logical_ordinal.to_le_bytes());
        }
        ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index } => {
            hash.byte(4);
            hash.byte(word_index);
        }
        ElfProcedureLinkageSemanticTarget::GotPltSlot { logical_ordinal } => {
            hash.byte(5);
            hash.bytes(&logical_ordinal.to_le_bytes());
        }
        ElfProcedureLinkageSemanticTarget::RelocatedImageSection {
            section,
            byte_offset,
        } => {
            hash.byte(6);
            hash.byte(image_section_tag(section));
            hash.bytes(&(byte_offset as u64).to_le_bytes());
        }
    }
}

const fn image_section_tag(section: image::FinalImageSection) -> u8 {
    match section {
        image::FinalImageSection::Text => 1,
        image::FinalImageSection::Data => 2,
        image::FinalImageSection::Bss => 3,
        image::FinalImageSection::None => 4,
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
