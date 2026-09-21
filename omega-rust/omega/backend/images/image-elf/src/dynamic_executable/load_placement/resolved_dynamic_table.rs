//! Applied internal address fixups for the ELF64-LSB `.dynamic` payload.
//!
//! This layer consumes the placed section-header owner, copies the exact
//! indexed `.dynamic` bytes, and resolves only the eight retained address
//! obligations against their exact allocated section placements. It does not
//! modify any upstream bytes or the retained `FinalImage`, resolve procedure
//! or source relocations, serialize ELF/program headers, or
//! claim loader or runnable-image authority.

use crate::dynamic_executable::checked::{checked_product, checked_sum, read_u64, require};
use crate::dynamic_executable::dynamic_table::dynamic_tag_bytes::{
    ElfDynamicPayloadFixupKind, ValidatedElfDynamicTablePayload,
};
use crate::dynamic_executable::dynamic_table::dynamic_tags::{
    ElfDynamicAddressTarget, ElfDynamicTag, ElfDynamicValue, ValidatedElfDynamicTagPlan,
};
use crate::dynamic_executable::load_placement::load_layout::{
    ElfPlacedDynamicSectionKind, ValidatedElfDynamicLoadLayout,
};
use crate::dynamic_executable::load_placement::placed_section_headers::ValidatedElfPlacedSectionHeaderTable;
use crate::dynamic_executable::section_headers::section_payload_roster::{
    ElfIndexedDynamicFixup, ValidatedElfIndexedSectionPayloadPlan,
};
use crate::dynamic_executable::section_headers::section_roster::ElfDynamicRosterSectionKind;
use diagnostics::Diagnostic;

const ELF64_DYNAMIC_ROW_SIZE: usize = 16;
const ELF64_DYNAMIC_VALUE_OFFSET: usize = 8;
const ELF64_DYNAMIC_VALUE_SIZE: u8 = 8;
const DYNAMIC_SECTION_INDEX: u32 = 12;
const DYNAMIC_FIXUP_COUNT: usize = 9;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

const EXPECTED_TARGETS: [ElfDynamicAddressApplicationTarget; DYNAMIC_FIXUP_COUNT] = [
    ElfDynamicAddressApplicationTarget::ProcedureGot,
    ElfDynamicAddressApplicationTarget::SystemVHash,
    ElfDynamicAddressApplicationTarget::GnuHash,
    ElfDynamicAddressApplicationTarget::DynamicString,
    ElfDynamicAddressApplicationTarget::DynamicSymbol,
    ElfDynamicAddressApplicationTarget::ProcedureRelocation,
    ElfDynamicAddressApplicationTarget::GeneralRelocation,
    ElfDynamicAddressApplicationTarget::GnuSymbolVersion,
    ElfDynamicAddressApplicationTarget::GnuVersionRequirement,
];

/// The only field encoding admitted by this `.dynamic` application rung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ElfDynamicAddressApplicationKind {
    Elf64AbsoluteAddress = 1,
}

/// Semantic section target of one applied `.dynamic` address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ElfDynamicAddressApplicationTarget {
    ProcedureGot = 1,
    SystemVHash = 2,
    DynamicString = 3,
    DynamicSymbol = 4,
    ProcedureRelocation = 5,
    GnuSymbolVersion = 6,
    GnuVersionRequirement = 7,
    GnuHash = 8,
    GeneralRelocation = 9,
}

/// One exact address application retained beside the resulting `.dynamic`
/// bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElfAppliedDynamicAddress {
    row_ordinal: u32,
    storage_section_index: u32,
    byte_offset: usize,
    byte_width: u8,
    kind_tag: u8,
    target: ElfDynamicAddressApplicationTarget,
    target_section_index: u32,
    target_section_kind: ElfPlacedDynamicSectionKind,
    value: u64,
}

impl ElfAppliedDynamicAddress {
    pub const fn row_ordinal(&self) -> u32 {
        self.row_ordinal
    }

