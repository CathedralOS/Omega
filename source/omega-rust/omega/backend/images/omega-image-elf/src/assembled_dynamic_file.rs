//! Exact assembly of the closed dynamic ELF fragment roster into one file.
//!
//! This rung consumes the resolved procedure-linkage owner and copies every
//! retained file-backed fragment to its already-validated absolute offset. An
//! explicit placement ledger and an independent byte replay cover the header
//! prefix, source text/data, all eleven non-null section payloads, the section
//! name table, the section-header table, and every zero-filled alignment gap.
//!
//! The assembled bytes remain non-runnable custody. This layer does not mutate
//! the retained `FinalImage`, add `.gnu.hash`, publish bytes, or grant loader or
//! runnable-image authority.

use crate::load_layout::{ElfPlacedDynamicSectionKind, ValidatedElfDynamicLoadLayout};
use crate::resolved_procedure_linkage::ValidatedElfResolvedProcedureLinkage;
use crate::section_roster::ElfDynamicRosterSectionKind;
use omega_image::{ExecutableImageOutput, FinalImage, place_executable_regions};
use omega_target::TargetProfile;
use psi_diagnostics::Diagnostic;

const SECTION_COUNT: usize = 12;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Exact owner of one fragment copied into the assembled dynamic ELF file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfDynamicFileFragmentKind {
    HeaderPrefix,
    SourceText,
    SourceData,
    Section {
        index: u32,
        kind: ElfPlacedDynamicSectionKind,
    },
    SectionHeaderTable,
}

/// One exact fragment placement in the assembled dynamic ELF file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElfDynamicFileFragmentPlacement {
    ordinal: u32,
    kind: ElfDynamicFileFragmentKind,
    file_offset: u64,
    byte_count: u64,
}

impl ElfDynamicFileFragmentPlacement {
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub const fn kind(&self) -> ElfDynamicFileFragmentKind {
        self.kind
    }

    pub const fn file_offset(&self) -> u64 {
        self.file_offset
    }

    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }
}

/// Independently replayed, non-runnable dynamic ELF file bytes.
#[derive(Debug)]
#[must_use = "assembled dynamic ELF bytes retain complete non-runnable linkage custody"]
pub struct ValidatedElfAssembledDynamicFile {
    resolved_linkage: ValidatedElfResolvedProcedureLinkage,
    contents: ElfAssembledDynamicFileContents,
    non_authoritative_assembled_file_compatibility_fingerprint: u64,
}

impl ValidatedElfAssembledDynamicFile {
    pub const fn resolved_linkage(&self) -> &ValidatedElfResolvedProcedureLinkage {
        &self.resolved_linkage
    }

    pub fn bytes(&self) -> &[u8] {
        &self.contents.bytes
    }

    pub fn fragment_placements(&self) -> &[ElfDynamicFileFragmentPlacement] {
        &self.contents.fragment_placements
    }

    /// Compatibility/report coordinate only. A later mutation/admission rung
    /// must replay the exact retained owner, bytes, and placement ledger.
    pub const fn non_authoritative_assembled_file_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_assembled_file_compatibility_fingerprint
    }
}

/// Final admitted dynamic ELF bytes beside the exact consumed and relocated
/// source image. Publication and loader execution remain separate owners.
#[derive(Debug)]
#[must_use = "admitted dynamic ELF retains exact final-image and byte custody"]
pub struct ValidatedElfDynamicExecutable {
    image: FinalImage,
    output: ExecutableImageOutput,
    assembled_file_compatibility_fingerprint: u64,
}

impl ValidatedElfDynamicExecutable {
    pub const fn image(&self) -> &FinalImage {
        &self.image
    }

    pub const fn output(&self) -> &ExecutableImageOutput {
        &self.output
    }

    /// Compatibility/report coordinate only. Exact image and byte replay is
    /// authoritative for this carrier.
    pub const fn assembled_file_compatibility_fingerprint(&self) -> u64 {
        self.assembled_file_compatibility_fingerprint
    }

    pub fn into_parts(self) -> (FinalImage, ExecutableImageOutput) {
        (self.image, self.output)
    }
}

/// Rejected final-byte admission retaining the complete assembled-file owner.
#[derive(Debug)]
#[must_use = "dynamic ELF admission rejection retains assembled-file custody"]
pub struct ElfDynamicExecutableAdmissionError {
    assembled: ValidatedElfAssembledDynamicFile,
    diagnostic: Diagnostic,
}

impl ElfDynamicExecutableAdmissionError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfAssembledDynamicFile, Diagnostic) {
        (self.assembled, self.diagnostic)
    }
}

impl std::fmt::Display for ElfDynamicExecutableAdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfDynamicExecutableAdmissionError {}

/// Rejected file assembly retaining the exact resolved-linkage owner.
#[derive(Debug)]
#[must_use = "dynamic ELF assembly rejection retains resolved-linkage custody"]
pub struct ElfDynamicFileAssemblyError {
    resolved_linkage: ValidatedElfResolvedProcedureLinkage,
    diagnostic: Diagnostic,
}

impl ElfDynamicFileAssemblyError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfResolvedProcedureLinkage, Diagnostic) {
        (self.resolved_linkage, self.diagnostic)
    }
}

impl std::fmt::Display for ElfDynamicFileAssemblyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfDynamicFileAssemblyError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ElfAssembledDynamicFileContents {
    bytes: Vec<u8>,
    fragment_placements: Vec<ElfDynamicFileFragmentPlacement>,
}

#[derive(Clone, Copy)]
struct Fragment<'a> {
    kind: ElfDynamicFileFragmentKind,
    file_offset: u64,
    bytes: &'a [u8],
}

