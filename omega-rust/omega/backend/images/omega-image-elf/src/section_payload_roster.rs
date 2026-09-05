//! Address-free numeric roster of exact ELF dynamic-section payloads.
//!
//! This layer joins the closed numeric section roster to the already serialized
//! payload bytes and reindexes existing procedure-linkage and `.dynamic`
//! fixups. It does not place bytes, resolve a fixup, emit program headers, or
//! mutate the image.

use crate::dynamic_linkage_templates::{
    ElfProcedureLinkageFixupKind, ElfProcedureLinkageFixupStorage,
    ElfProcedureLinkagePlacementConstraint, ElfProcedureLinkageSemanticTarget,
    ValidatedElfProcedureLinkageTemplatePlan,
};
use crate::dynamic_tag_bytes::{ElfDynamicPayloadFixupKind, ValidatedElfDynamicTablePayload};
use crate::dynamic_tags::ElfDynamicAddressTarget;
use crate::section_header_bytes::ValidatedElfSectionHeaderTableTemplate;
use crate::section_roster::ElfDynamicRosterSectionKind;
use psi_diagnostics::Diagnostic;

const SECTION_COUNT: usize = 13;
const DYNAMIC_FIXUP_COUNT: usize = 8;
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
            storage_section_index: 11,
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
    }
}