    pub const fn storage_section_index(&self) -> u32 {
        self.storage_section_index
    }

    pub const fn byte_offset(&self) -> usize {
        self.byte_offset
    }

    pub const fn byte_width(&self) -> u8 {
        self.byte_width
    }

    pub const fn kind(&self) -> ElfDynamicAddressApplicationKind {
        ElfDynamicAddressApplicationKind::Elf64AbsoluteAddress
    }

    pub const fn target(&self) -> ElfDynamicAddressApplicationTarget {
        self.target
    }

    pub const fn target_section_index(&self) -> u32 {
        self.target_section_index
    }

    pub const fn target_section_kind(&self) -> ElfPlacedDynamicSectionKind {
        self.target_section_kind
    }

    pub const fn value(&self) -> u64 {
        self.value
    }
}

/// Independently replayed `.dynamic` payload with all nine internal address
/// fields resolved.
///
/// This non-clone carrier retains the complete placed-section-header and load-
/// layout custody chain. Its bytes have no image-mutation, header-emission,
/// loader, publication, or runnable-image authority.
#[derive(Debug)]
#[must_use = "resolved ELF dynamic bytes retain placed-section and load-layout custody"]
pub struct ValidatedElfResolvedDynamicTable {
    placed_section_headers: ValidatedElfPlacedSectionHeaderTable,
    contents: ElfResolvedDynamicTableContents,
    non_authoritative_resolved_compatibility_fingerprint: u64,
}

impl ValidatedElfResolvedDynamicTable {
    pub const fn placed_section_headers(&self) -> &ValidatedElfPlacedSectionHeaderTable {
        &self.placed_section_headers
    }

    pub fn bytes(&self) -> &[u8] {
        &self.contents.bytes
    }

    pub fn applied_addresses(&self) -> &[ElfAppliedDynamicAddress] {
        &self.contents.applications
    }

    pub const fn non_authoritative_resolved_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_resolved_compatibility_fingerprint
    }

    #[allow(dead_code)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidatedElfPlacedSectionHeaderTable,
        ElfResolvedDynamicTableContents,
    ) {
        (self.placed_section_headers, self.contents)
    }
}

/// Rejected internal `.dynamic` resolution with exact upstream custody.
#[derive(Debug)]
#[must_use = "dynamic-address rejection retains the placed section-header owner"]
pub struct ElfDynamicAddressApplicationError {
    placed_section_headers: ValidatedElfPlacedSectionHeaderTable,
    diagnostic: Diagnostic,
}

impl ElfDynamicAddressApplicationError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfPlacedSectionHeaderTable, Diagnostic) {
        (self.placed_section_headers, self.diagnostic)
    }
}

impl std::fmt::Display for ElfDynamicAddressApplicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfDynamicAddressApplicationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfResolvedDynamicTableContents {
    pub(crate) bytes: Vec<u8>,
    pub(crate) applications: Vec<ElfAppliedDynamicAddress>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DecodedElfDynamicRow {
    tag: i64,
    value: u64,
}

struct Candidate {
    placed_section_headers: ValidatedElfPlacedSectionHeaderTable,
    contents: ElfResolvedDynamicTableContents,
    non_authoritative_resolved_compatibility_fingerprint: u64,
}

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

/// Apply the exact eight indexed `.dynamic` address fixups from the retained
/// absolute section layout.
pub fn apply_elf_dynamic_address_fixups(
    placed_section_headers: ValidatedElfPlacedSectionHeaderTable,
) -> Result<ValidatedElfResolvedDynamicTable, Box<ElfDynamicAddressApplicationError>> {
    let contents = match derive_contents(&placed_section_headers) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(ElfDynamicAddressApplicationError {
                placed_section_headers,
                diagnostic,
            }));
        }
    };
    let non_authoritative_resolved_compatibility_fingerprint =
        non_authoritative_resolved_compatibility_fingerprint(&placed_section_headers, &contents);
    let candidate = Candidate {
        placed_section_headers,
        contents,
        non_authoritative_resolved_compatibility_fingerprint,
    };
    validate_candidate(candidate).map_err(|error| {
        Box::new(ElfDynamicAddressApplicationError {
            placed_section_headers: error.candidate.placed_section_headers,
            diagnostic: error.diagnostic,
        })
    })
}

