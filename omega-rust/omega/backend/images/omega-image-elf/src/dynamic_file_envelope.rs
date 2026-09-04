//! Exact ELF64 header prefix and final section-header-table file fragment.
//!
//! This layer consumes the resolved `.dynamic` owner so the linear custody
//! chain cannot fork before file-container serialization. It emits one exact
//! ELF64-LSB header followed by the five already-planned program headers and
//! binds the already-applied thirteen-row section-header table to the exact
//! `e_shoff` retained by the absolute load layout. A separate decoder replays
//! every field and rejoins both byte regions to their upstream owners.
//!
//! The carrier deliberately does not resolve procedure or source relocations,
//! copy any dynamic payload into a file image, mutate the retained `FinalImage`,
//! grant loader, publication, or runnable-image authority.

use crate::bytes::{write_u16, write_u32, write_u64};
use crate::entry::elf_entry_address;
use crate::load_layout::{
    ElfLoadProgramHeader, ElfLoadProgramHeaderKind, ValidatedElfDynamicLoadLayout,
};
use crate::resolved_dynamic_table::ValidatedElfResolvedDynamicTable;
use omega_image::FinalImageSection;
use omega_object_file::SymbolKind;
use omega_target::TargetProfile;
use psi_diagnostics::Diagnostic;

const ELF64_HEADER_SIZE: usize = 64;
const ELF64_PROGRAM_HEADER_SIZE: usize = 56;
const ELF64_PROGRAM_HEADER_COUNT: usize = 5;
const ELF64_HEADER_PREFIX_SIZE: usize =
    ELF64_HEADER_SIZE + ELF64_PROGRAM_HEADER_SIZE * ELF64_PROGRAM_HEADER_COUNT;
const ELF64_SECTION_HEADER_SIZE: usize = 64;
const ELF64_SECTION_HEADER_COUNT: usize = 13;
const ELF64_SECTION_HEADER_TABLE_SIZE: usize =
    ELF64_SECTION_HEADER_SIZE * ELF64_SECTION_HEADER_COUNT;
const ELF64_SECTION_NAME_TABLE_INDEX: u16 = 12;

const ET_EXEC: u16 = 2;
const EM_X86_64: u16 = 62;
const EM_AARCH64: u16 = 183;
const EV_CURRENT: u32 = 1;
const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Independently replayed dynamic ELF file-container envelope.
///
/// The two byte regions are exact file fragments, not a runnable file. The
/// retained resolved-dynamic owner remains the sole custody path to every
/// section payload, source byte, relocation obligation, and selected import.
#[derive(Debug)]
#[must_use = "dynamic ELF file envelope retains the complete pre-mutation custody chain"]
pub struct ValidatedElfDynamicFileEnvelope {
    resolved_dynamic_table: ValidatedElfResolvedDynamicTable,
    contents: ElfDynamicFileEnvelopeContents,
    non_authoritative_envelope_compatibility_fingerprint: u64,
}

impl ValidatedElfDynamicFileEnvelope {
    pub const fn resolved_dynamic_table(&self) -> &ValidatedElfResolvedDynamicTable {
        &self.resolved_dynamic_table
    }

    /// Exact bytes at file offset zero: one `Elf64_Ehdr` followed by five
    /// `Elf64_Phdr` rows.
    pub fn header_prefix_bytes(&self) -> &[u8] {
        &self.contents.header_prefix_bytes
    }

    pub const fn entry_address(&self) -> u64 {
        self.contents.entry_address
    }

    pub const fn section_header_table_file_offset(&self) -> u64 {
        self.contents.section_header_table_file_offset
    }

    /// Exact applied thirteen-row section-header table to place at
    /// [`Self::section_header_table_file_offset`].
    pub fn section_header_table_bytes(&self) -> &[u8] {
        &self.contents.section_header_table_bytes
    }

    /// Compatibility/report coordinate only. Later mutation and admission must
    /// replay the exact retained owner and both byte regions.
    pub const fn non_authoritative_envelope_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_envelope_compatibility_fingerprint
    }

    pub(crate) fn into_resolved_dynamic_table(self) -> ValidatedElfResolvedDynamicTable {
        self.resolved_dynamic_table
    }
}

/// Rejected serialization retaining the exact resolved-dynamic owner.
#[derive(Debug)]
#[must_use = "dynamic ELF envelope rejection retains resolved-dynamic custody"]
pub struct ElfDynamicFileEnvelopeSerializationError {
    resolved_dynamic_table: ValidatedElfResolvedDynamicTable,
    diagnostic: Diagnostic,
}

impl ElfDynamicFileEnvelopeSerializationError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfResolvedDynamicTable, Diagnostic) {
        (self.resolved_dynamic_table, self.diagnostic)
    }
}

