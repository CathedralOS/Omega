//! Exact assembly of the closed dynamic ELF fragment roster into one file.
//!
//! This rung consumes the resolved procedure-linkage owner and copies every
//! retained file-backed fragment to its already-validated absolute offset. An
//! explicit placement ledger and an independent byte replay cover the header
//! prefix, source text/data, all twelve non-null section payloads, the section
//! name table, the section-header table, and every zero-filled alignment gap.
//!
//! The assembled bytes remain non-runnable custody. This layer does not mutate
//! the retained `FinalImage`, publish bytes, or grant loader or
//! runnable-image authority.

use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::{
    ElfAppliedProcedureLinkageStorage, ElfAppliedProcedureLinkageTarget,
    ValidatedElfResolvedProcedureLinkage,
};
use crate::dynamic_executable::load_placement::load_layout::{
    ElfPlacedDynamicSectionKind, ValidatedElfDynamicLoadLayout,
};
use crate::dynamic_executable::section_headers::section_roster::ElfDynamicRosterSectionKind;
use crate::imports::ElfImportLocator;
use diagnostics::Diagnostic;
use image::{
    ExecutableImageOutput, FinalDataRegion, FinalDataRegionOrigin, FinalImage,
    PlacedDataRegionInventory, place_data_extent, place_data_regions, place_executable_regions,
};
use target::TargetProfile;

const SECTION_COUNT: usize = 14;
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
    non_authoritative_assembled_file_compatibility_fingerprint: u64,
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
    pub const fn non_authoritative_assembled_file_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_assembled_file_compatibility_fingerprint
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

    let non_authoritative_assembled_file_compatibility_fingerprint =
        assembled.non_authoritative_assembled_file_compatibility_fingerprint;
    let ValidatedElfAssembledDynamicFile {
        resolved_linkage, ..
    } = assembled;
    let mut image = recover_retained_image(resolved_linkage);
    image.memory.text = output.final_text_bytes.clone();
    image.memory.data = output.final_data_bytes.clone();
    debug_assert_eq!(image, expected_image);

    Ok(ValidatedElfDynamicExecutable {
        image,
        output,
        non_authoritative_assembled_file_compatibility_fingerprint,
    })
}

fn derive_executable_output(
    assembled: &ValidatedElfAssembledDynamicFile,
    image: &FinalImage,
) -> Result<ExecutableImageOutput, Diagnostic> {
    let load = load_layout(&assembled.resolved_linkage);
    let format = dynamic_executable_format(load.target())?;
    let executable_regions = place_executable_regions(image, load.final_image_layout())?;
    let data_regions = place_data_regions(image, load.final_image_layout())?;
    let (final_import_data_bytes, import_data_regions) =
        procedure_got_import_custody(&assembled.resolved_linkage, load)?;
    Ok(ExecutableImageOutput {
        bytes: assembled.bytes().to_vec(),
        final_image_layout: load.final_image_layout(),
        final_text_bytes: image.memory.text.clone(),
        final_data_bytes: image.memory.data.clone(),
        final_import_data_bytes,
        file_name: "omega-program".to_owned(),
        format: format.to_owned(),
        text_bytes: image.memory.text.len(),
        data_bytes: image.memory.data.len(),
        bss_bytes: image.memory.bss_size,
        symbols: image.symbol_table.symbols.len(),
        imports: image.symbol_table.imports.len(),
        relocations: image.relocation_table.relocations.len(),
        executable_regions,
        data_regions,
        import_data_regions,
    })
}