fn derive_contents(
    placed: &ValidatedElfPlacedSectionHeaderTable,
) -> Result<ElfResolvedDynamicTableContents, Diagnostic> {
    let indexed = indexed_payloads(placed);
    let source = dynamic_row(indexed)?;
    require(
        indexed.contents().dynamic_fixups.len() == DYNAMIC_FIXUP_COUNT,
        "internal ELF dynamic application requires exactly eight indexed fixups",
    )?;
    let mut bytes = source.bytes.clone();
    let mut applications = Vec::with_capacity(DYNAMIC_FIXUP_COUNT);
    for (fixup, expected_target) in indexed
        .contents()
        .dynamic_fixups
        .iter()
        .zip(EXPECTED_TARGETS)
    {
        let application = application_for_fixup(placed.load_layout(), fixup)?;
        require(
            application.target == expected_target,
            "indexed ELF dynamic address targets are not in canonical order",
        )?;
        let field = field_mut(&mut bytes, application.byte_offset, application.byte_width)?;
        require(
            field.iter().all(|byte| *byte == 0),
            "indexed ELF dynamic address is not an exact zero placeholder",
        )?;
        field.copy_from_slice(&application.value.to_le_bytes());
        applications.push(application);
    }
    Ok(ElfResolvedDynamicTableContents {
        bytes,
        applications,
    })
}

fn application_for_fixup(
    load_layout: &ValidatedElfDynamicLoadLayout,
    fixup: &ElfIndexedDynamicFixup,
) -> Result<ElfAppliedDynamicAddress, Diagnostic> {
    require(
        fixup.storage_section_index == DYNAMIC_SECTION_INDEX,
        "indexed ELF dynamic fixup names the wrong storage section",
    )?;
    require(
        fixup.byte_width == ELF64_DYNAMIC_VALUE_SIZE
            && fixup.kind == ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
        "indexed ELF dynamic fixup is not an eight-byte absolute address",
    )?;
    let target = public_target(fixup.target);
    let target_kind = target_section_kind(target);
    let expected_index = target_section_index(target);
    require(
        fixup.target_section_index == expected_index,
        "indexed ELF dynamic fixup names the wrong target section index",
    )?;
    let section = load_layout
        .sections()
        .get(expected_index as usize)
        .filter(|section| {
            section.index() == expected_index
                && section.kind() == target_kind
                && section.virtual_address().is_some()
        })
        .ok_or_else(|| {
            Diagnostic::error("indexed ELF dynamic target has no exact allocated placement")
        })?;
    Ok(ElfAppliedDynamicAddress {
        row_ordinal: fixup.row_ordinal,
        storage_section_index: fixup.storage_section_index,
        byte_offset: fixup.byte_offset,
        byte_width: fixup.byte_width,
        kind_tag: fixup.kind as u8,
        target,
        target_section_index: fixup.target_section_index,
        target_section_kind: target_kind,
        value: section
            .virtual_address()
            .expect("filtered allocated ELF dynamic target"),
    })
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfResolvedDynamicTable, CandidateValidationError> {
    if let Err(diagnostic) =
        validate_contents(&candidate.placed_section_headers, &candidate.contents)
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    let expected_identity = non_authoritative_resolved_compatibility_fingerprint(
        &candidate.placed_section_headers,
        &candidate.contents,
    );
    if candidate.non_authoritative_resolved_compatibility_fingerprint == 0
        || candidate.non_authoritative_resolved_compatibility_fingerprint != expected_identity
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error(
                "resolved ELF dynamic-table compatibility fingerprint does not replay",
            ),
        });
    }
    Ok(ValidatedElfResolvedDynamicTable {
        placed_section_headers: candidate.placed_section_headers,
        contents: candidate.contents,
        non_authoritative_resolved_compatibility_fingerprint: candidate
            .non_authoritative_resolved_compatibility_fingerprint,
    })
}