impl std::fmt::Display for ElfDynamicFileEnvelopeSerializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfDynamicFileEnvelopeSerializationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ElfDynamicFileEnvelopeContents {
    header_prefix_bytes: Vec<u8>,
    entry_address: u64,
    section_header_table_file_offset: u64,
    section_header_table_bytes: Vec<u8>,
}

struct Candidate {
    resolved_dynamic_table: ValidatedElfResolvedDynamicTable,
    contents: ElfDynamicFileEnvelopeContents,
    non_authoritative_envelope_compatibility_fingerprint: u64,
}

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DecodedElf64Header {
    object_type: u16,
    machine: u16,
    version: u32,
    entry: u64,
    program_header_offset: u64,
    section_header_offset: u64,
    flags: u32,
    header_size: u16,
    program_header_entry_size: u16,
    program_header_count: u16,
    section_header_entry_size: u16,
    section_header_count: u16,
    section_name_table_index: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DecodedElf64ProgramHeader {
    segment_type: u32,
    flags: u32,
    file_offset: u64,
    virtual_address: u64,
    physical_address: u64,
    file_size: u64,
    memory_size: u64,
    alignment: u64,
}

/// Serialize and replay the exact dynamic ELF header envelope without mutating
/// any retained payload or claiming a runnable file.
pub fn serialize_elf_dynamic_file_envelope(
    resolved_dynamic_table: ValidatedElfResolvedDynamicTable,
) -> Result<ValidatedElfDynamicFileEnvelope, Box<ElfDynamicFileEnvelopeSerializationError>> {
    let contents = match derive_contents(&resolved_dynamic_table) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(ElfDynamicFileEnvelopeSerializationError {
                resolved_dynamic_table,
                diagnostic,
            }));
        }
    };
    let non_authoritative_envelope_compatibility_fingerprint =
        non_authoritative_envelope_compatibility_fingerprint(&resolved_dynamic_table, &contents);
    let candidate = Candidate {
        resolved_dynamic_table,
        contents,
        non_authoritative_envelope_compatibility_fingerprint,
    };
    validate_candidate(candidate).map_err(|error| {
        Box::new(ElfDynamicFileEnvelopeSerializationError {
            resolved_dynamic_table: error.candidate.resolved_dynamic_table,
            diagnostic: error.diagnostic,
        })
    })
}

fn derive_contents(
    resolved: &ValidatedElfResolvedDynamicTable,
) -> Result<ElfDynamicFileEnvelopeContents, Diagnostic> {
    let layout = load_layout(resolved);
    let entry_address = expected_entry_address(layout)?;
    let section_header_table_file_offset = layout.section_header_table_file_offset();
    let mut header_prefix_bytes = Vec::with_capacity(ELF64_HEADER_PREFIX_SIZE);

    header_prefix_bytes.extend([0x7f, b'E', b'L', b'F']);
    header_prefix_bytes.push(2); // ELFCLASS64
    header_prefix_bytes.push(1); // ELFDATA2LSB
    header_prefix_bytes.push(1); // EV_CURRENT
    header_prefix_bytes.push(0); // ELFOSABI_NONE
    header_prefix_bytes.push(0); // unspecified ABI version
    header_prefix_bytes.extend([0; 7]);
    write_u16(&mut header_prefix_bytes, ET_EXEC);
    write_u16(&mut header_prefix_bytes, target_machine(layout.target())?);
    write_u32(&mut header_prefix_bytes, EV_CURRENT);
    write_u64(&mut header_prefix_bytes, entry_address);
    write_u64(&mut header_prefix_bytes, ELF64_HEADER_SIZE as u64);
    write_u64(&mut header_prefix_bytes, section_header_table_file_offset);
    write_u32(&mut header_prefix_bytes, 0);
    write_u16(&mut header_prefix_bytes, ELF64_HEADER_SIZE as u16);
    write_u16(&mut header_prefix_bytes, ELF64_PROGRAM_HEADER_SIZE as u16);
    write_u16(&mut header_prefix_bytes, ELF64_PROGRAM_HEADER_COUNT as u16);
    write_u16(&mut header_prefix_bytes, ELF64_SECTION_HEADER_SIZE as u16);
    write_u16(&mut header_prefix_bytes, ELF64_SECTION_HEADER_COUNT as u16);
    write_u16(&mut header_prefix_bytes, ELF64_SECTION_NAME_TABLE_INDEX);

    for header in layout.program_headers() {
        write_program_header(&mut header_prefix_bytes, header);
    }

    Ok(ElfDynamicFileEnvelopeContents {
        header_prefix_bytes,
        entry_address,
        section_header_table_file_offset,
        section_header_table_bytes: resolved.placed_section_headers().bytes().to_vec(),
    })
}