struct Candidate {
    resolved_linkage: ValidatedElfResolvedProcedureLinkage,
    contents: ElfAssembledDynamicFileContents,
    non_authoritative_assembled_file_compatibility_fingerprint: u64,
}

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

/// Assemble every exact file-backed dynamic ELF fragment at its validated
/// absolute file offset without mutating the retained final image.
pub fn assemble_elf_dynamic_file(
    resolved_linkage: ValidatedElfResolvedProcedureLinkage,
) -> Result<ValidatedElfAssembledDynamicFile, Box<ElfDynamicFileAssemblyError>> {
    let contents = match derive_contents(&resolved_linkage) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(ElfDynamicFileAssemblyError {
                resolved_linkage,
                diagnostic,
            }));
        }
    };
    let non_authoritative_assembled_file_compatibility_fingerprint =
        non_authoritative_assembled_file_compatibility_fingerprint(&resolved_linkage, &contents);
    let candidate = Candidate {
        resolved_linkage,
        contents,
        non_authoritative_assembled_file_compatibility_fingerprint,
    };
    validate_candidate(candidate).map_err(|error| {
        Box::new(ElfDynamicFileAssemblyError {
            resolved_linkage: error.candidate.resolved_linkage,
            diagnostic: error.diagnostic,
        })
    })
}

/// Consume one independently replayed dynamic ELF assembly, apply its exact
/// resolved source text to the retained `FinalImage`, and admit the complete
/// final byte image. This grants no publication receipt or execution event.
pub fn admit_elf_dynamic_executable(
    assembled: ValidatedElfAssembledDynamicFile,
) -> Result<ValidatedElfDynamicExecutable, Box<ElfDynamicExecutableAdmissionError>> {
    let mut expected_image = load_layout(&assembled.resolved_linkage)
        .retained_image()
        .clone();
    expected_image.memory.text = assembled.resolved_linkage.source_text_bytes().to_vec();
    let output = match derive_executable_output(&assembled, &expected_image) {
        Ok(output) => output,
        Err(diagnostic) => {
            return Err(Box::new(ElfDynamicExecutableAdmissionError {
                assembled,
                diagnostic,
            }));
        }
    };
    if let Err(diagnostic) = validate_executable_output(&assembled, &expected_image, &output) {
        return Err(Box::new(ElfDynamicExecutableAdmissionError {
            assembled,
            diagnostic,
        }));
    }

    let assembled_file_compatibility_fingerprint =
        assembled.non_authoritative_assembled_file_compatibility_fingerprint;
    let ValidatedElfAssembledDynamicFile {
        resolved_linkage, ..
    } = assembled;
    let mut image = recover_retained_image(resolved_linkage);
    image.memory.text = output.final_text_bytes.clone();
    debug_assert_eq!(image, expected_image);

    Ok(ValidatedElfDynamicExecutable {
        image,
        output,
        assembled_file_compatibility_fingerprint,
    })
}

fn derive_executable_output(
    assembled: &ValidatedElfAssembledDynamicFile,
    image: &FinalImage,
) -> Result<ExecutableImageOutput, Diagnostic> {
    let load = load_layout(&assembled.resolved_linkage);
    let format = dynamic_executable_format(load.target())?;
    let executable_regions = place_executable_regions(image, load.final_image_layout())?;
    Ok(ExecutableImageOutput {
        bytes: assembled.bytes().to_vec(),
        final_text_bytes: image.memory.text.clone(),
        file_name: "omega-program".to_owned(),
        format: format.to_owned(),
        text_bytes: image.memory.text.len(),
        data_bytes: image.memory.data.len(),
        bss_bytes: image.memory.bss_size,
        symbols: image.symbol_table.symbols.len(),
        imports: image.symbol_table.imports.len(),
        relocations: image.relocation_table.relocations.len(),
        executable_regions,
    })
}

fn validate_executable_output(
    assembled: &ValidatedElfAssembledDynamicFile,
    image: &FinalImage,
    output: &ExecutableImageOutput,
) -> Result<(), Diagnostic> {
    validate_contents(&assembled.resolved_linkage, &assembled.contents)?;
    let load = load_layout(&assembled.resolved_linkage);
    let expected_format = dynamic_executable_format(load.target())?;
    let expected_regions = place_executable_regions(image, load.final_image_layout())?;
    require(
        image.target == load.target().native_target()
            && image.memory.text == assembled.resolved_linkage.source_text_bytes()
            && output.bytes == assembled.contents.bytes
            && output.final_text_bytes == image.memory.text
            && output.file_name == "omega-program"
            && output.format == expected_format
            && output.text_bytes == image.memory.text.len()
            && output.data_bytes == image.memory.data.len()
            && output.bss_bytes == image.memory.bss_size
            && output.symbols == image.symbol_table.symbols.len()
            && output.imports == image.symbol_table.imports.len()
            && output.relocations == image.relocation_table.relocations.len()
            && output.executable_regions == expected_regions,
        "admitted dynamic ELF output drifted from exact assembled-file custody",
    )?;
    require(
        output.bytes.starts_with(b"\x7fELF")
            && output.bytes.get(4) == Some(&2)
            && output.bytes.get(5) == Some(&1),
        "admitted dynamic ELF output is not an ELF64-LSB image",
    )
}

fn dynamic_executable_format(target: TargetProfile) -> Result<&'static str, Diagnostic> {
    match target {
        TargetProfile::LinuxX64 => Ok("elf64-x86-64-dynamic-executable"),
        TargetProfile::LinuxArm64 => Ok("elf64-aarch64-dynamic-executable"),
        _ => Err(Diagnostic::error(
            "dynamic ELF admission requires an exact Linux target profile",
        )),
    }
}