fn validate_contents(
    placed: &ValidatedElfPlacedSectionHeaderTable,
    contents: &ElfResolvedDynamicTableContents,
) -> Result<(), Diagnostic> {
    let indexed = indexed_payloads(placed);
    let source = dynamic_row(indexed)?;
    let fixups = &indexed.contents().dynamic_fixups;
    let serialized_fixups = &dynamic_payload(placed).contents().address_fixups;
    let semantic = semantic_plan(placed).contents();
    require(
        fixups.len() == DYNAMIC_FIXUP_COUNT
            && serialized_fixups.len() == DYNAMIC_FIXUP_COUNT
            && semantic.address_obligations.len() == DYNAMIC_FIXUP_COUNT
            && contents.applications.len() == DYNAMIC_FIXUP_COUNT,
        "resolved ELF dynamic table does not retain exactly eight applications",
    )?;
    require(
        contents.bytes.len() == source.bytes.len(),
        "resolved ELF dynamic table length drifted from indexed storage",
    )?;

    let mut mutable = vec![false; source.bytes.len()];
    for ((((fixup, serialized), expected_target), application), obligation) in fixups
        .iter()
        .zip(serialized_fixups)
        .zip(EXPECTED_TARGETS)
        .zip(&contents.applications)
        .zip(&semantic.address_obligations)
    {
        require(
            fixup.row_ordinal == serialized.row_ordinal
                && fixup.byte_offset == serialized.byte_offset
                && fixup.byte_width == serialized.byte_width
                && fixup.kind == serialized.kind
                && fixup.target == serialized.target,
            "indexed ELF dynamic fixup drifted from its serialized typed fixup",
        )?;
        let expected = application_for_fixup(placed.load_layout(), fixup)?;
        require(
            expected.target == expected_target
                && application.row_ordinal == expected.row_ordinal
                && application.storage_section_index == expected.storage_section_index
                && application.byte_offset == expected.byte_offset
                && application.byte_width == expected.byte_width
                && application.kind_tag == expected.kind_tag
                && application.target == expected.target
                && application.target_section_index == expected.target_section_index
                && application.target_section_kind == expected.target_section_kind
                && application.value == expected.value,
            "resolved ELF dynamic application drifted from its indexed fixup or placed target",
        )?;
        require(
            application.row_ordinal == obligation.row_ordinal
                && application.byte_width == obligation.byte_width
                && application.target == public_target(obligation.target),
            "resolved ELF dynamic application drifted from its semantic address obligation",
        )?;
        let semantic_row = semantic
            .rows
            .get(application.row_ordinal as usize)
            .ok_or_else(|| Diagnostic::error("resolved ELF dynamic application row is missing"))?;
        require(
            semantic_row.tag == target_tag(application.target)
                && semantic_row.value == ElfDynamicValue::AddressPlaceholder,
            "resolved ELF dynamic target does not match its exact semantic tag row",
        )?;
        let expected_offset = checked_sum(
            checked_product(
                application.row_ordinal as usize,
                ELF64_DYNAMIC_ROW_SIZE,
                "resolved Elf64_Dyn row offset",
            )?,
            ELF64_DYNAMIC_VALUE_OFFSET,
            "resolved Elf64_Dyn value offset",
        )?;
        require(
            application.storage_section_index == DYNAMIC_SECTION_INDEX
                && application.byte_offset == expected_offset
                && application.byte_width == ELF64_DYNAMIC_VALUE_SIZE
                && application.kind_tag == ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress as u8,
            "resolved ELF dynamic application has the wrong storage, coordinate, width, or kind",
        )?;
        let end = checked_sum(
            application.byte_offset,
            usize::from(application.byte_width),
            "resolved ELF dynamic application end",
        )?;
        require(
            end <= mutable.len(),
            "resolved ELF dynamic application exceeds its payload",
        )?;
        require(
            mutable[application.byte_offset..end]
                .iter()
                .all(|covered| !*covered),
            "resolved ELF dynamic applications overlap or duplicate one field",
        )?;
        mutable[application.byte_offset..end].fill(true);
        require(
            read_u64(
                &source.bytes,
                application.byte_offset,
                "indexed Elf64_Dyn address placeholder",
            )? == 0,
            "indexed Elf64_Dyn address field is not an exact zero placeholder",
        )?;
        require(
            read_u64(
                &contents.bytes,
                application.byte_offset,
                "resolved Elf64_Dyn address value",
            )? == application.value,
            "resolved Elf64_Dyn bytes do not contain the placed target address",
        )?;
    }
    for (offset, (actual, upstream)) in contents.bytes.iter().zip(&source.bytes).enumerate() {
        if !mutable[offset] {
            require(
                actual == upstream,
                "non-fixup Elf64_Dyn byte drifted from indexed storage",
            )?;
        }
    }

    let decoded = decode_rows(&contents.bytes, semantic.rows.len())?;
    for (ordinal, (decoded, row)) in decoded.iter().zip(&semantic.rows).enumerate() {
        let application = contents
            .applications
            .iter()
            .find(|application| application.row_ordinal as usize == ordinal);
        let expected_value = match row.value {
            ElfDynamicValue::AddressPlaceholder => {
                application
                    .ok_or_else(|| {
                        Diagnostic::error("resolved Elf64_Dyn row is missing its address")
                    })?
                    .value
            }
            value => {
                require(
                    application.is_none(),
                    "literal Elf64_Dyn row has an orphan address application",
                )?;
                encoded_value(value)
            }
        };
        require(
            decoded.tag == row.tag as i64 && decoded.value == expected_value,
            "decoded resolved Elf64_Dyn row drifted from semantic order or value",
        )?;
    }
    Ok(())
}