fn write_program_header(bytes: &mut Vec<u8>, header: &ElfLoadProgramHeader) {
    write_u32(bytes, segment_type(header.kind()));
    write_u32(bytes, header.flags());
    write_u64(bytes, header.file_offset());
    write_u64(bytes, header.virtual_address());
    write_u64(bytes, header.physical_address());
    write_u64(bytes, header.file_size());
    write_u64(bytes, header.memory_size());
    write_u64(bytes, header.alignment());
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfDynamicFileEnvelope, CandidateValidationError> {
    if let Err(diagnostic) =
        validate_contents(&candidate.resolved_dynamic_table, &candidate.contents)
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    let expected_fingerprint = non_authoritative_envelope_compatibility_fingerprint(
        &candidate.resolved_dynamic_table,
        &candidate.contents,
    );
    if candidate.non_authoritative_envelope_compatibility_fingerprint == 0
        || candidate.non_authoritative_envelope_compatibility_fingerprint != expected_fingerprint
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error(
                "dynamic ELF envelope compatibility fingerprint does not replay",
            ),
        });
    }
    Ok(ValidatedElfDynamicFileEnvelope {
        resolved_dynamic_table: candidate.resolved_dynamic_table,
        contents: candidate.contents,
        non_authoritative_envelope_compatibility_fingerprint: candidate
            .non_authoritative_envelope_compatibility_fingerprint,
    })
}

fn validate_contents(
    resolved: &ValidatedElfResolvedDynamicTable,
    contents: &ElfDynamicFileEnvelopeContents,
) -> Result<(), Diagnostic> {
    let layout = load_layout(resolved);
    require(
        contents.header_prefix_bytes.len() == ELF64_HEADER_PREFIX_SIZE,
        "dynamic ELF header prefix is truncated or has trailing bytes",
    )?;
    let header = decode_header(&contents.header_prefix_bytes)?;
    let expected_entry = expected_entry_address(layout)?;
    require(
        contents.entry_address == expected_entry && header.entry == expected_entry,
        "dynamic ELF entry address drifted from the retained text entry symbol",
    )?;
    require(
        header.section_header_offset == contents.section_header_table_file_offset,
        "dynamic ELF e_shoff drifted from its retained section-header fragment",
    )?;
    require(
        &contents.header_prefix_bytes[0..4] == b"\x7fELF"
            && contents.header_prefix_bytes[4] == 2
            && contents.header_prefix_bytes[5] == 1
            && contents.header_prefix_bytes[6] == 1
            && contents.header_prefix_bytes[7] == 0
            && contents.header_prefix_bytes[8] == 0
            && contents.header_prefix_bytes[9..16]
                .iter()
                .all(|byte| *byte == 0),
        "dynamic ELF identification bytes are not canonical ELF64-LSB",
    )?;
    require(
        header.object_type == ET_EXEC
            && header.machine == target_machine(layout.target())?
            && header.version == EV_CURRENT
            && header.program_header_offset == ELF64_HEADER_SIZE as u64
            && header.flags == 0
            && header.header_size == ELF64_HEADER_SIZE as u16
            && header.program_header_entry_size == ELF64_PROGRAM_HEADER_SIZE as u16
            && header.program_header_count == ELF64_PROGRAM_HEADER_COUNT as u16
            && header.section_header_entry_size == ELF64_SECTION_HEADER_SIZE as u16
            && header.section_header_count == ELF64_SECTION_HEADER_COUNT as u16
            && header.section_name_table_index == ELF64_SECTION_NAME_TABLE_INDEX,
        "decoded dynamic ELF header drifted from its target, sizes, or closed table geometry",
    )?;

    let decoded_program_headers = decode_program_headers(&contents.header_prefix_bytes)?;
    require(
        decoded_program_headers.len() == layout.program_headers().len()
            && layout.program_headers().len() == ELF64_PROGRAM_HEADER_COUNT,
        "dynamic ELF program-header count drifted from the absolute load layout",
    )?;
    for (decoded, planned) in decoded_program_headers.iter().zip(layout.program_headers()) {
        let expected = DecodedElf64ProgramHeader {
            segment_type: segment_type(planned.kind()),
            flags: planned.flags(),
            file_offset: planned.file_offset(),
            virtual_address: planned.virtual_address(),
            physical_address: planned.physical_address(),
            file_size: planned.file_size(),
            memory_size: planned.memory_size(),
            alignment: planned.alignment(),
        };
        require(
            *decoded == expected,
            "decoded dynamic ELF program header drifted from its exact planned row",
        )?;
    }

    let read_only = layout
        .program_headers()
        .iter()
        .find(|header| header.kind() == ElfLoadProgramHeaderKind::LoadReadOnly)
        .ok_or_else(|| Diagnostic::error("dynamic ELF envelope has no read-only load"))?;
    require(
        read_only.file_offset() == 0 && read_only.file_size() >= ELF64_HEADER_PREFIX_SIZE as u64,
        "dynamic ELF read-only load does not contain the complete header prefix",
    )?;
    require(
        contents.section_header_table_bytes.len() == ELF64_SECTION_HEADER_TABLE_SIZE
            && contents.section_header_table_bytes == resolved.placed_section_headers().bytes(),
        "dynamic ELF section-header fragment drifted from the applied table owner",
    )?;
    validate_section_header_fragment_geometry(
        layout.program_headers(),
        contents.section_header_table_file_offset,
        contents.section_header_table_bytes.len(),
    )?;
    require(
        contents.section_header_table_file_offset == layout.section_header_table_file_offset(),
        "dynamic ELF section-header fragment drifted from the retained absolute placement",
    )?;
    Ok(())
}

