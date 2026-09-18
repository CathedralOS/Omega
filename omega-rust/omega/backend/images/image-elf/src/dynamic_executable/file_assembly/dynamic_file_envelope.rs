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
use crate::dynamic_executable::checked::{checked_product, checked_sum, read_u64, require};
use crate::dynamic_executable::load_placement::load_layout::{
    ElfLoadProgramHeader, ElfLoadProgramHeaderKind, ValidatedElfDynamicLoadLayout,
};
use crate::dynamic_executable::load_placement::resolved_dynamic_table::ValidatedElfResolvedDynamicTable;
use crate::entry_symbol::elf_entry_address;
use diagnostics::Diagnostic;
use image::FinalImageSection;
use object_file::SymbolKind;
use target::TargetProfile;

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

fn checked_sum_u64(left: u64, right: u64, context: &'static str) -> Result<u64, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows Elf64_Off")))
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
mod tests;