fn recover_retained_image(resolved_linkage: ValidatedElfResolvedProcedureLinkage) -> FinalImage {
    let envelope = resolved_linkage.into_envelope();
    let resolved_dynamic = envelope.into_resolved_dynamic_table();
    let (placed_headers, _) = resolved_dynamic.into_parts();
    let (load_layout, _) = placed_headers.into_parts();
    let relative = load_layout.into_relative();
    let (indexed_payloads, _) = relative.into_parts();
    let (section_headers, _) = indexed_payloads.into_parts();
    let (section_roster, _) = section_headers.into_parts();
    let (section_names, _) = section_roster.into_parts();
    let (dynamic_descriptor, _) = section_names.into_parts();
    let (dynamic_payload, _) = dynamic_descriptor.into_parts();
    let (dynamic_tags, _) = dynamic_payload.into_parts();
    let (linkage_descriptors, _) = dynamic_tags.into_parts();
    let (linkage_templates, _) = linkage_descriptors.into_parts();
    let (linkage_relocations, _) = linkage_templates.into_parts();
    let (section_descriptors, _) = linkage_relocations.into_parts();
    let (section_payloads, _) = section_descriptors.into_parts();
    let (section_plan, _) = section_payloads.into_parts();
    let (inputs, _) = section_plan.into_parts();
    let (image, _, _) = inputs.into_parts();
    image
}

fn derive_contents(
    resolved_linkage: &ValidatedElfResolvedProcedureLinkage,
) -> Result<ElfAssembledDynamicFileContents, Diagnostic> {
    let fragments = expected_fragments(resolved_linkage)?;
    let file_byte_count = expected_file_byte_count(resolved_linkage)?;
    let mut bytes = vec![0; file_byte_count];
    let mut occupied = vec![false; file_byte_count];
    let mut fragment_placements = Vec::with_capacity(fragments.len());

    for (ordinal, fragment) in fragments.iter().enumerate() {
        copy_fragment(&mut bytes, &mut occupied, *fragment)?;
        fragment_placements.push(ElfDynamicFileFragmentPlacement {
            ordinal: checked_u32(ordinal, "dynamic ELF fragment ordinal")?,
            kind: fragment.kind,
            file_offset: fragment.file_offset,
            byte_count: checked_u64(fragment.bytes.len(), "dynamic ELF fragment size")?,
        });
    }

    Ok(ElfAssembledDynamicFileContents {
        bytes,
        fragment_placements,
    })
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfAssembledDynamicFile, CandidateValidationError> {
    if let Err(diagnostic) = validate_contents(&candidate.resolved_linkage, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    let expected = non_authoritative_assembled_file_compatibility_fingerprint(
        &candidate.resolved_linkage,
        &candidate.contents,
    );
    if candidate.non_authoritative_assembled_file_compatibility_fingerprint == 0
        || candidate.non_authoritative_assembled_file_compatibility_fingerprint != expected
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error(
                "assembled dynamic ELF compatibility fingerprint does not replay",
            ),
        });
    }
    Ok(ValidatedElfAssembledDynamicFile {
        resolved_linkage: candidate.resolved_linkage,
        contents: candidate.contents,
        non_authoritative_assembled_file_compatibility_fingerprint: candidate
            .non_authoritative_assembled_file_compatibility_fingerprint,
    })
}

fn validate_contents(
    resolved_linkage: &ValidatedElfResolvedProcedureLinkage,
    contents: &ElfAssembledDynamicFileContents,
) -> Result<(), Diagnostic> {
    let expected_fragments = expected_fragments(resolved_linkage)?;
    let expected_file_byte_count = expected_file_byte_count(resolved_linkage)?;
    require(
        contents.bytes.len() == expected_file_byte_count,
        "assembled dynamic ELF file length drifted from its section-header extent",
    )?;
    require(
        contents.fragment_placements.len() == expected_fragments.len(),
        "assembled dynamic ELF fragment ledger coverage drifted",
    )?;

    let mut occupied = vec![false; contents.bytes.len()];
    for (ordinal, (placement, fragment)) in contents
        .fragment_placements
        .iter()
        .zip(expected_fragments)
        .enumerate()
    {
        let expected_ordinal = checked_u32(ordinal, "dynamic ELF fragment ordinal")?;
        let expected_byte_count = checked_u64(fragment.bytes.len(), "dynamic ELF fragment size")?;
        require(
            placement.ordinal == expected_ordinal
                && placement.kind == fragment.kind
                && placement.file_offset == fragment.file_offset
                && placement.byte_count == expected_byte_count,
            "assembled dynamic ELF fragment ledger drifted from exact source custody",
        )?;
        let range = fragment_range(
            fragment.file_offset,
            fragment.bytes.len(),
            contents.bytes.len(),
        )?;
        require(
            occupied[range.clone()].iter().all(|byte| !*byte),
            "assembled dynamic ELF fragments overlap",
        )?;
        require(
            contents.bytes[range.clone()] == *fragment.bytes,
            "assembled dynamic ELF fragment bytes do not replay",
        )?;
        occupied[range].fill(true);
    }

    require(
        contents
            .bytes
            .iter()
            .zip(occupied)
            .all(|(byte, occupied)| occupied || *byte == 0),
        "assembled dynamic ELF alignment padding is not zero-filled",
    )?;
    validate_file_extents(resolved_linkage, contents.bytes.len())
}