/// Declare the writer-owned `.got.plt` import-slot custody the assembled file
/// already carries. Unlike the static lane — which refuses an import rather
/// than emit a slot it cannot bind — the dynamic lane writes one reserved
/// word per bound import under the section's three-word header, so the emitted
/// output surfaces the same evidence shape the PE writer's `.rdata` leg does:
/// the exact extent bytes plus one `ImportBindingSlot` row per slot. The
/// header words stay `unclassified_gaps` under `place_data_extent`.
///
/// Each slot's symbol spelling comes from its canonical versioned locator —
/// the spelling the dynamic linker binds, as `.dynsym`/`.dynstr` carry it —
/// and each slot's section-relative offset comes from the applied `.rela.plt`
/// `r_offset` write that committed the slot's absolute address, so a custody
/// row is grounded in the exact binding the loader consumes rather than in a
/// re-derived layout guess.
fn procedure_got_import_custody(
    resolved_linkage: &ValidatedElfResolvedProcedureLinkage,
    load: &ValidatedElfDynamicLoadLayout,
) -> Result<(Vec<u8>, PlacedDataRegionInventory), Diagnostic> {
    let extent = resolved_linkage.procedure_got_bytes();
    let section = load
        .sections()
        .iter()
        .find(|section| section.kind() == ElfPlacedDynamicSectionKind::ProcedureGot)
        .ok_or_else(|| Diagnostic::error("dynamic ELF load layout has no procedure GOT section"))?;
    require(
        checked_u64(extent.len(), "dynamic ELF procedure GOT extent")? == section.byte_size(),
        "dynamic ELF procedure GOT extent drifted from its placed section",
    )?;
    let base = section.virtual_address().ok_or_else(|| {
        Diagnostic::error("dynamic ELF procedure GOT section has no allocated address")
    })?;

    // Walk the retained custody chain back to the validated linkage plan and
    // the canonical import requests, so a slot row names the exact binding a
    // .rela.plt `JUMP_SLOT` row resolves.
    let linkage = load
        .relative()
        .payloads()
        .section_headers()
        .roster()
        .section_names()
        .dynamic_table()
        .payload()
        .plan()
        .descriptors()
        .templates()
        .linkage();
    let imports = linkage.descriptors().payloads().plan().inputs().imports();

    let mut slot_bindings = std::collections::BTreeMap::new();
    for fixup in resolved_linkage.applied_fixups() {
        if fixup.storage() != ElfAppliedProcedureLinkageStorage::ProcedureRelocation {
            continue;
        }
        let ElfAppliedProcedureLinkageTarget::ProcedureGotSlot { logical_ordinal } = fixup.target()
        else {
            continue;
        };
        if slot_bindings
            .insert(
                logical_ordinal,
                (fixup.target_address(), fixup.byte_width()),
            )
            .is_some()
        {
            return Err(Diagnostic::error(
                "dynamic ELF relocation ledger binds one GOT slot twice",
            ));
        }
    }

    let mut regions = Vec::with_capacity(linkage.contents().slots.len());
    for slot in &linkage.contents().slots {
        let &(slot_address, byte_width) =
            slot_bindings.get(&slot.logical_ordinal).ok_or_else(|| {
                Diagnostic::error("dynamic ELF procedure GOT slot has no binding relocation")
            })?;
        let section_offset = usize::try_from(slot_address.checked_sub(base).ok_or_else(|| {
            Diagnostic::error("dynamic ELF procedure GOT binding lies below its section base")
        })?)
        .map_err(|_| {
            Diagnostic::error("dynamic ELF procedure GOT slot offset exceeds host address space")
        })?;
        let request = imports.get(slot.request_index).ok_or_else(|| {
            Diagnostic::error("dynamic ELF slot names an import outside the canonical requests")
        })?;
        let ElfImportLocator::Versioned { symbol, .. } = &request.locator else {
            return Err(Diagnostic::error(
                "dynamic ELF slot binds a non-versioned import locator",
            ));
        };
        let symbol = String::from_utf8(symbol.clone())
            .map_err(|_| Diagnostic::error("dynamic ELF import symbol spelling is not UTF-8"))?;
        regions.push(FinalDataRegion {
            origin: FinalDataRegionOrigin::ImportBindingSlot,
            section_offset,
            byte_count: usize::from(byte_width),
            symbol,
        });
    }
    let inventory = place_data_extent(extent, regions, base, "dynamic ELF .got.plt")?;
    Ok((extent.to_vec(), inventory))
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
    let expected_data_regions = place_data_regions(image, load.final_image_layout())?;
    let (expected_import_data, expected_import_regions) =
        procedure_got_import_custody(&assembled.resolved_linkage, load)?;
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
            && output.executable_regions == expected_regions
            && output.data_regions == expected_data_regions
            && output.final_import_data_bytes == expected_import_data
            && output.import_data_regions == expected_import_regions,
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
        "dynamic ELF assembly requires the exact fourteen-row section roster",
    )?;

    let mut fragments = Vec::with_capacity(16);
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
        8 => resolved_linkage.procedure_linkage_bytes(),
        9 => resolved_linkage.procedure_got_bytes(),
        10 => resolved_linkage.procedure_relocation_bytes(),
        11 => resolved_linkage.general_relocation_bytes(),
        12 => resolved_linkage.envelope().resolved_dynamic_table().bytes(),
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
        ElfDynamicRosterSectionKind::GnuHash => ElfPlacedDynamicSectionKind::GnuHash,
        ElfDynamicRosterSectionKind::ProcedureLinkage => {
            ElfPlacedDynamicSectionKind::ProcedureLinkage
        }
        ElfDynamicRosterSectionKind::ProcedureGot => ElfPlacedDynamicSectionKind::ProcedureGot,
        ElfDynamicRosterSectionKind::ProcedureRelocation => {
            ElfPlacedDynamicSectionKind::ProcedureRelocation
        }
        ElfDynamicRosterSectionKind::GeneralRelocation => {
            ElfPlacedDynamicSectionKind::GeneralRelocation
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
mod tests;