fn validate_section_header_fragment_geometry(
    program_headers: &[ElfLoadProgramHeader],
    file_offset: u64,
    byte_count: usize,
) -> Result<(), Diagnostic> {
    let byte_count = u64::try_from(byte_count).map_err(|_| {
        Diagnostic::error("dynamic ELF section-header byte count exceeds Elf64_Xword")
    })?;
    checked_sum_u64(
        file_offset,
        byte_count,
        "dynamic ELF section-header fragment end",
    )?;
    require(
        file_offset >= ELF64_HEADER_PREFIX_SIZE as u64,
        "dynamic ELF section-header fragment overlaps the header prefix",
    )?;
    for load in program_headers.iter().filter(|header| {
        matches!(
            header.kind(),
            ElfLoadProgramHeaderKind::LoadReadOnly
                | ElfLoadProgramHeaderKind::LoadReadExecute
                | ElfLoadProgramHeaderKind::LoadReadWrite
        )
    }) {
        let load_end = checked_sum_u64(
            load.file_offset(),
            load.file_size(),
            "dynamic ELF PT_LOAD file end",
        )?;
        require(
            file_offset >= load_end,
            "dynamic ELF section-header fragment overlaps a loadable file extent",
        )?;
    }
    Ok(())
}

fn decode_header(bytes: &[u8]) -> Result<DecodedElf64Header, Diagnostic> {
    require(
        bytes.len() >= ELF64_HEADER_SIZE,
        "dynamic ELF header is truncated",
    )?;
    Ok(DecodedElf64Header {
        object_type: read_u16(bytes, 16, "Elf64_Ehdr.e_type")?,
        machine: read_u16(bytes, 18, "Elf64_Ehdr.e_machine")?,
        version: read_u32(bytes, 20, "Elf64_Ehdr.e_version")?,
        entry: read_u64(bytes, 24, "Elf64_Ehdr.e_entry")?,
        program_header_offset: read_u64(bytes, 32, "Elf64_Ehdr.e_phoff")?,
        section_header_offset: read_u64(bytes, 40, "Elf64_Ehdr.e_shoff")?,
        flags: read_u32(bytes, 48, "Elf64_Ehdr.e_flags")?,
        header_size: read_u16(bytes, 52, "Elf64_Ehdr.e_ehsize")?,
        program_header_entry_size: read_u16(bytes, 54, "Elf64_Ehdr.e_phentsize")?,
        program_header_count: read_u16(bytes, 56, "Elf64_Ehdr.e_phnum")?,
        section_header_entry_size: read_u16(bytes, 58, "Elf64_Ehdr.e_shentsize")?,
        section_header_count: read_u16(bytes, 60, "Elf64_Ehdr.e_shnum")?,
        section_name_table_index: read_u16(bytes, 62, "Elf64_Ehdr.e_shstrndx")?,
    })
}

fn decode_program_headers(bytes: &[u8]) -> Result<Vec<DecodedElf64ProgramHeader>, Diagnostic> {
    require(
        bytes.len() == ELF64_HEADER_PREFIX_SIZE,
        "dynamic ELF program-header table is truncated or has trailing bytes",
    )?;
    let mut headers = Vec::with_capacity(ELF64_PROGRAM_HEADER_COUNT);
    for ordinal in 0..ELF64_PROGRAM_HEADER_COUNT {
        let offset = checked_sum(
            ELF64_HEADER_SIZE,
            checked_product(ordinal, ELF64_PROGRAM_HEADER_SIZE, "Elf64_Phdr row offset")?,
            "Elf64_Phdr table offset",
        )?;
        headers.push(DecodedElf64ProgramHeader {
            segment_type: read_u32(bytes, offset, "Elf64_Phdr.p_type")?,
            flags: read_u32(bytes, offset + 4, "Elf64_Phdr.p_flags")?,
            file_offset: read_u64(bytes, offset + 8, "Elf64_Phdr.p_offset")?,
            virtual_address: read_u64(bytes, offset + 16, "Elf64_Phdr.p_vaddr")?,
            physical_address: read_u64(bytes, offset + 24, "Elf64_Phdr.p_paddr")?,
            file_size: read_u64(bytes, offset + 32, "Elf64_Phdr.p_filesz")?,
            memory_size: read_u64(bytes, offset + 40, "Elf64_Phdr.p_memsz")?,
            alignment: read_u64(bytes, offset + 48, "Elf64_Phdr.p_align")?,
        });
    }
    Ok(headers)
}