fn expected_fragments(
    resolved_linkage: &ValidatedElfResolvedProcedureLinkage,
) -> Result<Vec<Fragment<'_>>, Diagnostic> {
    let envelope = resolved_linkage.envelope();
    let load = load_layout(resolved_linkage);
    let indexed = load.relative().payloads().contents();
    require(
        indexed.rows.len() == SECTION_COUNT && load.sections().len() == SECTION_COUNT,
        "dynamic ELF assembly requires the exact twelve-row section roster",
    )?;

    let mut fragments = Vec::with_capacity(15);
    fragments.push(Fragment {
        kind: ElfDynamicFileFragmentKind::HeaderPrefix,
        file_offset: 0,
        bytes: envelope.header_prefix_bytes(),
    });
    fragments.push(Fragment {
        kind: ElfDynamicFileFragmentKind::SourceText,
        file_offset: load.image_memory().text_file_offset(),
        bytes: resolved_linkage.source_text_bytes(),
    });
    fragments.push(Fragment {
        kind: ElfDynamicFileFragmentKind::SourceData,
        file_offset: load.image_memory().data_file_offset(),
        bytes: &load.retained_image().memory.data,
    });

    for (index, (row, placed)) in indexed.rows.iter().zip(load.sections()).enumerate() {
        let expected_index = checked_u32(index, "dynamic ELF section index")?;
        require(
            row.index == expected_index
                && placed.index() == expected_index
                && public_section_kind(row.kind) == placed.kind(),
            "dynamic ELF assembly section roster drifted",
        )?;
        if index == 0 {
            require(
                placed.kind() == ElfPlacedDynamicSectionKind::Null
                    && placed.byte_size() == 0
                    && row.bytes.is_empty(),
                "dynamic ELF null section acquired file bytes",
            )?;
            continue;
        }
        let bytes = section_bytes(resolved_linkage, index, &row.bytes);
        require(
            checked_u64(bytes.len(), "dynamic ELF section payload size")? == placed.byte_size(),
            "dynamic ELF assembled section size drifted from absolute placement",
        )?;
        fragments.push(Fragment {
            kind: ElfDynamicFileFragmentKind::Section {
                index: expected_index,
                kind: placed.kind(),
            },
            file_offset: placed.file_offset(),
            bytes,
        });
    }

    fragments.push(Fragment {
        kind: ElfDynamicFileFragmentKind::SectionHeaderTable,
        file_offset: envelope.section_header_table_file_offset(),
        bytes: envelope.section_header_table_bytes(),
    });
    fragments.sort_by_key(|fragment| (fragment.file_offset, fragment_kind_order(fragment.kind)));
    Ok(fragments)
}

fn section_bytes<'a>(
    resolved_linkage: &'a ValidatedElfResolvedProcedureLinkage,
    index: usize,
    indexed_bytes: &'a [u8],
) -> &'a [u8] {
    match index {
        7 => resolved_linkage.procedure_linkage_bytes(),
        8 => resolved_linkage.procedure_got_bytes(),
        9 => resolved_linkage.procedure_relocation_bytes(),
        10 => resolved_linkage.envelope().resolved_dynamic_table().bytes(),
        _ => indexed_bytes,
    }
}

fn copy_fragment(
    destination: &mut [u8],
    occupied: &mut [bool],
    fragment: Fragment<'_>,
) -> Result<(), Diagnostic> {
    let range = fragment_range(
        fragment.file_offset,
        fragment.bytes.len(),
        destination.len(),
    )?;
    require(
        occupied[range.clone()].iter().all(|byte| !*byte),
        "dynamic ELF file fragments overlap during assembly",
    )?;
    destination[range.clone()].copy_from_slice(fragment.bytes);
    occupied[range].fill(true);
    Ok(())
}

fn validate_file_extents(
    resolved_linkage: &ValidatedElfResolvedProcedureLinkage,
    file_byte_count: usize,
) -> Result<(), Diagnostic> {
    let load = load_layout(resolved_linkage);
    let file_byte_count = checked_u64(file_byte_count, "assembled dynamic ELF file size")?;
    for header in load.program_headers() {
        let end = checked_sum(
            header.file_offset(),
            header.file_size(),
            "dynamic ELF program-header file extent",
        )?;
        require(
            end <= file_byte_count,
            "assembled dynamic ELF file does not contain a complete program-header extent",
        )?;
    }
    let image = load.image_memory();
    let data_end = checked_sum(
        image.data_file_offset(),
        image.data_size(),
        "assembled source-data extent",
    )?;
    require(
        data_end <= file_byte_count
            && checked_u64(load.retained_image().memory.text.len(), "source text size")?
                == image.text_size()
            && checked_u64(load.retained_image().memory.data.len(), "source data size")?
                == image.data_size(),
        "assembled dynamic ELF source-memory extent drifted",
    )
}

fn expected_file_byte_count(
    resolved_linkage: &ValidatedElfResolvedProcedureLinkage,
) -> Result<usize, Diagnostic> {
    let envelope = resolved_linkage.envelope();
    let end = checked_sum(
        envelope.section_header_table_file_offset(),
        checked_u64(
            envelope.section_header_table_bytes().len(),
            "dynamic ELF section-header table size",
        )?,
        "dynamic ELF file end",
    )?;
    usize::try_from(end)
        .map_err(|_| Diagnostic::error("dynamic ELF assembled file exceeds host address space"))
}

fn fragment_range(
    file_offset: u64,
    byte_count: usize,
    file_byte_count: usize,
) -> Result<std::ops::Range<usize>, Diagnostic> {
    let start = usize::try_from(file_offset)
        .map_err(|_| Diagnostic::error("dynamic ELF fragment offset exceeds host address space"))?;
    let end = start.checked_add(byte_count).ok_or_else(|| {
        Diagnostic::error("dynamic ELF fragment extent overflows host address space")
    })?;
    require(
        end <= file_byte_count,
        "dynamic ELF fragment exceeds the assembled file extent",
    )?;
    Ok(start..end)
}