fn indexed_payloads(
    placed: &ValidatedElfPlacedSectionHeaderTable,
) -> &ValidatedElfIndexedSectionPayloadPlan {
    placed.load_layout().relative().payloads()
}

fn dynamic_row(
    indexed: &ValidatedElfIndexedSectionPayloadPlan,
) -> Result<&crate::dynamic_executable::section_headers::section_payload_roster::ElfIndexedSectionPayloadRow, Diagnostic>{
    indexed
        .contents()
        .rows
        .get(DYNAMIC_SECTION_INDEX as usize)
        .filter(|row| {
            row.index == DYNAMIC_SECTION_INDEX
                && row.kind == ElfDynamicRosterSectionKind::DynamicTable
        })
        .ok_or_else(|| Diagnostic::error("indexed ELF .dynamic storage row is missing"))
}

fn dynamic_payload(
    placed: &ValidatedElfPlacedSectionHeaderTable,
) -> &ValidatedElfDynamicTablePayload {
    placed
        .load_layout()
        .relative()
        .payloads()
        .section_headers()
        .roster()
        .section_names()
        .dynamic_table()
        .payload()
}

fn semantic_plan(placed: &ValidatedElfPlacedSectionHeaderTable) -> &ValidatedElfDynamicTagPlan {
    dynamic_payload(placed).plan()
}