fn expected_entry_address(layout: &ValidatedElfDynamicLoadLayout) -> Result<u64, Diagnostic> {
    let image = layout.retained_image();
    let entry = image
        .symbol_table
        .symbols
        .is_valid(image.symbol_table.entry_symbol)
        .then(|| {
            image
                .symbol_table
                .symbols
                .get(image.symbol_table.entry_symbol)
        })
        .ok_or_else(|| {
            Diagnostic::error("dynamic ELF envelope has no exact final-image entry symbol")
        })?;
    require(
        entry.section == FinalImageSection::Text,
        "dynamic ELF envelope entry symbol is not in source text",
    )?;
    require(
        entry.kind == SymbolKind::Function,
        "dynamic ELF envelope entry symbol is not a function",
    )?;
    require(
        entry.size != 0,
        "dynamic ELF envelope entry function has an empty text span",
    )?;
    let entry_end = entry
        .offset
        .checked_add(entry.size)
        .ok_or_else(|| Diagnostic::error("dynamic ELF envelope entry span overflows usize"))?;
    require(
        entry.offset < image.memory.text.len() && entry_end <= image.memory.text.len(),
        "dynamic ELF envelope entry function lies outside source text",
    )?;
    layout
        .image_memory()
        .text_virtual_address()
        .checked_add(entry.offset as u64)
        .ok_or_else(|| Diagnostic::error("dynamic ELF entry address overflows Elf64_Addr"))?;
    elf_entry_address(image, layout.image_memory().text_virtual_address())
}

fn load_layout(resolved: &ValidatedElfResolvedDynamicTable) -> &ValidatedElfDynamicLoadLayout {
    resolved.placed_section_headers().load_layout()
}

fn target_machine(target: TargetProfile) -> Result<u16, Diagnostic> {
    match target {
        TargetProfile::LinuxX64 => Ok(EM_X86_64),
        TargetProfile::LinuxArm64 => Ok(EM_AARCH64),
        _ => Err(Diagnostic::error(
            "dynamic ELF envelope requires an exact Linux x86-64 or AArch64 profile",
        )),
    }
}

const fn segment_type(kind: ElfLoadProgramHeaderKind) -> u32 {
    match kind {
        ElfLoadProgramHeaderKind::Interpreter => PT_INTERP,
        ElfLoadProgramHeaderKind::LoadReadOnly
        | ElfLoadProgramHeaderKind::LoadReadExecute
        | ElfLoadProgramHeaderKind::LoadReadWrite => PT_LOAD,
        ElfLoadProgramHeaderKind::Dynamic => PT_DYNAMIC,
    }
}

fn read_u16(bytes: &[u8], offset: usize, context: &'static str) -> Result<u16, Diagnostic> {
    let end = checked_sum(offset, 2, context)?;
    let value = bytes
        .get(offset..end)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))?;
    Ok(u16::from_le_bytes(value))
}

fn read_u32(bytes: &[u8], offset: usize, context: &'static str) -> Result<u32, Diagnostic> {
    let end = checked_sum(offset, 4, context)?;
    let value = bytes
        .get(offset..end)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))?;
    Ok(u32::from_le_bytes(value))
}

fn read_u64(bytes: &[u8], offset: usize, context: &'static str) -> Result<u64, Diagnostic> {
    let end = checked_sum(offset, 8, context)?;
    let value = bytes
        .get(offset..end)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))?;
    Ok(u64::from_le_bytes(value))
}

fn checked_product(left: usize, right: usize, context: &'static str) -> Result<usize, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
}

fn checked_sum(left: usize, right: usize, context: &'static str) -> Result<usize, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
}

fn checked_sum_u64(left: u64, right: u64, context: &'static str) -> Result<u64, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows Elf64_Off")))
}

fn require(condition: bool, message: &'static str) -> Result<(), Diagnostic> {
    condition
        .then_some(())
        .ok_or_else(|| Diagnostic::error(message))
}