fn load_layout(
    resolved_linkage: &ValidatedElfResolvedProcedureLinkage,
) -> &ValidatedElfDynamicLoadLayout {
    resolved_linkage
        .envelope()
        .resolved_dynamic_table()
        .placed_section_headers()
        .load_layout()
}

const fn public_section_kind(kind: ElfDynamicRosterSectionKind) -> ElfPlacedDynamicSectionKind {
    match kind {
        ElfDynamicRosterSectionKind::Null => ElfPlacedDynamicSectionKind::Null,
        ElfDynamicRosterSectionKind::Interpreter => ElfPlacedDynamicSectionKind::Interpreter,
        ElfDynamicRosterSectionKind::DynamicString => ElfPlacedDynamicSectionKind::DynamicString,
        ElfDynamicRosterSectionKind::DynamicSymbol => ElfPlacedDynamicSectionKind::DynamicSymbol,
        ElfDynamicRosterSectionKind::SystemVHash => ElfPlacedDynamicSectionKind::SystemVHash,
        ElfDynamicRosterSectionKind::GnuSymbolVersion => {
            ElfPlacedDynamicSectionKind::GnuSymbolVersion
        }
        ElfDynamicRosterSectionKind::GnuVersionRequirement => {
            ElfPlacedDynamicSectionKind::GnuVersionRequirement
        }
        ElfDynamicRosterSectionKind::ProcedureLinkage => {
            ElfPlacedDynamicSectionKind::ProcedureLinkage
        }
        ElfDynamicRosterSectionKind::ProcedureGot => ElfPlacedDynamicSectionKind::ProcedureGot,
        ElfDynamicRosterSectionKind::ProcedureRelocation => {
            ElfPlacedDynamicSectionKind::ProcedureRelocation
        }
        ElfDynamicRosterSectionKind::DynamicTable => ElfPlacedDynamicSectionKind::DynamicTable,
        ElfDynamicRosterSectionKind::SectionNameTable => {
            ElfPlacedDynamicSectionKind::SectionNameTable
        }
    }
}

const fn fragment_kind_order(kind: ElfDynamicFileFragmentKind) -> u8 {
    match kind {
        ElfDynamicFileFragmentKind::HeaderPrefix => 0,
        ElfDynamicFileFragmentKind::SourceText => 1,
        ElfDynamicFileFragmentKind::Section { .. } => 2,
        ElfDynamicFileFragmentKind::SourceData => 3,
        ElfDynamicFileFragmentKind::SectionHeaderTable => 4,
    }
}

fn non_authoritative_assembled_file_compatibility_fingerprint(
    resolved_linkage: &ValidatedElfResolvedProcedureLinkage,
    contents: &ElfAssembledDynamicFileContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(
        &resolved_linkage
            .non_authoritative_resolved_linkage_compatibility_fingerprint()
            .to_le_bytes(),
    );
    hash.bytes(&contents.bytes);
    hash.bytes(&(contents.fragment_placements.len() as u64).to_le_bytes());
    for placement in &contents.fragment_placements {
        hash.bytes(&placement.ordinal.to_le_bytes());
        hash_fragment_kind(&mut hash, placement.kind);
        hash.bytes(&placement.file_offset.to_le_bytes());
        hash.bytes(&placement.byte_count.to_le_bytes());
    }
    hash.finish()
}

fn hash_fragment_kind(hash: &mut Fnv1a, kind: ElfDynamicFileFragmentKind) {
    match kind {
        ElfDynamicFileFragmentKind::HeaderPrefix => hash.byte(1),
        ElfDynamicFileFragmentKind::SourceText => hash.byte(2),
        ElfDynamicFileFragmentKind::SourceData => hash.byte(3),
        ElfDynamicFileFragmentKind::Section { index, kind } => {
            hash.byte(4);
            hash.bytes(&index.to_le_bytes());
            hash.byte(kind as u8);
        }
        ElfDynamicFileFragmentKind::SectionHeaderTable => hash.byte(5),
    }
}

fn checked_u32(value: usize, context: &str) -> Result<u32, Diagnostic> {
    u32::try_from(value).map_err(|_| Diagnostic::error(format!("{context} exceeds u32")))
}

fn checked_u64(value: usize, context: &str) -> Result<u64, Diagnostic> {
    u64::try_from(value).map_err(|_| Diagnostic::error(format!("{context} exceeds Elf64_Xword")))
}

fn checked_sum(left: u64, right: u64, context: &str) -> Result<u64, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows Elf64_Xword")))
}