const fn public_target(target: ElfDynamicAddressTarget) -> ElfDynamicAddressApplicationTarget {
    match target {
        ElfDynamicAddressTarget::ProcedureGot => ElfDynamicAddressApplicationTarget::ProcedureGot,
        ElfDynamicAddressTarget::SystemVHash => ElfDynamicAddressApplicationTarget::SystemVHash,
        ElfDynamicAddressTarget::GnuHash => ElfDynamicAddressApplicationTarget::GnuHash,
        ElfDynamicAddressTarget::DynamicString => ElfDynamicAddressApplicationTarget::DynamicString,
        ElfDynamicAddressTarget::DynamicSymbol => ElfDynamicAddressApplicationTarget::DynamicSymbol,
        ElfDynamicAddressTarget::ProcedureRelocation => {
            ElfDynamicAddressApplicationTarget::ProcedureRelocation
        }
        ElfDynamicAddressTarget::GnuSymbolVersion => {
            ElfDynamicAddressApplicationTarget::GnuSymbolVersion
        }
        ElfDynamicAddressTarget::GnuVersionRequirement => {
            ElfDynamicAddressApplicationTarget::GnuVersionRequirement
        }
        ElfDynamicAddressTarget::GeneralRelocation => {
            ElfDynamicAddressApplicationTarget::GeneralRelocation
        }
    }
}

const fn target_section_index(target: ElfDynamicAddressApplicationTarget) -> u32 {
    match target {
        ElfDynamicAddressApplicationTarget::ProcedureGot => 9,
        ElfDynamicAddressApplicationTarget::SystemVHash => 4,
        ElfDynamicAddressApplicationTarget::GnuHash => 7,
        ElfDynamicAddressApplicationTarget::DynamicString => 2,
        ElfDynamicAddressApplicationTarget::DynamicSymbol => 3,
        ElfDynamicAddressApplicationTarget::ProcedureRelocation => 10,
        ElfDynamicAddressApplicationTarget::GnuSymbolVersion => 5,
        ElfDynamicAddressApplicationTarget::GnuVersionRequirement => 6,
        ElfDynamicAddressApplicationTarget::GeneralRelocation => 11,
    }
}

const fn target_section_kind(
    target: ElfDynamicAddressApplicationTarget,
) -> ElfPlacedDynamicSectionKind {
    match target {
        ElfDynamicAddressApplicationTarget::ProcedureGot => {
            ElfPlacedDynamicSectionKind::ProcedureGot
        }
        ElfDynamicAddressApplicationTarget::SystemVHash => ElfPlacedDynamicSectionKind::SystemVHash,
        ElfDynamicAddressApplicationTarget::GnuHash => ElfPlacedDynamicSectionKind::GnuHash,
        ElfDynamicAddressApplicationTarget::DynamicString => {
            ElfPlacedDynamicSectionKind::DynamicString
        }
        ElfDynamicAddressApplicationTarget::DynamicSymbol => {
            ElfPlacedDynamicSectionKind::DynamicSymbol
        }
        ElfDynamicAddressApplicationTarget::ProcedureRelocation => {
            ElfPlacedDynamicSectionKind::ProcedureRelocation
        }
        ElfDynamicAddressApplicationTarget::GnuSymbolVersion => {
            ElfPlacedDynamicSectionKind::GnuSymbolVersion
        }
        ElfDynamicAddressApplicationTarget::GnuVersionRequirement => {
            ElfPlacedDynamicSectionKind::GnuVersionRequirement
        }
        ElfDynamicAddressApplicationTarget::GeneralRelocation => {
            ElfPlacedDynamicSectionKind::GeneralRelocation
        }
    }
}

const fn target_tag(target: ElfDynamicAddressApplicationTarget) -> ElfDynamicTag {
    match target {
        ElfDynamicAddressApplicationTarget::ProcedureGot => ElfDynamicTag::ProcedureGot,
        ElfDynamicAddressApplicationTarget::SystemVHash => ElfDynamicTag::SystemVHash,
        ElfDynamicAddressApplicationTarget::GnuHash => ElfDynamicTag::GnuHash,
        ElfDynamicAddressApplicationTarget::DynamicString => ElfDynamicTag::DynamicString,
        ElfDynamicAddressApplicationTarget::DynamicSymbol => ElfDynamicTag::DynamicSymbol,
        ElfDynamicAddressApplicationTarget::ProcedureRelocation => {
            ElfDynamicTag::ProcedureRelocation
        }
        ElfDynamicAddressApplicationTarget::GnuSymbolVersion => ElfDynamicTag::GnuSymbolVersion,
        ElfDynamicAddressApplicationTarget::GnuVersionRequirement => {
            ElfDynamicTag::GnuVersionRequirement
        }
        ElfDynamicAddressApplicationTarget::GeneralRelocation => ElfDynamicTag::Rela,
    }
}