fn non_authoritative_envelope_compatibility_fingerprint(
    resolved: &ValidatedElfResolvedDynamicTable,
    contents: &ElfDynamicFileEnvelopeContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf.dynamic-file-envelope.v1");
    hash.bytes(
        &resolved
            .non_authoritative_resolved_compatibility_fingerprint()
            .to_le_bytes(),
    );
    hash.bytes(&contents.header_prefix_bytes);
    hash.bytes(&contents.entry_address.to_le_bytes());
    hash.bytes(&contents.section_header_table_file_offset.to_le_bytes());
    hash.bytes(&contents.section_header_table_bytes);
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
        apply_elf_dynamic_address_fixups, apply_elf_section_header_placements,
        plan_elf_dynamic_link_inputs, plan_elf_dynamic_load_layout,
        plan_elf_dynamic_section_descriptors, plan_elf_dynamic_section_roster,
        plan_elf_dynamic_sections, plan_elf_dynamic_table_section_descriptor,
        plan_elf_dynamic_tags, plan_elf_indexed_section_payloads,
        plan_elf_procedure_linkage_relocations, plan_elf_procedure_linkage_section_descriptors,
        plan_elf_procedure_linkage_templates, plan_elf_relative_section_payload_layout,
        plan_elf_section_name_table, serialize_elf_dynamic_sections, serialize_elf_dynamic_table,
        serialize_elf_section_header_table,
    };
    use omega_image::{
        FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
        FinalImageSymbol,
    };
    use omega_object_file::{RelocationKind, SymbolKind};
    use omega_target::{
        ForeignLocatorCandidate, normalize_elf_interpreter_plan, normalize_foreign_locator,
    };
    use psi_arena::Handle;

    #[derive(Clone, Copy)]
    enum EntryFixture {
        Text {
            offset: usize,
        },
        TextExplicit {
            offset: usize,
            size: usize,
            kind: SymbolKind,
        },
        Data,
        Missing,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct CustodySnapshot {
        image: FinalImage,
        resolved_dynamic_bytes: Vec<u8>,
        placed_section_header_bytes: Vec<u8>,
    }

    fn resolved(
        target: TargetProfile,
        entry_fixture: EntryFixture,
        imported_symbol: &[u8],
    ) -> ValidatedElfResolvedDynamicTable {
        let mut image = FinalImage::with_capacity(
            target.native_target(),
            FinalImageMemory {
                text: vec![0; 32],
                data: vec![0x5a; 13],
                bss_size: 23,
                bss_alignment: 16,
            },
            Handle::invalid(),
            2,
            1,
            1,
        );
        let entry = match entry_fixture {
            EntryFixture::Text { offset } => {
                Some((FinalImageSection::Text, offset, 4, SymbolKind::Function))
            }
            EntryFixture::TextExplicit { offset, size, kind } => {
                Some((FinalImageSection::Text, offset, size, kind))
            }
            EntryFixture::Data => Some((FinalImageSection::Data, 0, 4, SymbolKind::Function)),
            EntryFixture::Missing => None,
        };
        if let Some((section, offset, size, kind)) = entry {
            let entry = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: "_start".to_owned(),
                section,
                offset,
                size,
                kind,
            });
            image.symbol_table.entry_symbol = entry;
        }
        let imported = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "__omega_dynamic_envelope_import".to_owned(),
            section: FinalImageSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: imported,
            import: FinalImageImportPlan::Normalized(
                normalize_foreign_locator(
                    ForeignLocatorCandidate::ElfVersioned {
                        object: b"libdynamic-envelope.so".to_vec(),
                        symbol: imported_symbol.to_vec(),
                        version: b"DYNAMIC_ENVELOPE_1".to_vec(),
                    },
                    target,
                )
                .unwrap(),
            ),
        });
        let (offset, kind) = match target {
            TargetProfile::LinuxX64 => {
                image.memory.text[0] = 0xe8;
                (1, RelocationKind::X86_64Relative32)
            }
            TargetProfile::LinuxArm64 => {
                image.memory.text[0..4].copy_from_slice(&[0, 0, 0, 0x94]);
                (0, RelocationKind::Aarch64Branch26)
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
                symbol_handle: imported,
                addend: 0,
                kind,
            });
        let interpreter_path = match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-x86-64.so.2".as_slice(),
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-aarch64.so.1".as_slice(),
            _ => unreachable!(),
        };
        let interpreter =
            normalize_elf_interpreter_plan(interpreter_path.to_vec(), target).unwrap();
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
        let headers = serialize_elf_section_header_table(roster).unwrap();
        let payloads = plan_elf_indexed_section_payloads(headers).unwrap();
        let relative = plan_elf_relative_section_payload_layout(payloads).unwrap();
        let load = plan_elf_dynamic_load_layout(relative).unwrap();
        let placed = apply_elf_section_header_placements(load).unwrap();
        apply_elf_dynamic_address_fixups(placed).unwrap()
    }

    fn standard_resolved(target: TargetProfile) -> ValidatedElfResolvedDynamicTable {
        resolved(
            target,
            EntryFixture::Text { offset: 8 },
            b"dynamic_envelope_call",
        )
    }

    fn candidate(target: TargetProfile) -> Candidate {
        let resolved_dynamic_table = standard_resolved(target);
        let contents = derive_contents(&resolved_dynamic_table).unwrap();
        let non_authoritative_envelope_compatibility_fingerprint =
            non_authoritative_envelope_compatibility_fingerprint(
                &resolved_dynamic_table,
                &contents,
            );
        Candidate {
            resolved_dynamic_table,
            contents,
            non_authoritative_envelope_compatibility_fingerprint,
        }
    }

    fn custody_snapshot(resolved: &ValidatedElfResolvedDynamicTable) -> CustodySnapshot {
        CustodySnapshot {
            image: load_layout(resolved).retained_image().clone(),
            resolved_dynamic_bytes: resolved.bytes().to_vec(),
            placed_section_header_bytes: resolved.placed_section_headers().bytes().to_vec(),
        }
    }

    fn assert_rejected_with_custody(candidate: Candidate) -> Diagnostic {
        let upstream = candidate
            .resolved_dynamic_table
            .non_authoritative_resolved_compatibility_fingerprint();
        let exact_upstream = custody_snapshot(&candidate.resolved_dynamic_table);
        let error = validate_candidate(candidate).unwrap_err();
        assert_eq!(
            error
                .candidate
                .resolved_dynamic_table
                .non_authoritative_resolved_compatibility_fingerprint(),
            upstream,
        );
        assert_eq!(
            custody_snapshot(&error.candidate.resolved_dynamic_table),
            exact_upstream,
        );
        error.diagnostic
    }

    fn overwrite_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn both_linux_targets_emit_exact_non_runnable_envelopes() {
        for (target, machine) in [
            (TargetProfile::LinuxX64, EM_X86_64),
            (TargetProfile::LinuxArm64, EM_AARCH64),
        ] {
            let envelope = serialize_elf_dynamic_file_envelope(standard_resolved(target)).unwrap();
            assert_eq!(envelope.header_prefix_bytes().len(), 344);
            assert_eq!(envelope.section_header_table_bytes().len(), 832);
            assert_ne!(
                envelope.non_authoritative_envelope_compatibility_fingerprint(),
                0,
            );
            let header = decode_header(envelope.header_prefix_bytes()).unwrap();
            assert_eq!(header.machine, machine);
            assert_eq!(header.entry, envelope.entry_address());
            assert_eq!(
                header.section_header_offset,
                envelope.section_header_table_file_offset(),
            );
            assert_eq!(
                envelope.section_header_table_bytes(),
                envelope
                    .resolved_dynamic_table()
                    .placed_section_headers()
                    .bytes(),
            );
            assert_eq!(
                decode_program_headers(envelope.header_prefix_bytes())
                    .unwrap()
                    .iter()
                    .map(|header| header.segment_type)
                    .collect::<Vec<_>>(),
                [PT_INTERP, PT_LOAD, PT_LOAD, PT_LOAD, PT_DYNAMIC],
            );
        }
    }

    #[test]
    fn exact_input_and_entry_changes_change_the_envelope() {
        let first = serialize_elf_dynamic_file_envelope(resolved(
            TargetProfile::LinuxX64,
            EntryFixture::Text { offset: 8 },
            b"first_envelope_call",
        ))
        .unwrap();
        let replay = serialize_elf_dynamic_file_envelope(resolved(
            TargetProfile::LinuxX64,
            EntryFixture::Text { offset: 8 },
            b"first_envelope_call",
        ))
        .unwrap();
        let entry_change = serialize_elf_dynamic_file_envelope(resolved(
            TargetProfile::LinuxX64,
            EntryFixture::Text { offset: 12 },
            b"first_envelope_call",
        ))
        .unwrap();
        let import_change = serialize_elf_dynamic_file_envelope(resolved(
            TargetProfile::LinuxX64,
            EntryFixture::Text { offset: 8 },
            b"second_envelope_call",
        ))
        .unwrap();
        assert_eq!(first.header_prefix_bytes(), replay.header_prefix_bytes());
        assert_eq!(
            first.section_header_table_bytes(),
            replay.section_header_table_bytes(),
        );
        assert_ne!(
            first.non_authoritative_envelope_compatibility_fingerprint(),
            entry_change.non_authoritative_envelope_compatibility_fingerprint(),
        );
        assert_ne!(
            first.non_authoritative_envelope_compatibility_fingerprint(),
            import_change.non_authoritative_envelope_compatibility_fingerprint(),
        );
    }

    #[test]
    fn invalid_entry_rejection_returns_the_complete_upstream_owner() {
        for entry in [
            EntryFixture::Missing,
            EntryFixture::Data,
            EntryFixture::TextExplicit {
                offset: 8,
                size: 4,
                kind: SymbolKind::Object,
            },
            EntryFixture::TextExplicit {
                offset: 8,
                size: 0,
                kind: SymbolKind::Function,
            },
            EntryFixture::TextExplicit {
                offset: 31,
                size: 4,
                kind: SymbolKind::Function,
            },
        ] {
            let resolved = resolved(TargetProfile::LinuxX64, entry, b"rejected_entry_call");
            let upstream = resolved.non_authoritative_resolved_compatibility_fingerprint();
            let exact_upstream = custody_snapshot(&resolved);
            let error = serialize_elf_dynamic_file_envelope(resolved).unwrap_err();
            let (returned, diagnostic) = error.into_parts();
            assert_eq!(
                returned.non_authoritative_resolved_compatibility_fingerprint(),
                upstream,
            );
            assert_eq!(custody_snapshot(&returned), exact_upstream);
            assert!(!diagnostic.to_string().is_empty());
        }
    }

    #[test]
    fn exact_header_and_table_substitutions_are_rejected_with_custody() {
        let mut identity = candidate(TargetProfile::LinuxX64);
        identity.contents.header_prefix_bytes[0] ^= 0xff;
        assert_rejected_with_custody(identity);

        let mut target = candidate(TargetProfile::LinuxX64);
        target.contents.header_prefix_bytes[18..20].copy_from_slice(&EM_AARCH64.to_le_bytes());
        assert_rejected_with_custody(target);

        let mut program_header = candidate(TargetProfile::LinuxX64);
        program_header.contents.header_prefix_bytes[ELF64_HEADER_SIZE + 8] ^= 1;
        assert_rejected_with_custody(program_header);

        let mut section_header = candidate(TargetProfile::LinuxX64);
        section_header.contents.section_header_table_bytes[65] ^= 1;
        assert_rejected_with_custody(section_header);

        let mut truncated = candidate(TargetProfile::LinuxX64);
        truncated.contents.header_prefix_bytes.pop();
        assert_rejected_with_custody(truncated);
    }

    #[test]
    fn self_consistent_entry_and_file_offset_substitutions_cannot_escape_replay() {
        let mut entry = candidate(TargetProfile::LinuxX64);
        entry.contents.entry_address += 4;
        overwrite_u64(
            &mut entry.contents.header_prefix_bytes,
            24,
            entry.contents.entry_address,
        );
        entry.non_authoritative_envelope_compatibility_fingerprint =
            non_authoritative_envelope_compatibility_fingerprint(
                &entry.resolved_dynamic_table,
                &entry.contents,
            );
        assert_rejected_with_custody(entry);

        let mut section_offset = candidate(TargetProfile::LinuxX64);
        section_offset.contents.section_header_table_file_offset += 4096;
        overwrite_u64(
            &mut section_offset.contents.header_prefix_bytes,
            40,
            section_offset.contents.section_header_table_file_offset,
        );
        section_offset.non_authoritative_envelope_compatibility_fingerprint =
            non_authoritative_envelope_compatibility_fingerprint(
                &section_offset.resolved_dynamic_table,
                &section_offset.contents,
            );
        assert_rejected_with_custody(section_offset);
    }

    #[test]
    fn section_header_fragment_overlap_and_end_overflow_are_rejected() {
        let mut overlap = candidate(TargetProfile::LinuxX64);
        overlap.contents.section_header_table_file_offset = 128;
        overwrite_u64(
            &mut overlap.contents.header_prefix_bytes,
            40,
            overlap.contents.section_header_table_file_offset,
        );
        overlap.non_authoritative_envelope_compatibility_fingerprint =
            non_authoritative_envelope_compatibility_fingerprint(
                &overlap.resolved_dynamic_table,
                &overlap.contents,
            );
        let diagnostic = assert_rejected_with_custody(overlap);
        assert!(
            diagnostic
                .to_string()
                .contains("overlaps the header prefix")
        );

        let mut load_overlap = candidate(TargetProfile::LinuxX64);
        load_overlap.contents.section_header_table_file_offset = ELF64_HEADER_PREFIX_SIZE as u64;
        overwrite_u64(
            &mut load_overlap.contents.header_prefix_bytes,
            40,
            load_overlap.contents.section_header_table_file_offset,
        );
        load_overlap.non_authoritative_envelope_compatibility_fingerprint =
            non_authoritative_envelope_compatibility_fingerprint(
                &load_overlap.resolved_dynamic_table,
                &load_overlap.contents,
            );
        let diagnostic = assert_rejected_with_custody(load_overlap);
        assert!(
            diagnostic
                .to_string()
                .contains("overlaps a loadable file extent")
        );

        let mut overflow = candidate(TargetProfile::LinuxX64);
        overflow.contents.section_header_table_file_offset = u64::MAX - 100;
        overwrite_u64(
            &mut overflow.contents.header_prefix_bytes,
            40,
            overflow.contents.section_header_table_file_offset,
        );
        overflow.non_authoritative_envelope_compatibility_fingerprint =
            non_authoritative_envelope_compatibility_fingerprint(
                &overflow.resolved_dynamic_table,
                &overflow.contents,
            );
        let diagnostic = assert_rejected_with_custody(overflow);
        assert!(diagnostic.to_string().contains("fragment end overflows"));
    }

    #[test]
    fn compatibility_fingerprint_is_report_only_but_must_replay() {
        let mut zero = candidate(TargetProfile::LinuxX64);
        zero.non_authoritative_envelope_compatibility_fingerprint = 0;
        assert_rejected_with_custody(zero);

        let mut drifted = candidate(TargetProfile::LinuxX64);
        drifted.non_authoritative_envelope_compatibility_fingerprint ^= 1;
        assert_rejected_with_custody(drifted);
    }
}