fn require(condition: bool, message: &str) -> Result<(), Diagnostic> {
    if condition {
        Ok(())
    } else {
        Err(Diagnostic::error(message))
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
        apply_elf_dynamic_address_fixups, apply_elf_procedure_linkage_fixups,
        apply_elf_section_header_placements, plan_elf_dynamic_link_inputs,
        plan_elf_dynamic_load_layout, plan_elf_dynamic_section_descriptors,
        plan_elf_dynamic_section_roster, plan_elf_dynamic_sections,
        plan_elf_dynamic_table_section_descriptor, plan_elf_dynamic_tags,
        plan_elf_indexed_section_payloads, plan_elf_procedure_linkage_relocations,
        plan_elf_procedure_linkage_section_descriptors, plan_elf_procedure_linkage_templates,
        plan_elf_relative_section_payload_layout, plan_elf_section_name_table,
        serialize_elf_dynamic_file_envelope, serialize_elf_dynamic_sections,
        serialize_elf_dynamic_table, serialize_elf_section_header_table,
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

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct ResolvedCustodySnapshot {
        image: FinalImage,
        header_prefix: Vec<u8>,
        section_headers: Vec<u8>,
        resolved_dynamic: Vec<u8>,
        source_text: Vec<u8>,
        procedure_linkage: Vec<u8>,
        procedure_got: Vec<u8>,
        procedure_relocation: Vec<u8>,
        applications: Vec<crate::ElfAppliedProcedureLinkageFixup>,
    }

    fn standard_resolved_linkage(target: TargetProfile) -> ValidatedElfResolvedProcedureLinkage {
        resolved_linkage(target, [b"alpha_call", b"beta_call"])
    }

    fn resolved_linkage(
        target: TargetProfile,
        imported_symbols: [&[u8]; 2],
    ) -> ValidatedElfResolvedProcedureLinkage {
        let mut image = FinalImage::with_capacity(
            target.native_target(),
            FinalImageMemory {
                text: vec![0; 64],
                data: (0_u8..13).map(|byte| byte.wrapping_add(0x50)).collect(),
                bss_size: 23,
                bss_alignment: 16,
            },
            Handle::invalid(),
            3,
            2,
            3,
        );
        let entry = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "_start".to_owned(),
            section: FinalImageSection::Text,
            offset: 24,
            size: 4,
            kind: SymbolKind::Function,
        });
        image.symbol_table.entry_symbol = entry;

        let mut imports = Vec::new();
        for (index, symbol) in imported_symbols.into_iter().enumerate() {
            let handle = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: format!("__omega_assembled_import_{index}"),
                section: FinalImageSection::None,
                offset: 0,
                size: 0,
                kind: SymbolKind::Import,
            });
            image.symbol_table.imports.insert(FinalImageImport {
                symbol_handle: handle,
                import: FinalImageImportPlan::Normalized(
                    normalize_foreign_locator(
                        ForeignLocatorCandidate::ElfVersioned {
                            object: b"libassembled-file.so".to_vec(),
                            symbol: symbol.to_vec(),
                            version: b"ASSEMBLED_FILE_1".to_vec(),
                        },
                        target,
                    )
                    .unwrap(),
                ),
            });
            imports.push(handle);
        }
        for (instruction_offset, symbol_handle) in
            [(0, imports[0]), (8, imports[0]), (16, imports[1])]
        {
            let (relocation_offset, kind) = match target {
                TargetProfile::LinuxX64 => {
                    image.memory.text[instruction_offset] = 0xe8;
                    (instruction_offset + 1, RelocationKind::X86_64Relative32)
                }
                TargetProfile::LinuxArm64 => {
                    image.memory.text[instruction_offset..instruction_offset + 4]
                        .copy_from_slice(&[0, 0, 0, 0x94]);
                    (instruction_offset, RelocationKind::Aarch64Branch26)
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
        let resolved = apply_elf_dynamic_address_fixups(placed).unwrap();
        let envelope = serialize_elf_dynamic_file_envelope(resolved).unwrap();
        apply_elf_procedure_linkage_fixups(envelope).unwrap()
    }

    fn candidate(target: TargetProfile) -> Candidate {
        let resolved_linkage = standard_resolved_linkage(target);
        let contents = derive_contents(&resolved_linkage).unwrap();
        let non_authoritative_assembled_file_compatibility_fingerprint =
            non_authoritative_assembled_file_compatibility_fingerprint(
                &resolved_linkage,
                &contents,
            );
        Candidate {
            resolved_linkage,
            contents,
            non_authoritative_assembled_file_compatibility_fingerprint,
        }
    }

    fn refresh_fingerprint(candidate: &mut Candidate) {
        candidate.non_authoritative_assembled_file_compatibility_fingerprint =
            non_authoritative_assembled_file_compatibility_fingerprint(
                &candidate.resolved_linkage,
                &candidate.contents,
            );
    }

    fn custody_snapshot(
        resolved_linkage: &ValidatedElfResolvedProcedureLinkage,
    ) -> ResolvedCustodySnapshot {
        let envelope = resolved_linkage.envelope();
        ResolvedCustodySnapshot {
            image: load_layout(resolved_linkage).retained_image().clone(),
            header_prefix: envelope.header_prefix_bytes().to_vec(),
            section_headers: envelope.section_header_table_bytes().to_vec(),
            resolved_dynamic: envelope.resolved_dynamic_table().bytes().to_vec(),
            source_text: resolved_linkage.source_text_bytes().to_vec(),
            procedure_linkage: resolved_linkage.procedure_linkage_bytes().to_vec(),
            procedure_got: resolved_linkage.procedure_got_bytes().to_vec(),
            procedure_relocation: resolved_linkage.procedure_relocation_bytes().to_vec(),
            applications: resolved_linkage.applied_fixups().to_vec(),
        }
    }

    fn assert_rejected_with_exact_custody(candidate: Candidate) -> Diagnostic {
        let compact = candidate
            .resolved_linkage
            .non_authoritative_resolved_linkage_compatibility_fingerprint();
        let exact = custody_snapshot(&candidate.resolved_linkage);
        let error = validate_candidate(candidate).unwrap_err();
        assert_eq!(
            error
                .candidate
                .resolved_linkage
                .non_authoritative_resolved_linkage_compatibility_fingerprint(),
            compact,
        );
        assert_eq!(custody_snapshot(&error.candidate.resolved_linkage), exact);
        error.diagnostic
    }

    fn byte_range(placement: &ElfDynamicFileFragmentPlacement) -> std::ops::Range<usize> {
        let start = usize::try_from(placement.file_offset()).unwrap();
        let count = usize::try_from(placement.byte_count()).unwrap();
        start..start + count
    }

    #[test]
    fn both_linux_targets_assemble_every_exact_fragment_and_zero_gap_without_bss_bytes() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let resolved = standard_resolved_linkage(target);
            let image_before = load_layout(&resolved).retained_image().clone();
            let assembled = assemble_elf_dynamic_file(resolved).unwrap();
            let resolved = assembled.resolved_linkage();
            let load = load_layout(resolved);
            let envelope = resolved.envelope();

            assert_eq!(assembled.fragment_placements().len(), 15);
            assert_eq!(
                assembled.bytes().len(),
                usize::try_from(envelope.section_header_table_file_offset()).unwrap()
                    + envelope.section_header_table_bytes().len(),
            );
            assert_ne!(
                assembled.non_authoritative_assembled_file_compatibility_fingerprint(),
                0,
            );
            assert_eq!(load.retained_image(), &image_before);

            let indexed = load.relative().payloads().contents();
            let mut occupied = vec![false; assembled.bytes().len()];
            for (ordinal, placement) in assembled.fragment_placements().iter().enumerate() {
                assert_eq!(placement.ordinal(), ordinal as u32);
                let (expected_offset, expected) = match placement.kind() {
                    ElfDynamicFileFragmentKind::HeaderPrefix => (0, envelope.header_prefix_bytes()),
                    ElfDynamicFileFragmentKind::SourceText => (
                        load.image_memory().text_file_offset(),
                        resolved.source_text_bytes(),
                    ),
                    ElfDynamicFileFragmentKind::SourceData => (
                        load.image_memory().data_file_offset(),
                        load.retained_image().memory.data.as_slice(),
                    ),
                    ElfDynamicFileFragmentKind::Section { index, kind } => {
                        let placed = &load.sections()[index as usize];
                        assert_eq!(placed.index(), index);
                        assert_eq!(placed.kind(), kind);
                        (
                            placed.file_offset(),
                            section_bytes(
                                resolved,
                                index as usize,
                                &indexed.rows[index as usize].bytes,
                            ),
                        )
                    }
                    ElfDynamicFileFragmentKind::SectionHeaderTable => (
                        envelope.section_header_table_file_offset(),
                        envelope.section_header_table_bytes(),
                    ),
                };
                assert_eq!(placement.file_offset(), expected_offset);
                let range = byte_range(placement);
                assert_eq!(range.len(), expected.len());
                assert_eq!(&assembled.bytes()[range.clone()], expected);
                assert!(occupied[range.clone()].iter().all(|occupied| !occupied));
                occupied[range].fill(true);
            }
            assert!(
                assembled
                    .bytes()
                    .iter()
                    .zip(&occupied)
                    .all(|(byte, occupied)| *occupied || *byte == 0),
            );

            let memory = load.image_memory();
            let read_write = load
                .program_headers()
                .iter()
                .find(|header| header.kind() == crate::ElfLoadProgramHeaderKind::LoadReadWrite)
                .unwrap();
            assert!(read_write.memory_size() > read_write.file_size());
            assert!(
                memory.bss_virtual_address()
                    >= read_write.virtual_address() + read_write.file_size()
            );
            assert!(
                memory.bss_virtual_address() + memory.bss_size()
                    <= read_write.virtual_address() + read_write.memory_size()
            );
            assert!(
                assembled
                    .fragment_placements()
                    .iter()
                    .all(|placement| !matches!(
                        placement.kind(),
                        ElfDynamicFileFragmentKind::Section { index: 0, .. }
                    ))
            );
        }
    }

    #[test]
    fn assembly_is_deterministic_and_bound_to_target_and_exact_imports() {
        let first =
            assemble_elf_dynamic_file(standard_resolved_linkage(TargetProfile::LinuxX64)).unwrap();
        let replay =
            assemble_elf_dynamic_file(standard_resolved_linkage(TargetProfile::LinuxX64)).unwrap();
        let target_change =
            assemble_elf_dynamic_file(standard_resolved_linkage(TargetProfile::LinuxArm64))
                .unwrap();
        let import_change = assemble_elf_dynamic_file(resolved_linkage(
            TargetProfile::LinuxX64,
            [b"alpha_call", b"gamma_call"],
        ))
        .unwrap();
        assert_eq!(first.bytes(), replay.bytes());
        assert_eq!(first.fragment_placements(), replay.fragment_placements());
        assert_eq!(
            first.non_authoritative_assembled_file_compatibility_fingerprint(),
            replay.non_authoritative_assembled_file_compatibility_fingerprint(),
        );
        assert_ne!(first.bytes(), target_change.bytes());
        assert_ne!(first.bytes(), import_change.bytes());
        assert_ne!(
            first.non_authoritative_assembled_file_compatibility_fingerprint(),
            target_change.non_authoritative_assembled_file_compatibility_fingerprint(),
        );
        assert_ne!(
            first.non_authoritative_assembled_file_compatibility_fingerprint(),
            import_change.non_authoritative_assembled_file_compatibility_fingerprint(),
        );
    }

    #[test]
    fn every_fragment_and_one_alignment_gap_mutation_reject_with_exact_custody() {
        let placements = candidate(TargetProfile::LinuxX64)
            .contents
            .fragment_placements
            .clone();
        for placement in placements {
            let mut changed = candidate(TargetProfile::LinuxX64);
            let range = byte_range(&placement);
            assert!(!range.is_empty());
            changed.contents.bytes[range.start] ^= 1;
            refresh_fingerprint(&mut changed);
            let diagnostic = assert_rejected_with_exact_custody(changed);
            assert!(
                diagnostic
                    .to_string()
                    .contains("fragment bytes do not replay")
            );
        }

        let mut padding = candidate(TargetProfile::LinuxX64);
        let mut occupied = vec![false; padding.contents.bytes.len()];
        for placement in &padding.contents.fragment_placements {
            occupied[byte_range(placement)].fill(true);
        }
        let gap = occupied
            .iter()
            .position(|occupied| !occupied)
            .expect("fixture must contain alignment padding");
        padding.contents.bytes[gap] = 1;
        refresh_fingerprint(&mut padding);
        let diagnostic = assert_rejected_with_exact_custody(padding);
        assert!(
            diagnostic
                .to_string()
                .contains("padding is not zero-filled")
        );
    }

    #[test]
    fn truncation_append_and_ledger_substitutions_reject_with_exact_custody() {
        let mut truncated = candidate(TargetProfile::LinuxArm64);
        truncated.contents.bytes.pop();
        refresh_fingerprint(&mut truncated);
        assert!(
            assert_rejected_with_exact_custody(truncated)
                .to_string()
                .contains("file length drifted"),
        );

        let mut appended = candidate(TargetProfile::LinuxArm64);
        appended.contents.bytes.push(0);
        refresh_fingerprint(&mut appended);
        assert!(
            assert_rejected_with_exact_custody(appended)
                .to_string()
                .contains("file length drifted"),
        );

        let mut missing = candidate(TargetProfile::LinuxArm64);
        missing.contents.fragment_placements.pop();
        refresh_fingerprint(&mut missing);
        assert!(
            assert_rejected_with_exact_custody(missing)
                .to_string()
                .contains("ledger coverage drifted"),
        );

        let mut reordered = candidate(TargetProfile::LinuxArm64);
        reordered.contents.fragment_placements.swap(1, 2);
        refresh_fingerprint(&mut reordered);
        assert!(
            assert_rejected_with_exact_custody(reordered)
                .to_string()
                .contains("ledger drifted"),
        );

        let mut kind = candidate(TargetProfile::LinuxArm64);
        kind.contents.fragment_placements[0].kind = ElfDynamicFileFragmentKind::SourceData;
        refresh_fingerprint(&mut kind);
        assert_rejected_with_exact_custody(kind);

        let mut offset = candidate(TargetProfile::LinuxArm64);
        offset.contents.fragment_placements[1].file_offset += 1;
        refresh_fingerprint(&mut offset);
        assert_rejected_with_exact_custody(offset);

        let mut count = candidate(TargetProfile::LinuxArm64);
        count.contents.fragment_placements[1].byte_count -= 1;
        refresh_fingerprint(&mut count);
        assert_rejected_with_exact_custody(count);

        let mut ordinal = candidate(TargetProfile::LinuxArm64);
        ordinal.contents.fragment_placements[1].ordinal += 1;
        refresh_fingerprint(&mut ordinal);
        assert_rejected_with_exact_custody(ordinal);
    }

    #[test]
    fn report_fingerprint_zero_or_drift_rejects_with_exact_custody() {
        let mut zero = candidate(TargetProfile::LinuxX64);
        zero.non_authoritative_assembled_file_compatibility_fingerprint = 0;
        assert!(
            assert_rejected_with_exact_custody(zero)
                .to_string()
                .contains("fingerprint does not replay"),
        );

        let mut drift = candidate(TargetProfile::LinuxX64);
        drift.non_authoritative_assembled_file_compatibility_fingerprint ^= 1;
        assert!(
            assert_rejected_with_exact_custody(drift)
                .to_string()
                .contains("fingerprint does not replay"),
        );
    }

    #[test]
    fn both_linux_targets_consume_the_retained_image_into_exact_admitted_bytes() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let resolved = standard_resolved_linkage(target);
            let original = load_layout(&resolved).retained_image().clone();
            let resolved_text = resolved.source_text_bytes().to_vec();
            assert_ne!(original.memory.text, resolved_text);
            let assembled = assemble_elf_dynamic_file(resolved).unwrap();
            let assembled_bytes = assembled.bytes().to_vec();
            let assembled_fingerprint =
                assembled.non_authoritative_assembled_file_compatibility_fingerprint();

            let admitted = admit_elf_dynamic_executable(assembled).unwrap();
            assert_eq!(admitted.image().memory.text, resolved_text);
            assert_eq!(admitted.image().memory.data, original.memory.data);
            assert_eq!(admitted.image().memory.bss_size, original.memory.bss_size);
            assert_eq!(admitted.output().bytes, assembled_bytes);
            assert_eq!(admitted.output().final_text_bytes, resolved_text);
            assert_eq!(admitted.output().imports, 2);
            assert_eq!(admitted.output().relocations, 3);
            assert_eq!(
                admitted.assembled_file_compatibility_fingerprint(),
                assembled_fingerprint,
            );
            assert!(admitted.output().bytes.starts_with(b"\x7fELF"));
            assert!(admitted.output().format.contains("dynamic-executable"));
        }
    }

    #[test]
    fn final_byte_admission_rejects_independent_output_drift() {
        let assembled =
            assemble_elf_dynamic_file(standard_resolved_linkage(TargetProfile::LinuxX64)).unwrap();
        let mut image = load_layout(&assembled.resolved_linkage)
            .retained_image()
            .clone();
        image.memory.text = assembled.resolved_linkage.source_text_bytes().to_vec();
        let output = derive_executable_output(&assembled, &image).unwrap();
        validate_executable_output(&assembled, &image, &output).unwrap();

        let mut bytes = output.clone();
        bytes.bytes[0] ^= 1;
        assert!(validate_executable_output(&assembled, &image, &bytes).is_err());

        let mut text = output.clone();
        text.final_text_bytes[0] ^= 1;
        assert!(validate_executable_output(&assembled, &image, &text).is_err());

        let mut format = output.clone();
        format.format.push_str("-drift");
        assert!(validate_executable_output(&assembled, &image, &format).is_err());

        let mut statistics = output;
        statistics.imports += 1;
        assert!(validate_executable_output(&assembled, &image, &statistics).is_err());
    }
}