const fn encoded_value(value: ElfDynamicValue) -> u64 {
    match value {
        ElfDynamicValue::NeededStringOffset(offset) => offset as u64,
        ElfDynamicValue::ProcedureRelocationByteCount(count)
        | ElfDynamicValue::GeneralRelocationByteCount(count)
        | ElfDynamicValue::DynamicStringByteCount(count)
        | ElfDynamicValue::DynamicSymbolEntryByteCount(count)
        | ElfDynamicValue::VersionRequirementRecordCount(count) => count,
        ElfDynamicValue::RelocationTag(tag) => (tag as i64) as u64,
        ElfDynamicValue::AddressPlaceholder | ElfDynamicValue::Null => 0,
    }
}

fn decode_rows(
    bytes: &[u8],
    expected_count: usize,
) -> Result<Vec<DecodedElfDynamicRow>, Diagnostic> {
    let expected_size = checked_product(
        expected_count,
        ELF64_DYNAMIC_ROW_SIZE,
        "resolved Elf64_Dyn payload size",
    )?;
    require(
        bytes.len() == expected_size,
        "resolved Elf64_Dyn payload has a truncated row or trailing bytes",
    )?;
    let mut rows = Vec::with_capacity(expected_count);
    for ordinal in 0..expected_count {
        let offset = checked_product(ordinal, ELF64_DYNAMIC_ROW_SIZE, "resolved Elf64_Dyn row")?;
        rows.push(DecodedElfDynamicRow {
            tag: read_i64(bytes, offset, "resolved Elf64_Dyn.d_tag")?,
            value: read_u64(
                bytes,
                checked_sum(offset, ELF64_DYNAMIC_VALUE_OFFSET, "resolved d_un offset")?,
                "resolved Elf64_Dyn.d_un",
            )?,
        });
    }
    Ok(rows)
}

fn field_mut(bytes: &mut [u8], offset: usize, width: u8) -> Result<&mut [u8], Diagnostic> {
    let end = checked_sum(offset, usize::from(width), "dynamic address field end")?;
    bytes
        .get_mut(offset..end)
        .ok_or_else(|| Diagnostic::error("dynamic address fixup exceeds its storage"))
}

fn read_i64(bytes: &[u8], offset: usize, context: &'static str) -> Result<i64, Diagnostic> {
    let end = checked_sum(offset, 8, context)?;
    let value = bytes
        .get(offset..end)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))?;
    Ok(i64::from_le_bytes(value))
}

fn non_authoritative_resolved_compatibility_fingerprint(
    placed: &ValidatedElfPlacedSectionHeaderTable,
    contents: &ElfResolvedDynamicTableContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf.resolved-dynamic-table.v1");
    hash.bytes(
        &placed
            .non_authoritative_placed_compatibility_fingerprint()
            .to_le_bytes(),
    );
    hash.bytes(&contents.bytes);
    hash.bytes(&(contents.applications.len() as u64).to_le_bytes());
    for application in &contents.applications {
        hash.bytes(&application.row_ordinal.to_le_bytes());
        hash.bytes(&application.storage_section_index.to_le_bytes());
        hash.bytes(&(application.byte_offset as u64).to_le_bytes());
        hash.byte(application.byte_width);
        hash.byte(application.kind_tag);
        hash.byte(application.target as u8);
        hash.bytes(&application.target_section_index.to_le_bytes());
        hash.byte(application.target_section_kind as u8);
        hash.bytes(&application.value.to_le_bytes());
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
mod tests;