const fn procedure_target_section(target: ElfProcedureLinkageSemanticTarget) -> u32 {
    match target {
        ElfProcedureLinkageSemanticTarget::FutureDynamicSection => 11,
        ElfProcedureLinkageSemanticTarget::PltHeader
        | ElfProcedureLinkageSemanticTarget::PltEntry { .. }
        | ElfProcedureLinkageSemanticTarget::PltLazyTail { .. } => 8,
        ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { .. }
        | ElfProcedureLinkageSemanticTarget::GotPltSlot { .. } => 9,
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
        "indexed ELF payload roster must contain exactly thirteen rows",
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
        "indexed dynamic fixups do not contain exactly eight rows",
    )?;
    let storage = contents
        .rows
        .get(11)
        .ok_or_else(|| Diagnostic::error("indexed .dynamic payload is missing"))?;
    for (indexed, upstream) in contents.dynamic_fixups.iter().zip(upstream) {
        require(
            indexed.row_ordinal == upstream.row_ordinal
                && indexed.storage_section_index == 11
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

fn checked_u32(value: usize, context: &'static str) -> Result<u32, Diagnostic> {
    u32::try_from(value).map_err(|_| Diagnostic::error(format!("{context} exceeds Elf64_Word")))
}

fn checked_sum(left: usize, right: usize, context: &'static str) -> Result<usize, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
}

fn require(condition: bool, message: &'static str) -> Result<(), Diagnostic> {
    condition
        .then_some(())
        .ok_or_else(|| Diagnostic::error(message))
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
        plan_elf_dynamic_section_roster, plan_elf_dynamic_sections,
        plan_elf_dynamic_table_section_descriptor, plan_elf_dynamic_tags,
        plan_elf_procedure_linkage_relocations, plan_elf_procedure_linkage_section_descriptors,
        plan_elf_procedure_linkage_templates, plan_elf_section_name_table,
        serialize_elf_dynamic_sections, serialize_elf_dynamic_table,
        serialize_elf_section_header_table,
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
        sites: &'static [usize],
    }

    const IMPORTS: [ImportFixture; 2] = [
        ImportFixture {
            object: b"liba\xff.so",
            symbol: b"alpha\xfe",
            version: b"V1\xfd",
            sites: &[0, 32],
        },
        ImportFixture {
            object: b"libb.so",
            symbol: b"beta",
            version: b"V2",
            sites: &[16],
        },
    ];

    fn headers(
        target: TargetProfile,
        imports: &[ImportFixture],
    ) -> ValidatedElfSectionHeaderTableTemplate {
        let mut image = FinalImage::with_capacity(
            target.native_target(),
            FinalImageMemory {
                text: vec![0; 64],
                ..FinalImageMemory::default()
            },
            Handle::invalid(),
            imports.len(),
            imports.len(),
            imports.iter().map(|row| row.sites.len()).sum(),
        );
        for (index, fixture) in imports.iter().enumerate() {
            let symbol_handle = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: format!("__omega_payload_roster_import_{index}"),
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
                    .unwrap(),
                ),
            });
            for site in fixture.sites {
                let (offset, kind) = match target {
                    TargetProfile::LinuxX64 => {
                        image.memory.text[*site] = 0xe8;
                        (site + 1, RelocationKind::X86_64Relative32)
                    }
                    TargetProfile::LinuxArm64 => {
                        image.memory.text[*site..*site + 4].copy_from_slice(&[0, 0, 0, 0x94]);
                        (*site, RelocationKind::Aarch64Branch26)
                    }
                    _ => unreachable!(),
                };
                image
                    .relocation_table
                    .relocations
                    .insert(FinalImageRelocation {
                        section: FinalImageSection::Text,
                        offset,
                        byte_width: 4,
                        symbol_handle,
                        addend: 0,
                        kind,
                    });
            }
        }
        let path = match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2".as_slice(),
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1".as_slice(),
            _ => unreachable!(),
        };
        let interpreter = normalize_elf_interpreter_plan(path.to_vec(), target).unwrap();
        let inputs = plan_elf_dynamic_link_inputs(image, interpreter).unwrap();
        let sections = plan_elf_dynamic_sections(inputs).unwrap();
        let payloads = serialize_elf_dynamic_sections(sections).unwrap();
        let descriptors = plan_elf_dynamic_section_descriptors(payloads).unwrap();
        let linkage = plan_elf_procedure_linkage_relocations(descriptors).unwrap();
        let templates = plan_elf_procedure_linkage_templates(linkage).unwrap();
        let descriptors = plan_elf_procedure_linkage_section_descriptors(templates).unwrap();
        let tags = plan_elf_dynamic_tags(descriptors).unwrap();
        let dynamic = serialize_elf_dynamic_table(tags).unwrap();
        let descriptor = plan_elf_dynamic_table_section_descriptor(dynamic).unwrap();
        let names = plan_elf_section_name_table(descriptor).unwrap();
        let roster = plan_elf_dynamic_section_roster(names).unwrap();
        serialize_elf_section_header_table(roster).unwrap()
    }

    fn candidate(target: TargetProfile) -> Candidate {
        let section_headers = headers(target, &IMPORTS);
        let contents = derive_contents(&section_headers).unwrap();
        let non_authoritative_payload_roster_compatibility_fingerprint =
            non_authoritative_payload_roster_compatibility_fingerprint(&section_headers, &contents);
        Candidate {
            section_headers,
            contents,
            non_authoritative_payload_roster_compatibility_fingerprint,
        }
    }

    #[test]
    fn both_targets_join_exact_thirteen_payloads_and_indexed_fixups() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let plan = plan_elf_indexed_section_payloads(headers(target, &IMPORTS)).unwrap();
            assert_eq!(plan.row_count(), 13);
            assert_eq!(plan.dynamic_fixup_count(), 8);
            assert!(plan.contents.rows[0].bytes.is_empty());
            assert_eq!(plan.contents.rows[7].bytes.len(), 36);
            assert_eq!(plan.contents.rows[11].bytes.len(), 256);
            assert_eq!(plan.contents.rows[12].bytes.len(), 112);
            for (index, row) in plan.contents.rows.iter().enumerate() {
                assert_eq!(row.index, index as u32);
                assert_eq!(
                    row.bytes,
                    upstream_payload(plan.section_headers(), row.kind).unwrap()
                );
                assert_eq!(
                    row.bytes.len() as u64,
                    plan.section_headers.roster().contents().rows[index].payload_size
                );
            }
            validate_contents(plan.section_headers(), &plan.contents).unwrap();
            assert_ne!(
                plan.non_authoritative_payload_roster_compatibility_fingerprint(),
                0
            );
        }
    }

    #[test]
    fn indexed_storage_targets_constraints_and_raw_bytes_are_exact() {
        let plan =
            plan_elf_indexed_section_payloads(headers(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
        assert!(
            plan.contents.rows[2]
                .bytes
                .windows(8)
                .any(|window| window == b"liba\xff.so")
        );
        assert!(
            plan.contents
                .procedure_fixups
                .iter()
                .any(|fixup| fixup.storage == ElfIndexedProcedureFixupStorage::SourceText)
        );
        for fixup in &plan.contents.procedure_fixups {
            assert_eq!(
                fixup.target_section_index,
                procedure_target_section(fixup.target)
            );
            if let ElfIndexedProcedureFixupStorage::Section { index, kind } = fixup.storage {
                assert_eq!(plan.contents.rows[index as usize].kind, kind);
            }
        }
        assert_eq!(
            plan.contents.procedure_constraints,
            procedure_templates(plan.section_headers())
                .contents()
                .constraints
        );
        assert_eq!(
            plan.contents
                .dynamic_fixups
                .iter()
                .map(|fixup| fixup.target_section_index)
                .collect::<Vec<_>>(),
            [9, 4, 7, 2, 3, 10, 5, 6]
        );
    }

    #[test]
    fn permutation_preserves_identity_and_target_remains_bound() {
        let forward =
            plan_elf_indexed_section_payloads(headers(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
        let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
        let reverse =
            plan_elf_indexed_section_payloads(headers(TargetProfile::LinuxX64, &reverse_imports))
                .unwrap();
        let arm = plan_elf_indexed_section_payloads(headers(TargetProfile::LinuxArm64, &IMPORTS))
            .unwrap();
        assert_eq!(forward.contents, reverse.contents);
        assert_eq!(
            forward.non_authoritative_payload_roster_compatibility_fingerprint(),
            reverse.non_authoritative_payload_roster_compatibility_fingerprint()
        );
        assert_ne!(
            forward.non_authoritative_payload_roster_compatibility_fingerprint(),
            arm.non_authoritative_payload_roster_compatibility_fingerprint()
        );
    }

    #[test]
    fn every_payload_byte_corruption_rejects_with_header_custody() {
        let lengths = candidate(TargetProfile::LinuxX64)
            .contents
            .rows
            .iter()
            .map(|row| row.bytes.len())
            .collect::<Vec<_>>();
        for (row, length) in lengths.into_iter().enumerate() {
            for offset in 0..length {
                let mut candidate = candidate(TargetProfile::LinuxX64);
                let identity = candidate
                    .section_headers
                    .non_authoritative_template_compatibility_fingerprint();
                candidate.contents.rows[row].bytes[offset] ^= 1;
                let error =
                    validate_candidate(candidate).expect_err("payload corruption must reject");
                assert_eq!(
                    error
                        .candidate
                        .section_headers
                        .non_authoritative_template_compatibility_fingerprint(),
                    identity
                );
            }
        }
    }

    #[test]
    fn row_fixup_constraint_and_identity_corruption_reject_recoverably() {
        let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
            Box::new(|c| {
                c.contents.rows.pop();
            }),
            Box::new(|c| c.contents.rows.push(c.contents.rows[0].clone())),
            Box::new(|c| c.contents.rows.swap(1, 2)),
            Box::new(|c| c.contents.rows[1].index = u32::MAX),
            Box::new(|c| c.contents.rows[1].kind = ElfDynamicRosterSectionKind::DynamicString),
            Box::new(|c| {
                c.contents.procedure_fixups.pop();
            }),
            Box::new(|c| c.contents.procedure_fixups[0].byte_offset = usize::MAX),
            Box::new(|c| c.contents.procedure_fixups[0].byte_width = 3),
            Box::new(|c| c.contents.procedure_fixups[0].mutable_mask ^= 1),
            Box::new(|c| c.contents.procedure_fixups[0].target_section_index ^= 1),
            Box::new(|c| {
                c.contents.procedure_constraints.pop();
            }),
            Box::new(|c| {
                c.contents.dynamic_fixups.pop();
            }),
            Box::new(|c| c.contents.dynamic_fixups[0].storage_section_index = 9),
            Box::new(|c| c.contents.dynamic_fixups[0].target_section_index ^= 1),
            Box::new(|c| c.non_authoritative_payload_roster_compatibility_fingerprint ^= 1),
        ];
        for corrupt in corruptions {
            let mut candidate = candidate(TargetProfile::LinuxArm64);
            let identity = candidate
                .section_headers
                .non_authoritative_template_compatibility_fingerprint();
            corrupt(&mut candidate);
            let error =
                validate_candidate(candidate).expect_err("corrupt indexed payloads must reject");
            assert_eq!(
                error
                    .candidate
                    .section_headers
                    .non_authoritative_template_compatibility_fingerprint(),
                identity
            );
        }
    }

    #[test]
    fn malformed_bounds_widths_and_arithmetic_reject_without_panicking() {
        assert!(checked_sum(usize::MAX, 8, "sum").is_err());
        assert!(checked_u32(usize::MAX, "word").is_err());
        assert!(read_field(&[], usize::MAX, 8).is_err());
        assert!(read_field(&[0; 3], 0, 4).is_err());
        assert!(read_field(&[0; 8], 0, 3).is_err());
    }
}
