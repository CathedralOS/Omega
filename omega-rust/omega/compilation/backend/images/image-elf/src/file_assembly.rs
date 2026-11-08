/*
We execute stages 19 through 22 of the dynamic ELF pipeline: serializing the
ELF64 header and program-header envelope (`serialize_elf_dynamic_file_envelope`),
applying `.plt`, `.got.plt`, and image-local relocations
(`apply_elf_procedure_linkage_fixups`), assembling the complete container file
(`assemble_elf_dynamic_file`), and re-verifying final-byte admission
(`admit_elf_dynamic_executable`).
*/

pub(crate) mod assembled_dynamic_file {
    use target::compatibility_fingerprint::Fnv1a;

    use super::super::checked::{self, CandidateValidationError};
    use crate::constants::SECTION_COUNT;
    use crate::dynamic_executable::checked::{checked_sum_u64, checked_u32, require};
    use crate::dynamic_executable::checked_u64;
    use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::{
        ElfAppliedProcedureLinkageStorage, ElfAppliedProcedureLinkageTarget,
        ValidatedElfResolvedProcedureLinkage,
    };
    use crate::dynamic_executable::load_placement::load_layout::{
        ElfPlacedDynamicSectionKind, ValidatedElfDynamicLoadLayout, public_section_kind,
    };
    use crate::imports::ElfImportLocator;
    use diagnostics::Diagnostic;
    use image::{
        ExecutableImageOutput, FinalDataRegion, FinalDataRegionOrigin, FinalImage,
        PlacedDataRegionInventory, place_data_extent, place_executable_extent,
    };
    #[cfg(test)]
    use image::{place_data_regions, place_executable_regions};
    use target::TargetProfile;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ElfDynamicFileFragmentKind {
        HeaderPrefix,
        SourceText,
        SourceData,
        Section {
            index: u32,
            kind: ElfPlacedDynamicSectionKind,
        },
        SectionHeaderTable,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfDynamicFileFragmentPlacement {
        ordinal: u32,
        kind: ElfDynamicFileFragmentKind,
        file_offset: u64,
        byte_count: u64,
    }

    impl ElfDynamicFileFragmentPlacement {
        #[cfg(test)]
        pub const fn ordinal(&self) -> u32 {
            self.ordinal
        }

        #[cfg(test)]
        pub const fn kind(&self) -> ElfDynamicFileFragmentKind {
            self.kind
        }

        #[cfg(test)]
        pub const fn file_offset(&self) -> u64 {
            self.file_offset
        }

        #[cfg(test)]
        pub const fn byte_count(&self) -> u64 {
            self.byte_count
        }
    }

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

        #[cfg(test)]
        pub(crate) fn fragment_placements(&self) -> &[ElfDynamicFileFragmentPlacement] {
            &self.contents.fragment_placements
        }

        pub const fn non_authoritative_assembled_file_compatibility_fingerprint(&self) -> u64 {
            self.non_authoritative_assembled_file_compatibility_fingerprint
        }
    }

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

        pub const fn non_authoritative_assembled_file_compatibility_fingerprint(&self) -> u64 {
            self.non_authoritative_assembled_file_compatibility_fingerprint
        }

        pub fn into_parts(self) -> (FinalImage, ExecutableImageOutput) {
            (self.image, self.output)
        }
    }

    #[derive(Debug)]
    #[must_use = "dynamic ELF admission rejection retains assembled-file custody"]
    pub struct ElfDynamicExecutableAdmissionError {
        assembled: ValidatedElfAssembledDynamicFile,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(
        ElfDynamicExecutableAdmissionError;
        assembled: ValidatedElfAssembledDynamicFile,
    );

    #[derive(Debug)]
    #[must_use = "dynamic ELF assembly rejection retains resolved-linkage custody"]
    pub struct ElfDynamicFileAssemblyError {
        resolved_linkage: ValidatedElfResolvedProcedureLinkage,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(
        ElfDynamicFileAssemblyError;
        resolved_linkage: ValidatedElfResolvedProcedureLinkage,
    );

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

    pub(crate) fn assemble_elf_dynamic_file(
        resolved_linkage: ValidatedElfResolvedProcedureLinkage,
    ) -> Result<ValidatedElfAssembledDynamicFile, Box<ElfDynamicFileAssemblyError>> {
        checked::plan_stage!(
            plan: resolved_linkage,
            derive: derive_contents,
            contents: contents,
            fingerprint: non_authoritative_assembled_file_compatibility_fingerprint,
            error: ElfDynamicFileAssemblyError,
        )
    }

    pub(crate) fn admit_elf_dynamic_executable(
        assembled: ValidatedElfAssembledDynamicFile,
    ) -> Result<ValidatedElfDynamicExecutable, Box<ElfDynamicExecutableAdmissionError>> {
        let output = match derive_executable_output(&assembled) {
            Ok(output) => output,
            Err(diagnostic) => {
                return Err(Box::new(ElfDynamicExecutableAdmissionError {
                    assembled,
                    diagnostic,
                }));
            }
        };

        let non_authoritative_assembled_file_compatibility_fingerprint =
            assembled.non_authoritative_assembled_file_compatibility_fingerprint;
        let ValidatedElfAssembledDynamicFile {
            resolved_linkage, ..
        } = assembled;
        let mut image = recover_retained_image(resolved_linkage);
        image.memory.text = output.final_text_bytes.clone();
        image.memory.data = output.final_data_bytes.clone();

        Ok(ValidatedElfDynamicExecutable {
            image,
            output,
            non_authoritative_assembled_file_compatibility_fingerprint,
        })
    }

    fn derive_executable_output(
        assembled: &ValidatedElfAssembledDynamicFile,
    ) -> Result<ExecutableImageOutput, Diagnostic> {
        let load = load_layout(&assembled.resolved_linkage);
        let format = dynamic_executable_format(load.target())?;
        let image = load.retained_image();
        let text_bytes = assembled.resolved_linkage.source_text_bytes();
        let data_bytes = assembled.resolved_linkage.source_data_bytes();
        let executable_regions = place_executable_extent(
            text_bytes,
            image.executable_regions.clone(),
            load.final_image_layout().text_address,
        )?;
        let data_regions = place_data_extent(
            data_bytes,
            image.data_regions.clone(),
            load.final_image_layout().data_address,
            "initialized-data",
        )?;
        let (final_import_data_bytes, import_data_regions) =
            procedure_got_import_custody(&assembled.resolved_linkage, load)?;
        Ok(ExecutableImageOutput {
            bytes: assembled.bytes().to_vec(),
            final_image_layout: load.final_image_layout(),
            final_text_bytes: text_bytes.to_vec(),
            final_data_bytes: data_bytes.to_vec(),
            final_import_data_bytes,
            file_name: "omega-program".to_owned(),
            format: format.to_owned(),
            text_bytes: text_bytes.len(),
            data_bytes: data_bytes.len(),
            bss_bytes: image.memory.bss_size,
            symbols: image.symbol_table.symbols.len(),
            imports: image.symbol_table.imports.len(),
            relocations: image.relocation_table.relocations.len(),
            executable_regions,
            data_regions,
            import_data_regions,
        })
    }

    fn procedure_got_import_custody(
        resolved_linkage: &ValidatedElfResolvedProcedureLinkage,
        load: &ValidatedElfDynamicLoadLayout,
    ) -> Result<(Vec<u8>, PlacedDataRegionInventory), Diagnostic> {
        let extent = resolved_linkage.procedure_got_bytes();
        let section = load
            .sections()
            .iter()
            .find(|section| section.kind() == ElfPlacedDynamicSectionKind::ProcedureGot)
            .ok_or_else(|| {
                Diagnostic::error("dynamic ELF load layout has no procedure GOT section")
            })?;
        require(
            checked_u64(extent.len(), "dynamic ELF procedure GOT extent")? == section.byte_size(),
            "dynamic ELF procedure GOT extent drifted from its placed section",
        )?;
        let base = section.virtual_address().ok_or_else(|| {
            Diagnostic::error("dynamic ELF procedure GOT section has no allocated address")
        })?;

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
            let ElfAppliedProcedureLinkageTarget::ProcedureGotSlot { logical_ordinal } =
                fixup.target()
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
            let section_offset =
                usize::try_from(slot_address.checked_sub(base).ok_or_else(|| {
                    Diagnostic::error(
                        "dynamic ELF procedure GOT binding lies below its section base",
                    )
                })?)
                .map_err(|_| {
                    Diagnostic::error(
                        "dynamic ELF procedure GOT slot offset exceeds host address space",
                    )
                })?;
            let request = imports.get(slot.request_index).ok_or_else(|| {
                Diagnostic::error("dynamic ELF slot names an import outside the canonical requests")
            })?;
            let (ElfImportLocator::Versioned { symbol, .. }
            | ElfImportLocator::Component { symbol, .. }) = &request.locator
            else {
                return Err(Diagnostic::error(
                    "dynamic ELF slot binds a non-versioned import locator",
                ));
            };
            let symbol = String::from_utf8(symbol.clone()).map_err(|_| {
                Diagnostic::error("dynamic ELF import symbol spelling is not UTF-8")
            })?;
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

    #[cfg(test)]
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
                && image.memory.data == assembled.resolved_linkage.source_data_bytes()
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

    fn recover_retained_image(
        resolved_linkage: ValidatedElfResolvedProcedureLinkage,
    ) -> FinalImage {
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

    checked::seal_stage! {
        field: resolved_linkage,
        contents: contents,
        validated: ValidatedElfAssembledDynamicFile,
        validate: validate_contents,
        fingerprint: non_authoritative_assembled_file_compatibility_fingerprint,
        nonzero,
        message: "assembled dynamic ELF compatibility fingerprint does not replay",
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
            let expected_byte_count =
                checked_u64(fragment.bytes.len(), "dynamic ELF fragment size")?;
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
            bytes: resolved_linkage.source_data_bytes(),
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
        fragments
            .sort_by_key(|fragment| (fragment.file_offset, fragment_kind_order(fragment.kind)));
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
            let end = checked_sum_u64(
                header.file_offset(),
                header.file_size(),
                "dynamic ELF program-header file extent",
                "Elf64_Xword",
            )?;
            require(
                end <= file_byte_count,
                "assembled dynamic ELF file does not contain a complete program-header extent",
            )?;
        }
        let image = load.image_memory();
        let data_end = checked_sum_u64(
            image.data_file_offset(),
            image.data_size(),
            "assembled source-data extent",
            "Elf64_Xword",
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
        let end = checked_sum_u64(
            envelope.section_header_table_file_offset(),
            checked_u64(
                envelope.section_header_table_bytes().len(),
                "dynamic ELF section-header table size",
            )?,
            "dynamic ELF file end",
            "Elf64_Xword",
        )?;
        usize::try_from(end)
            .map_err(|_| Diagnostic::error("dynamic ELF assembled file exceeds host address space"))
    }

    fn fragment_range(
        file_offset: u64,
        byte_count: usize,
        file_byte_count: usize,
    ) -> Result<std::ops::Range<usize>, Diagnostic> {
        let start = usize::try_from(file_offset).map_err(|_| {
            Diagnostic::error("dynamic ELF fragment offset exceeds host address space")
        })?;
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

    #[cfg(test)]
    mod tests {
        use super::{
            Candidate, Diagnostic, ElfDynamicFileFragmentKind, ElfDynamicFileFragmentPlacement,
            ValidatedElfResolvedProcedureLinkage, admit_elf_dynamic_executable,
            assemble_elf_dynamic_file, derive_contents, derive_executable_output, load_layout,
            non_authoritative_assembled_file_compatibility_fingerprint, section_bytes,
            validate_candidate, validate_executable_output,
        };
        use crate::{
            apply_elf_dynamic_address_fixups, apply_elf_procedure_linkage_fixups,
            apply_elf_section_header_placements, plan_elf_dynamic_load_layout,
            plan_elf_indexed_section_payloads, plan_elf_relative_section_payload_layout,
            serialize_elf_dynamic_file_envelope,
        };
        use arena::Handle;
        use image::{
            FinalDataRegion, FinalDataRegionOrigin, FinalExecutableRegion,
            FinalExecutableRegionOrigin, FinalImage, FinalImageMemory,
            validate_placed_data_region_inventory, validate_placed_executable_region_inventory,
        };
        use target::TargetProfile;

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

        fn standard_resolved_linkage(
            target: TargetProfile,
        ) -> ValidatedElfResolvedProcedureLinkage {
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
            crate::test_support::text_entry(&mut image, 24);
            image.executable_regions.push(FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::CompilerFunction,
                section_offset: 0,
                byte_count: 28,
                symbol: "relocated-caller".to_owned(),
                footprint: None,
                entry_effects: None,
            });
            image.data_regions.push(FinalDataRegion {
                origin: FinalDataRegionOrigin::CompilerData,
                section_offset: 0,
                byte_count: 13,
                symbol: "caller-data".to_owned(),
            });

            let mut imports = Vec::new();
            for (index, symbol) in imported_symbols.into_iter().enumerate() {
                imports.push(crate::test_support::versioned_import(
                    &mut image,
                    &format!("__omega_assembled_import_{index}"),
                    b"libassembled-file.so",
                    symbol,
                    b"ASSEMBLED_FILE_1",
                    target,
                    "versioned import locator",
                ));
            }
            crate::test_support::insert_text_call_relocations(
                &mut image,
                target,
                [(0, imports[0]), (8, imports[0]), (16, imports[1])],
            );

            let linkage = crate::test_support::resolved_linkage(image, target);
            let headers = crate::test_support::section_header_table_from_linkage(linkage);
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

                assert_eq!(assembled.fragment_placements().len(), 17);
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
                        ElfDynamicFileFragmentKind::HeaderPrefix => {
                            (0, envelope.header_prefix_bytes())
                        }
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
                assemble_elf_dynamic_file(standard_resolved_linkage(TargetProfile::LinuxX64))
                    .unwrap();
            let replay =
                assemble_elf_dynamic_file(standard_resolved_linkage(TargetProfile::LinuxX64))
                    .unwrap();
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
                if range.is_empty() {
                    continue;
                }
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
                    admitted.non_authoritative_assembled_file_compatibility_fingerprint(),
                    assembled_fingerprint,
                );
                assert!(admitted.output().bytes.starts_with(b"\x7fELF"));
                assert!(admitted.output().format.contains("dynamic-executable"));
                let output = admitted.output();
                validate_placed_executable_region_inventory(
                    &output.executable_regions,
                    &admitted.image().memory.text,
                )
                .unwrap();
                assert!(
                    validate_placed_executable_region_inventory(
                        &output.executable_regions,
                        &original.memory.text,
                    )
                    .is_err()
                );
                validate_placed_data_region_inventory(
                    &output.data_regions,
                    &admitted.image().memory.data,
                )
                .unwrap();
                assert_eq!(output.executable_regions.regions.len(), 1);
                assert_eq!(output.executable_regions.unclassified_gaps.len(), 1);
                assert_eq!(output.data_regions.regions.len(), 1);
            }
        }

        #[test]
        fn final_byte_admission_rejects_independent_output_drift() {
            let assembled =
                assemble_elf_dynamic_file(standard_resolved_linkage(TargetProfile::LinuxX64))
                    .unwrap();
            let mut image = load_layout(&assembled.resolved_linkage)
                .retained_image()
                .clone();
            image.memory.text = assembled.resolved_linkage.source_text_bytes().to_vec();
            let output = derive_executable_output(&assembled).unwrap();
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
}

pub(crate) mod dynamic_file_envelope {
    use super::super::checked::{self, CandidateValidationError};
    use crate::bytes::{write_u16, write_u32, write_u64};
    use crate::constants::ELF64_SECTION_HEADER_SIZE;
    use crate::dynamic_executable::checked::{
        checked_product, checked_sum, read_u16, read_u32, read_u64, require,
    };
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
    const ELF64_SECTION_HEADER_COUNT: usize = 14;
    const ELF64_SECTION_HEADER_TABLE_SIZE: usize =
        ELF64_SECTION_HEADER_SIZE * ELF64_SECTION_HEADER_COUNT;

    const ET_EXEC: u16 = 2;
    const EM_X86_64: u16 = 62;
    const EM_AARCH64: u16 = 183;
    const EV_CURRENT: u32 = 1;
    const PT_LOAD: u32 = 1;
    const PT_DYNAMIC: u32 = 2;
    const PT_INTERP: u32 = 3;

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

        pub(crate) fn header_prefix_bytes(&self) -> &[u8] {
            &self.contents.header_prefix_bytes
        }

        pub const fn entry_address(&self) -> u64 {
            self.contents.entry_address
        }

        pub const fn section_header_table_file_offset(&self) -> u64 {
            self.contents.section_header_table_file_offset
        }

        pub(crate) fn section_header_table_bytes(&self) -> &[u8] {
            &self.contents.section_header_table_bytes
        }

        pub const fn non_authoritative_envelope_compatibility_fingerprint(&self) -> u64 {
            self.non_authoritative_envelope_compatibility_fingerprint
        }

        pub(crate) fn into_resolved_dynamic_table(self) -> ValidatedElfResolvedDynamicTable {
            self.resolved_dynamic_table
        }
    }

    #[derive(Debug)]
    #[must_use = "dynamic ELF envelope rejection retains resolved-dynamic custody"]
    pub struct ElfDynamicFileEnvelopeSerializationError {
        resolved_dynamic_table: ValidatedElfResolvedDynamicTable,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(
        ElfDynamicFileEnvelopeSerializationError;
        resolved_dynamic_table: ValidatedElfResolvedDynamicTable,
    );

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

    pub(crate) fn serialize_elf_dynamic_file_envelope(
        resolved_dynamic_table: ValidatedElfResolvedDynamicTable,
    ) -> Result<ValidatedElfDynamicFileEnvelope, Box<ElfDynamicFileEnvelopeSerializationError>>
    {
        checked::plan_stage!(
            plan: resolved_dynamic_table,
            derive: derive_contents,
            contents: contents,
            fingerprint: non_authoritative_envelope_compatibility_fingerprint,
            error: ElfDynamicFileEnvelopeSerializationError,
        )
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
        write_u16(&mut header_prefix_bytes, section_name_table_index(layout)?);

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

    checked::seal_stage! {
        field: resolved_dynamic_table,
        contents: contents,
        validated: ValidatedElfDynamicFileEnvelope,
        validate: validate_contents,
        fingerprint: non_authoritative_envelope_compatibility_fingerprint,
        nonzero,
        message: "dynamic ELF envelope compatibility fingerprint does not replay",
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
                && header.section_name_table_index == section_name_table_index(layout)?,
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
            read_only.file_offset() == 0
                && read_only.file_size() >= ELF64_HEADER_PREFIX_SIZE as u64,
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
        crate::dynamic_executable::checked::checked_sum_u64(
            file_offset,
            byte_count,
            "dynamic ELF section-header fragment end",
            "Elf64_Off",
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
            let load_end = crate::dynamic_executable::checked::checked_sum_u64(
                load.file_offset(),
                load.file_size(),
                "dynamic ELF PT_LOAD file end",
                "Elf64_Off",
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

    fn section_name_table_index(layout: &ValidatedElfDynamicLoadLayout) -> Result<u16, Diagnostic> {
        u16::try_from(
            layout
                .relative()
                .payloads()
                .section_headers()
                .roster()
                .section_name_table_index(),
        )
        .map_err(|_| Diagnostic::error("dynamic ELF section-name table index exceeds e_shstrndx"))
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

    fn non_authoritative_envelope_compatibility_fingerprint(
        resolved: &ValidatedElfResolvedDynamicTable,
        contents: &ElfDynamicFileEnvelopeContents,
    ) -> u64 {
        let mut hash = checked::stage_fingerprint(
            b"omega.elf.dynamic-file-envelope.v1",
            resolved.non_authoritative_resolved_compatibility_fingerprint(),
        );
        hash.bytes(&contents.header_prefix_bytes);
        hash.bytes(&contents.entry_address.to_le_bytes());
        hash.bytes(&contents.section_header_table_file_offset.to_le_bytes());
        hash.bytes(&contents.section_header_table_bytes);
        hash.finish()
    }

    #[cfg(test)]
    mod tests {
        use super::{
            Candidate, Diagnostic, ELF64_HEADER_PREFIX_SIZE, ELF64_HEADER_SIZE, EM_AARCH64,
            EM_X86_64, FinalImageSection, PT_DYNAMIC, PT_INTERP, PT_LOAD, TargetProfile,
            ValidatedElfResolvedDynamicTable, decode_header, decode_program_headers,
            derive_contents, load_layout, non_authoritative_envelope_compatibility_fingerprint,
            serialize_elf_dynamic_file_envelope, validate_candidate,
        };
        use crate::{
            apply_elf_dynamic_address_fixups, apply_elf_section_header_placements,
            plan_elf_dynamic_load_layout, plan_elf_indexed_section_payloads,
            plan_elf_relative_section_payload_layout,
        };
        use image::{FinalImage, FinalImageSymbol};
        use object_file::SymbolKind;

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
            let mut image = crate::test_support::import_image(target, 16, 2);
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
            let imported = crate::test_support::versioned_import(
                &mut image,
                "__omega_dynamic_envelope_import",
                b"libdynamic-envelope.so",
                imported_symbol,
                b"DYNAMIC_ENVELOPE_1",
                target,
                "versioned import locator",
            );
            crate::test_support::insert_text_call_relocations(&mut image, target, [(0, imported)]);
            let linkage = crate::test_support::resolved_linkage(image, target);
            let headers = crate::test_support::section_header_table_from_linkage(linkage);
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
            optimization_core::codec_bytes::patch_encoded(bytes, offset, value.to_le_bytes());
        }

        #[test]
        fn both_linux_targets_emit_exact_non_runnable_envelopes() {
            for (target, machine) in [
                (TargetProfile::LinuxX64, EM_X86_64),
                (TargetProfile::LinuxArm64, EM_AARCH64),
            ] {
                let envelope =
                    serialize_elf_dynamic_file_envelope(standard_resolved(target)).unwrap();
                assert_eq!(envelope.header_prefix_bytes().len(), 344);
                assert_eq!(envelope.section_header_table_bytes().len(), 896);
                assert_ne!(
                    envelope.non_authoritative_envelope_compatibility_fingerprint(),
                    0,
                );
                let header = decode_header(envelope.header_prefix_bytes()).unwrap();
                assert_eq!(header.machine, machine);
                const SHT_STRTAB: u32 = 3;
                let name_table_index = usize::from(header.section_name_table_index);
                let name_table_header = &envelope.section_header_table_bytes()
                    [name_table_index * 64..(name_table_index + 1) * 64];
                assert_eq!(
                    u32::from_le_bytes(name_table_header[4..8].try_into().unwrap()),
                    SHT_STRTAB,
                );
                let access_name_table =
                    &load_layout(envelope.resolved_dynamic_table()).sections()[name_table_index];
                assert_eq!(
                    access_name_table.kind(),
                    crate::ElfPlacedDynamicSectionKind::SectionNameTable,
                );
                assert_eq!(
                    u64::from_le_bytes(name_table_header[24..32].try_into().unwrap()),
                    access_name_table.file_offset(),
                );
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
            optimization_core::codec_bytes::patch_encoded(
                &mut target.contents.header_prefix_bytes,
                18,
                EM_AARCH64.to_le_bytes(),
            );
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
            load_overlap.contents.section_header_table_file_offset =
                ELF64_HEADER_PREFIX_SIZE as u64;
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
}

pub(crate) mod resolved_procedure_linkage {
    mod candidates {
        use crate::dynamic_executable::file_assembly::dynamic_file_envelope::{ValidatedElfDynamicFileEnvelope};
        use crate::dynamic_executable::section_headers::section_payload_roster::{ElfIndexedProcedureFixup};
        use crate::dynamic_executable::section_headers::section_roster::{ElfDynamicRosterSectionKind};
        use diagnostics::{Diagnostic};
        use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::{ElfAppliedProcedureLinkageFixup, ElfAppliedProcedureLinkageStorage, ValidatedElfResolvedProcedureLinkage};
        use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::compatibility_fingerprint::{non_authoritative_resolved_linkage_compatibility_fingerprint};
        use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::storage_fields::{ decode_rejoins_target, encode_field, indexed_payloads, indexed_row_bytes, load_layout, public_kind, public_storage, public_target, semantic_target_address, storage_address, storage_bytes, storage_bytes_mut, upstream_storage_bytes, write_field};
        use crate::dynamic_executable::checked::{checked_u32, read_field, require};
        use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::image_local_relocations::{ElfImageLocalRelocatedMemory, relocate_image_local_memory};
        use super::super::super::checked::{self, CandidateValidationError};

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct ElfResolvedProcedureLinkageContents {
            pub(crate) source_text_bytes: Vec<u8>,
            pub(crate) source_data_bytes: Vec<u8>,
            pub(crate) procedure_linkage_bytes: Vec<u8>,
            pub(crate) procedure_got_bytes: Vec<u8>,
            pub(crate) procedure_relocation_bytes: Vec<u8>,
            pub(crate) general_relocation_bytes: Vec<u8>,
            pub(crate) applications: Vec<ElfAppliedProcedureLinkageFixup>,
        }

        pub(crate) struct Candidate {
            pub(super) envelope: ValidatedElfDynamicFileEnvelope,
            pub(super) contents: ElfResolvedProcedureLinkageContents,
            pub(super) non_authoritative_resolved_linkage_compatibility_fingerprint: u64,
        }

        pub(crate) fn derive_contents(
            envelope: &ValidatedElfDynamicFileEnvelope,
        ) -> Result<ElfResolvedProcedureLinkageContents, Diagnostic> {
            let payloads = indexed_payloads(envelope);
            let ElfImageLocalRelocatedMemory { text, data } =
                relocate_image_local_memory(load_layout(envelope))?;
            let mut contents = ElfResolvedProcedureLinkageContents {
                source_text_bytes: text,
                source_data_bytes: data,
                procedure_linkage_bytes: indexed_row_bytes(
                    payloads,
                    8,
                    ElfDynamicRosterSectionKind::ProcedureLinkage,
                )?
                .to_vec(),
                procedure_got_bytes: indexed_row_bytes(
                    payloads,
                    9,
                    ElfDynamicRosterSectionKind::ProcedureGot,
                )?
                .to_vec(),
                procedure_relocation_bytes: indexed_row_bytes(
                    payloads,
                    10,
                    ElfDynamicRosterSectionKind::ProcedureRelocation,
                )?
                .to_vec(),
                general_relocation_bytes: indexed_row_bytes(
                    payloads,
                    11,
                    ElfDynamicRosterSectionKind::GeneralRelocation,
                )?
                .to_vec(),
                applications: Vec::with_capacity(payloads.procedure_fixups.len()),
            };
            for fixup in &payloads.procedure_fixups {
                let application = derive_application(envelope, &contents, fixup)?;
                write_field(
                    storage_bytes_mut(&mut contents, application.storage),
                    application.byte_offset,
                    application.byte_width,
                    application.encoded_field,
                )?;
                contents.applications.push(application);
            }
            Ok(contents)
        }

        fn derive_application(
            envelope: &ValidatedElfDynamicFileEnvelope,
            contents: &ElfResolvedProcedureLinkageContents,
            fixup: &ElfIndexedProcedureFixup,
        ) -> Result<ElfAppliedProcedureLinkageFixup, Diagnostic> {
            let storage = public_storage(fixup.storage)?;
            let source_address = storage_address(load_layout(envelope), fixup)?;
            let target_address = semantic_target_address(load_layout(envelope), fixup)?;
            let original = read_field(
                storage_bytes(contents, storage),
                fixup.byte_offset,
                fixup.byte_width,
                "procedure-linkage fixup field",
            )?;
            require(
                original & fixup.mutable_mask == 0,
                "procedure-linkage fixup source is not an exact zero placeholder",
            )?;
            let encoded_field = encode_field(
                fixup.kind,
                original,
                fixup.mutable_mask,
                source_address,
                target_address,
            )?;
            Ok(ElfAppliedProcedureLinkageFixup {
                ordinal: fixup.upstream_ordinal,
                storage,
                byte_offset: fixup.byte_offset,
                byte_width: fixup.byte_width,
                mutable_mask: fixup.mutable_mask,
                kind: public_kind(fixup.kind),
                target: public_target(fixup.target),
                source_address,
                target_address,
                encoded_field,
            })
        }

        checked::seal_stage! {
            field: envelope,
            contents: contents,
            validated: ValidatedElfResolvedProcedureLinkage,
            validate: validate_contents,
            fingerprint: non_authoritative_resolved_linkage_compatibility_fingerprint,
            nonzero,
            message: "resolved procedure-linkage compatibility fingerprint does not replay",
            vis: pub(crate),
        }

        fn validate_contents(
            envelope: &ValidatedElfDynamicFileEnvelope,
            contents: &ElfResolvedProcedureLinkageContents,
        ) -> Result<(), Diagnostic> {
            let payloads = indexed_payloads(envelope);
            let relocated = relocate_image_local_memory(load_layout(envelope))?;
            require(
                contents.source_data_bytes == relocated.data,
                "resolved source data drifted from its exact image-local relocation",
            )?;
            require(
                contents.source_text_bytes.len()
                    == load_layout(envelope).retained_image().memory.text.len()
                    && contents.procedure_linkage_bytes.len()
                        == indexed_row_bytes(
                            payloads,
                            8,
                            ElfDynamicRosterSectionKind::ProcedureLinkage,
                        )?
                        .len()
                    && contents.procedure_got_bytes.len()
                        == indexed_row_bytes(
                            payloads,
                            9,
                            ElfDynamicRosterSectionKind::ProcedureGot,
                        )?
                        .len()
                    && contents.procedure_relocation_bytes.len()
                        == indexed_row_bytes(
                            payloads,
                            10,
                            ElfDynamicRosterSectionKind::ProcedureRelocation,
                        )?
                        .len(),
                "resolved procedure-linkage fragment length drifted from exact upstream storage",
            )?;
            require(
                contents.applications.len() == payloads.procedure_fixups.len(),
                "resolved procedure-linkage application coverage is incomplete or duplicated",
            )?;
            validate_nonoverlapping_applications(&contents.applications)?;

            for (ordinal, (application, fixup)) in contents
                .applications
                .iter()
                .zip(&payloads.procedure_fixups)
                .enumerate()
            {
                require(
                    application.ordinal == checked_u32(ordinal, "procedure application ordinal")?
                        && application.ordinal == fixup.upstream_ordinal
                        && application.storage == public_storage(fixup.storage)?
                        && application.byte_offset == fixup.byte_offset
                        && application.byte_width == fixup.byte_width
                        && application.mutable_mask == fixup.mutable_mask
                        && application.kind == public_kind(fixup.kind)
                        && application.target == public_target(fixup.target),
                    "resolved procedure-linkage application drifted from its indexed fixup",
                )?;
                let expected_source = storage_address(load_layout(envelope), fixup)?;
                let expected_target = semantic_target_address(load_layout(envelope), fixup)?;
                require(
                    application.source_address == expected_source
                        && application.target_address == expected_target,
                    "resolved procedure-linkage source or target address drifted from absolute layout",
                )?;
                let original = read_field(
                    upstream_storage_bytes(envelope, &relocated, application.storage)?,
                    application.byte_offset,
                    application.byte_width,
                    "procedure-linkage fixup field",
                )?;
                require(
                    original & application.mutable_mask == 0,
                    "retained procedure-linkage template no longer has an exact zero placeholder",
                )?;
                let expected_field = encode_field(
                    fixup.kind,
                    original,
                    application.mutable_mask,
                    application.source_address,
                    application.target_address,
                )?;
                let actual_field = read_field(
                    storage_bytes(contents, application.storage),
                    application.byte_offset,
                    application.byte_width,
                    "procedure-linkage fixup field",
                )?;
                require(
                    application.encoded_field == expected_field && actual_field == expected_field,
                    "resolved procedure-linkage encoded field does not replay",
                )?;
                require(
                    (actual_field & !application.mutable_mask)
                        == (original & !application.mutable_mask),
                    "resolved procedure-linkage application changed fixed opcode bits",
                )?;
                decode_rejoins_target(
                    fixup.kind,
                    actual_field,
                    application.source_address,
                    application.target_address,
                )?;
            }

            for storage in [
                ElfAppliedProcedureLinkageStorage::SourceText,
                ElfAppliedProcedureLinkageStorage::ProcedureLinkage,
                ElfAppliedProcedureLinkageStorage::ProcedureGot,
                ElfAppliedProcedureLinkageStorage::ProcedureRelocation,
            ] {
                validate_unchanged_bytes(
                    upstream_storage_bytes(envelope, &relocated, storage)?,
                    storage_bytes(contents, storage),
                    storage,
                    &contents.applications,
                )?;
            }
            Ok(())
        }

        pub(crate) fn validate_nonoverlapping_applications(
            applications: &[ElfAppliedProcedureLinkageFixup],
        ) -> Result<(), Diagnostic> {
            for (index, application) in applications.iter().enumerate() {
                let end = crate::dynamic_executable::checked::checked_sum(
                    application.byte_offset,
                    usize::from(application.byte_width),
                    "procedure application end",
                )?;
                for other in &applications[index + 1..] {
                    if application.storage != other.storage {
                        continue;
                    }
                    let other_end = crate::dynamic_executable::checked::checked_sum(
                        other.byte_offset,
                        usize::from(other.byte_width),
                        "procedure application end",
                    )?;
                    require(
                        end <= other.byte_offset || other_end <= application.byte_offset,
                        "resolved procedure-linkage applications overlap or duplicate one field",
                    )?;
                }
            }
            Ok(())
        }

        fn validate_unchanged_bytes(
            original: &[u8],
            resolved: &[u8],
            storage: ElfAppliedProcedureLinkageStorage,
            applications: &[ElfAppliedProcedureLinkageFixup],
        ) -> Result<(), Diagnostic> {
            require(
                original.len() == resolved.len(),
                "resolved procedure-linkage storage length drifted",
            )?;
            for (offset, (before, after)) in original.iter().zip(resolved).enumerate() {
                let mutable = applications.iter().any(|application| {
                    application.storage == storage
                        && offset >= application.byte_offset
                        && offset
                            < application
                                .byte_offset
                                .saturating_add(usize::from(application.byte_width))
                });
                require(
                    mutable || before == after,
                    "resolved procedure-linkage application changed an unowned byte",
                )?;
            }
            Ok(())
        }
    }

    mod compatibility_fingerprint {
        use target::compatibility_fingerprint::Fnv1a;

        use crate::dynamic_executable::file_assembly::dynamic_file_envelope::ValidatedElfDynamicFileEnvelope;
        use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::ElfAppliedProcedureLinkageTarget;
        use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::candidates::ElfResolvedProcedureLinkageContents;

        pub(crate) fn non_authoritative_resolved_linkage_compatibility_fingerprint(
            envelope: &ValidatedElfDynamicFileEnvelope,
            contents: &ElfResolvedProcedureLinkageContents,
        ) -> u64 {
            let mut hash = crate::dynamic_executable::checked::stage_fingerprint(
                b"omega.elf.resolved-procedure-linkage.v1",
                envelope.non_authoritative_envelope_compatibility_fingerprint(),
            );
            hash.bytes(&contents.source_text_bytes);
            hash.bytes(&contents.source_data_bytes);
            hash.bytes(&contents.procedure_linkage_bytes);
            hash.bytes(&contents.procedure_got_bytes);
            hash.bytes(&contents.procedure_relocation_bytes);
            hash.bytes(&(contents.applications.len() as u64).to_le_bytes());
            for application in &contents.applications {
                hash.bytes(&application.ordinal.to_le_bytes());
                hash.byte(application.storage as u8);
                hash.bytes(&(application.byte_offset as u64).to_le_bytes());
                hash.byte(application.byte_width);
                hash.bytes(&application.mutable_mask.to_le_bytes());
                hash.byte(application.kind as u8);
                hash_target(&mut hash, application.target);
                hash.bytes(&application.source_address.to_le_bytes());
                hash.bytes(&application.target_address.to_le_bytes());
                hash.bytes(&application.encoded_field.to_le_bytes());
            }
            hash.finish()
        }

        fn hash_target(hash: &mut Fnv1a, target: ElfAppliedProcedureLinkageTarget) {
            match target {
                ElfAppliedProcedureLinkageTarget::DynamicSection => hash.byte(1),
                ElfAppliedProcedureLinkageTarget::ProcedureLinkageHeader => hash.byte(2),
                ElfAppliedProcedureLinkageTarget::ProcedureLinkageEntry { logical_ordinal } => {
                    hash.byte(3);
                    hash.bytes(&logical_ordinal.to_le_bytes());
                }
                ElfAppliedProcedureLinkageTarget::ProcedureLinkageLazyTail { logical_ordinal } => {
                    hash.byte(4);
                    hash.bytes(&logical_ordinal.to_le_bytes());
                }
                ElfAppliedProcedureLinkageTarget::ProcedureGotHeaderWord { word_index } => {
                    hash.byte(5);
                    hash.byte(word_index);
                }
                ElfAppliedProcedureLinkageTarget::ProcedureGotSlot { logical_ordinal } => {
                    hash.byte(6);
                    hash.bytes(&logical_ordinal.to_le_bytes());
                }
                ElfAppliedProcedureLinkageTarget::RelocatedImageSection {
                    section,
                    byte_offset,
                } => {
                    hash.byte(7);
                    hash.byte(crate::dynamic_executable::image_section_tag(section));
                    hash.bytes(&(byte_offset as u64).to_le_bytes());
                }
            }
        }
    }

    mod image_local_relocations {
        use crate::dynamic_executable::load_placement::load_layout::ValidatedElfDynamicLoadLayout;
        use diagnostics::Diagnostic;
        use image::{
            FinalImage, FinalImageRelocationTable, apply_relocations, final_image_imports_symbol,
        };
        use target::TargetProfile;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct ElfImageLocalRelocatedMemory {
            pub(crate) text: Vec<u8>,
            pub(crate) data: Vec<u8>,
        }

        pub(crate) fn relocate_image_local_memory(
            layout: &ValidatedElfDynamicLoadLayout,
        ) -> Result<ElfImageLocalRelocatedMemory, Diagnostic> {
            let image = layout.retained_image();
            let mut relocation_table =
                FinalImageRelocationTable::with_capacity(image.relocation_table.relocations.len());
            for (_, relocation) in image.relocation_table.relocations.iter() {
                if !final_image_imports_symbol(image, relocation.symbol_handle) {
                    relocation_table.relocations.insert(relocation.clone());
                }
            }
            let mut local_image = FinalImage {
                target: image.target,
                memory: image.memory.clone(),
                symbol_table: image.symbol_table.clone(),
                relocation_table,
                executable_regions: Vec::new(),
                data_regions: Vec::new(),
            };
            let final_layout = layout.final_image_layout();
            match layout.target() {
                TargetProfile::LinuxX64 | TargetProfile::LinuxArm64 => {}
                _ => {
                    return Err(Diagnostic::error(
                        "dynamic ELF image-local relocation requires an exact Linux x86-64 or AArch64 profile",
                    ));
                }
            }
            apply_relocations(&mut local_image, &final_layout, "dynamic ELF image")?;
            Ok(ElfImageLocalRelocatedMemory {
                text: local_image.memory.text,
                data: local_image.memory.data,
            })
        }

        #[cfg(test)]
        mod tests {
            use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::candidates::{
                Candidate, derive_contents, validate_candidate,
            };
            use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::compatibility_fingerprint::non_authoritative_resolved_linkage_compatibility_fingerprint;
            use crate::{
                ValidatedElfDynamicFileEnvelope, apply_elf_dynamic_address_fixups,
                apply_elf_section_header_placements, emit_elf_dynamic_executable,
                plan_elf_dynamic_load_layout,
                plan_elf_indexed_section_payloads,
                plan_elf_relative_section_payload_layout,
                serialize_elf_dynamic_file_envelope,
            };
            use arena::Handle;
            use image::{
                FinalImage, FinalImageMemory, FinalImageRelocation, FinalImageSection, FinalImageSymbol,
            };
            use object_file::{RelocationKind, SymbolKind};
            use target::TargetProfile;

            const PRIVATE_CALLBACK_OFFSET: usize = 40;
            const CALLBACK_MATERIALIZATION_OFFSET: usize = 8;
            const CALLBACK_POINTER_DATA_OFFSET: usize = 8;

            fn callback_image(target: TargetProfile) -> FinalImage {
                let mut image = FinalImage::with_capacity(
                    target.native_target(),
                    FinalImageMemory {
                        text: vec![0; 64],
                        data: (0_u8..24).map(|byte| byte.wrapping_add(0x30)).collect(),
                        bss_size: 16,
                        bss_alignment: 8,
                    },
                    Handle::invalid(),
                    3,
                    1,
                    4,
                );
                image.memory.data[CALLBACK_POINTER_DATA_OFFSET..CALLBACK_POINTER_DATA_OFFSET + 8]
                    .fill(0);
                crate::test_support::text_entry(&mut image, 24);
                let private_callback = image.symbol_table.symbols.insert(FinalImageSymbol {
                    name: "__omega_private_callback".to_owned(),
                    section: FinalImageSection::Text,
                    offset: PRIVATE_CALLBACK_OFFSET,
                    size: 8,
                    kind: SymbolKind::Function,
                });
                let import = crate::test_support::versioned_import(
                    &mut image,
                    "__omega_callback_registrar",
                    b"libcallback-registrar.so",
                    b"register_callback",
                    b"CALLBACK_1",
                    target,
                    "versioned import locator",
                );

                let mut relocate = |section, offset, byte_width, symbol_handle, kind| {
                    image
                        .relocation_table
                        .relocations
                        .insert(FinalImageRelocation {
                            section,
                            offset,
                            byte_width,
                            symbol_handle,
                            addend: 0,
                            kind,
                        });
                };
                match target {
                    TargetProfile::LinuxX64 => {
                        relocate(
                            FinalImageSection::Text,
                            1,
                            4,
                            import,
                            RelocationKind::X86_64Relative32,
                        );
                        relocate(
                            FinalImageSection::Text,
                            CALLBACK_MATERIALIZATION_OFFSET + 3,
                            4,
                            private_callback,
                            RelocationKind::X86_64Relative32,
                        );
                    }
                    TargetProfile::LinuxArm64 => {
                        relocate(
                            FinalImageSection::Text,
                            0,
                            4,
                            import,
                            RelocationKind::Aarch64Branch26,
                        );
                        relocate(
                            FinalImageSection::Text,
                            CALLBACK_MATERIALIZATION_OFFSET,
                            4,
                            private_callback,
                            RelocationKind::Aarch64Page21,
                        );
                        relocate(
                            FinalImageSection::Text,
                            CALLBACK_MATERIALIZATION_OFFSET + 4,
                            4,
                            private_callback,
                            RelocationKind::Aarch64PageOffset12,
                        );
                    }
                    _ => unreachable!(),
                }
                relocate(
                    FinalImageSection::Data,
                    CALLBACK_POINTER_DATA_OFFSET,
                    8,
                    private_callback,
                    RelocationKind::Absolute64,
                );

                match target {
                    TargetProfile::LinuxX64 => {
                        image.memory.text[0] = 0xe8;
                        image.memory.text
                            [CALLBACK_MATERIALIZATION_OFFSET..CALLBACK_MATERIALIZATION_OFFSET + 3]
                            .copy_from_slice(&[0x48, 0x8d, 0x05]);
                    }
                    TargetProfile::LinuxArm64 => {
                        image.memory.text[0..4].copy_from_slice(&[0, 0, 0, 0x94]);
                        image.memory.text
                            [CALLBACK_MATERIALIZATION_OFFSET..CALLBACK_MATERIALIZATION_OFFSET + 8]
                            .copy_from_slice(&[0, 0, 0, 0x90, 0, 0, 0, 0x91]);
                    }
                    _ => unreachable!(),
                }
                image
            }

            use crate::test_support::interpreter;

            fn envelope(target: TargetProfile) -> ValidatedElfDynamicFileEnvelope {
                let linkage = crate::test_support::resolved_linkage(callback_image(target), target);
                let headers = crate::test_support::section_header_table_from_linkage(linkage);
                let payloads = plan_elf_indexed_section_payloads(headers).unwrap();
                let relative = plan_elf_relative_section_payload_layout(payloads).unwrap();
                let load = plan_elf_dynamic_load_layout(relative).unwrap();
                let placed = apply_elf_section_header_placements(load).unwrap();
                let resolved = apply_elf_dynamic_address_fixups(placed).unwrap();
                serialize_elf_dynamic_file_envelope(resolved).unwrap()
            }

            fn read_u32(bytes: &[u8], offset: usize) -> u32 {
                u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
            }

            fn read_u64(bytes: &[u8], offset: usize) -> u64 {
                u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
            }

            fn file_offset_of(file: &[u8], address: u64) -> usize {
                const PT_LOAD: u32 = 1;
                let program_header_offset = read_u64(file, 32) as usize;
                let program_header_size = usize::from(u16::from_le_bytes([file[54], file[55]]));
                let program_header_count = usize::from(u16::from_le_bytes([file[56], file[57]]));
                for index in 0..program_header_count {
                    let header = program_header_offset + index * program_header_size;
                    if read_u32(file, header) != PT_LOAD {
                        continue;
                    }
                    let file_offset = read_u64(file, header + 8);
                    let virtual_address = read_u64(file, header + 16);
                    let file_size = read_u64(file, header + 32);
                    if (virtual_address..virtual_address + file_size).contains(&address) {
                        return (file_offset + (address - virtual_address)) as usize;
                    }
                }
                panic!("address 0x{address:x} is not file-backed by any PT_LOAD segment");
            }

            fn materialized_callback_address(
                target: TargetProfile,
                text: &[u8],
                text_address: u64,
            ) -> u64 {
                let instruction_address = text_address + CALLBACK_MATERIALIZATION_OFFSET as u64;
                match target {
                    TargetProfile::LinuxX64 => {
                        let displacement =
                            read_u32(text, CALLBACK_MATERIALIZATION_OFFSET + 3).cast_signed();
                        (instruction_address + 7).wrapping_add_signed(i64::from(displacement))
                    }
                    TargetProfile::LinuxArm64 => {
                        let adrp = read_u32(text, CALLBACK_MATERIALIZATION_OFFSET);
                        let add = read_u32(text, CALLBACK_MATERIALIZATION_OFFSET + 4);
                        let page_immediate = (((adrp >> 5) & 0x7ffff) << 2) | ((adrp >> 29) & 0b11);
                        let signed_pages = i64::from(((page_immediate << 11) as i32) >> 11);
                        let page =
                            (instruction_address & !0xfff).wrapping_add_signed(signed_pages * 4096);
                        page + u64::from((add >> 10) & 0xfff)
                    }
                    _ => unreachable!(),
                }
            }

            #[test]
            fn dynamic_image_resolves_callback_address_and_data_pointer_to_the_private_function() {
                for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                    let admitted =
                        emit_elf_dynamic_executable(callback_image(target), interpreter(target))
                            .expect("import-bearing image with a private callback emits");
                    let output = admitted.output();
                    let text_address = output.final_image_layout.text_address;
                    let data_address = output.final_image_layout.data_address;
                    let private_callback = text_address + PRIVATE_CALLBACK_OFFSET as u64;

                    assert_eq!(
                        materialized_callback_address(
                            target,
                            &output.final_text_bytes,
                            text_address
                        ),
                        private_callback,
                        "{target:?} callback address materialization must name the exact private function",
                    );
                    assert_eq!(
                        read_u64(&output.final_data_bytes, CALLBACK_POINTER_DATA_OFFSET),
                        private_callback,
                        "{target:?} initialized-data callback pointer must hold the private function address",
                    );

                    let text_file_offset = file_offset_of(&output.bytes, text_address);
                    let file_text = &output.bytes[text_file_offset..text_file_offset + 64];
                    assert_eq!(file_text, output.final_text_bytes.as_slice());
                    assert_eq!(
                        materialized_callback_address(target, file_text, text_address),
                        private_callback,
                    );
                    let data_file_offset = file_offset_of(&output.bytes, data_address);
                    assert_eq!(
                        read_u64(
                            &output.bytes,
                            data_file_offset + CALLBACK_POINTER_DATA_OFFSET
                        ),
                        private_callback,
                    );
                    assert_eq!(
                        output.final_data_bytes[..CALLBACK_POINTER_DATA_OFFSET],
                        callback_image(target).memory.data[..CALLBACK_POINTER_DATA_OFFSET],
                    );

                    assert_eq!(admitted.image().memory.text, output.final_text_bytes);
                    assert_eq!(admitted.image().memory.data, output.final_data_bytes);
                }
            }

            #[test]
            fn substituted_callback_displacement_or_data_pointer_rejects_on_replay() {
                for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                    let envelope = envelope(target);
                    let contents = derive_contents(&envelope).unwrap();
                    let fingerprint = non_authoritative_resolved_linkage_compatibility_fingerprint(
                        &envelope, &contents,
                    );
                    let mut candidate = Candidate {
                        envelope,
                        contents,
                        non_authoritative_resolved_linkage_compatibility_fingerprint: fingerprint,
                    };

                    let field = match target {
                        TargetProfile::LinuxX64 => CALLBACK_MATERIALIZATION_OFFSET + 3,
                        TargetProfile::LinuxArm64 => CALLBACK_MATERIALIZATION_OFFSET + 5,
                        _ => unreachable!(),
                    };
                    candidate.contents.source_text_bytes[field] ^= 0x10;
                    candidate.non_authoritative_resolved_linkage_compatibility_fingerprint =
                        non_authoritative_resolved_linkage_compatibility_fingerprint(
                            &candidate.envelope,
                            &candidate.contents,
                        );
                    let error = validate_candidate(candidate).expect_err("text drift rejects");
                    assert!(
                        error.diagnostic.message.contains("unowned byte"),
                        "{}",
                        error.diagnostic.message
                    );

                    let mut candidate = error.candidate;
                    candidate.contents.source_text_bytes[field] ^= 0x10;
                    candidate.contents.source_data_bytes[CALLBACK_POINTER_DATA_OFFSET] ^= 0x08;
                    candidate.non_authoritative_resolved_linkage_compatibility_fingerprint =
                        non_authoritative_resolved_linkage_compatibility_fingerprint(
                            &candidate.envelope,
                            &candidate.contents,
                        );
                    let error = validate_candidate(candidate).expect_err("data drift rejects");
                    assert!(
                        error.diagnostic.message.contains("source data"),
                        "{}",
                        error.diagnostic.message
                    );

                    let mut candidate = error.candidate;
                    candidate.contents.source_data_bytes[CALLBACK_POINTER_DATA_OFFSET] ^= 0x08;
                    candidate.non_authoritative_resolved_linkage_compatibility_fingerprint =
                        non_authoritative_resolved_linkage_compatibility_fingerprint(
                            &candidate.envelope,
                            &candidate.contents,
                        );
                    validate_candidate(candidate)
                        .map(drop)
                        .unwrap_or_else(|error| {
                            panic!("restored candidate replays: {}", error.diagnostic)
                        });
                }
            }
        }
    }

    mod linkage_fixups {
        use crate::dynamic_executable::file_assembly::dynamic_file_envelope::ValidatedElfDynamicFileEnvelope;
        use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::candidates::ElfResolvedProcedureLinkageContents;
        use diagnostics::Diagnostic;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u8)]
        pub(crate) enum ElfAppliedProcedureLinkageStorage {
            SourceText = 1,
            ProcedureLinkage = 2,
            ProcedureGot = 3,
            ProcedureRelocation = 4,
            GeneralRelocation = 5,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u8)]
        pub(crate) enum ElfAppliedProcedureLinkageKind {
            X86PcRelative32 = 1,
            Aarch64Page21 = 2,
            Aarch64Load64Low12 = 3,
            Aarch64AddLow12 = 4,
            Aarch64Branch26 = 5,
            Absolute64 = 6,
            Elf64RelaOffset = 7,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum ElfAppliedProcedureLinkageTarget {
            DynamicSection,
            ProcedureLinkageHeader,
            ProcedureLinkageEntry {
                logical_ordinal: u32,
            },
            ProcedureLinkageLazyTail {
                logical_ordinal: u32,
            },
            ProcedureGotHeaderWord {
                word_index: u8,
            },
            ProcedureGotSlot {
                logical_ordinal: u32,
            },
            RelocatedImageSection {
                section: image::FinalImageSection,
                byte_offset: usize,
            },
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct ElfAppliedProcedureLinkageFixup {
            pub(crate) ordinal: u32,
            pub(crate) storage: ElfAppliedProcedureLinkageStorage,
            pub(crate) byte_offset: usize,
            pub(crate) byte_width: u8,
            pub(crate) mutable_mask: u64,
            pub(crate) kind: ElfAppliedProcedureLinkageKind,
            pub(crate) target: ElfAppliedProcedureLinkageTarget,
            pub(crate) source_address: u64,
            pub(crate) target_address: u64,
            pub(crate) encoded_field: u64,
        }

        impl ElfAppliedProcedureLinkageFixup {
            #[cfg(test)]
            pub const fn ordinal(&self) -> u32 {
                self.ordinal
            }

            pub const fn storage(&self) -> ElfAppliedProcedureLinkageStorage {
                self.storage
            }

            #[cfg(test)]
            pub const fn byte_offset(&self) -> usize {
                self.byte_offset
            }

            pub const fn byte_width(&self) -> u8 {
                self.byte_width
            }

            #[cfg(test)]
            pub const fn kind(&self) -> ElfAppliedProcedureLinkageKind {
                self.kind
            }

            pub const fn target(&self) -> ElfAppliedProcedureLinkageTarget {
                self.target
            }

            pub const fn target_address(&self) -> u64 {
                self.target_address
            }

            #[cfg(test)]
            pub const fn encoded_field(&self) -> u64 {
                self.encoded_field
            }
        }

        #[derive(Debug)]
        #[must_use = "resolved ELF procedure linkage retains exact non-runnable envelope custody"]
        pub struct ValidatedElfResolvedProcedureLinkage {
            pub(crate) envelope: ValidatedElfDynamicFileEnvelope,
            pub(crate) contents: ElfResolvedProcedureLinkageContents,
            pub(crate) non_authoritative_resolved_linkage_compatibility_fingerprint: u64,
        }

        impl ValidatedElfResolvedProcedureLinkage {
            pub const fn envelope(&self) -> &ValidatedElfDynamicFileEnvelope {
                &self.envelope
            }

            pub(crate) fn source_text_bytes(&self) -> &[u8] {
                &self.contents.source_text_bytes
            }

            pub(crate) fn source_data_bytes(&self) -> &[u8] {
                &self.contents.source_data_bytes
            }

            pub(crate) fn procedure_linkage_bytes(&self) -> &[u8] {
                &self.contents.procedure_linkage_bytes
            }

            pub(crate) fn procedure_got_bytes(&self) -> &[u8] {
                &self.contents.procedure_got_bytes
            }

            pub(crate) fn procedure_relocation_bytes(&self) -> &[u8] {
                &self.contents.procedure_relocation_bytes
            }

            pub(crate) fn general_relocation_bytes(&self) -> &[u8] {
                &self.contents.general_relocation_bytes
            }

            pub(crate) fn applied_fixups(&self) -> &[ElfAppliedProcedureLinkageFixup] {
                &self.contents.applications
            }

            pub const fn non_authoritative_resolved_linkage_compatibility_fingerprint(
                &self,
            ) -> u64 {
                self.non_authoritative_resolved_linkage_compatibility_fingerprint
            }

            pub(crate) fn into_envelope(self) -> ValidatedElfDynamicFileEnvelope {
                self.envelope
            }
        }

        #[derive(Debug)]
        #[must_use = "procedure-linkage rejection retains dynamic ELF envelope custody"]
        pub struct ElfProcedureLinkageApplicationError {
            pub(crate) envelope: ValidatedElfDynamicFileEnvelope,
            pub(crate) diagnostic: Diagnostic,
        }

        image::rejection_carrier!(
            ElfProcedureLinkageApplicationError;
            envelope: ValidatedElfDynamicFileEnvelope,
        );
    }

    mod storage_fields {
        use crate::dynamic_executable::checked;
        use crate::dynamic_executable::checked::require;
        use crate::dynamic_executable::file_assembly::dynamic_file_envelope::ValidatedElfDynamicFileEnvelope;
        use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::candidates::ElfResolvedProcedureLinkageContents;
        use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::image_local_relocations::ElfImageLocalRelocatedMemory;
        use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::{
            AARCH64_PLT_HEADER_SIZE, ELF64_GOT_WORD_SIZE, ElfAppliedProcedureLinkageKind,
            ElfAppliedProcedureLinkageStorage, ElfAppliedProcedureLinkageTarget, GOT_PLT_HEADER_WORDS,
            PROCEDURE_LINKAGE_ENTRY_SIZE, X86_PLT_HEADER_SIZE, X86_PLT_LAZY_TAIL_OFFSET,
        };
        use crate::dynamic_executable::load_placement::load_layout::{
            ElfPlacedDynamicSection, ElfPlacedDynamicSectionKind, ValidatedElfDynamicLoadLayout,
        };
        use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::{
            ElfProcedureLinkageFixupKind, ElfProcedureLinkageSemanticTarget,
        };
        use crate::dynamic_executable::section_headers::section_payload_roster::{
            ElfIndexedProcedureFixup, ElfIndexedProcedureFixupStorage, ElfIndexedSectionPayloadContents,
        };
        use crate::dynamic_executable::section_headers::section_roster::ElfDynamicRosterSectionKind;
        use diagnostics::Diagnostic;

        pub(crate) fn storage_address(
            layout: &ValidatedElfDynamicLoadLayout,
            fixup: &ElfIndexedProcedureFixup,
        ) -> Result<u64, Diagnostic> {
            let base = match fixup.storage {
                ElfIndexedProcedureFixupStorage::SourceText => {
                    layout.image_memory().text_virtual_address()
                }
                ElfIndexedProcedureFixupStorage::Section { index, kind } => {
                    let section = exact_section(layout, index)?;
                    require(
                        section.kind() == public_section_kind(kind),
                        "procedure fixup storage kind drifted from absolute layout",
                    )?;
                    section.virtual_address().ok_or_else(|| {
                        Diagnostic::error("procedure fixup storage has no allocated address")
                    })?
                }
            };
            checked::checked_sum_u64(
                base,
                u64::try_from(fixup.byte_offset).map_err(|_| {
                    Diagnostic::error("procedure fixup byte offset exceeds Elf64_Addr")
                })?,
                "procedure fixup source address",
                "Elf64_Addr",
            )
        }

        pub(crate) fn semantic_target_address(
            layout: &ValidatedElfDynamicLoadLayout,
            fixup: &ElfIndexedProcedureFixup,
        ) -> Result<u64, Diagnostic> {
            if let ElfProcedureLinkageSemanticTarget::RelocatedImageSection {
                section,
                byte_offset,
            } = fixup.target
            {
                let base = match section {
                    image::FinalImageSection::Text => layout.image_memory().text_virtual_address(),
                    image::FinalImageSection::Data => layout.image_memory().data_virtual_address(),
                    image::FinalImageSection::Bss => layout.image_memory().bss_virtual_address(),
                    image::FinalImageSection::None => {
                        return Err(Diagnostic::error(
                            "general relocation fixup targets no materialized image section",
                        ));
                    }
                };
                return checked::checked_sum_u64(
                    base,
                    u64::try_from(byte_offset).map_err(|_| {
                        Diagnostic::error("general relocation slot offset exceeds Elf64_Addr")
                    })?,
                    "general relocation target slot address",
                    "Elf64_Addr",
                );
            }
            let section = exact_section(layout, fixup.target_section_index)?;
            let (expected_kind, offset) = match fixup.target {
                ElfProcedureLinkageSemanticTarget::FutureDynamicSection => {
                    (ElfPlacedDynamicSectionKind::DynamicTable, 0)
                }
                ElfProcedureLinkageSemanticTarget::PltHeader => {
                    (ElfPlacedDynamicSectionKind::ProcedureLinkage, 0)
                }
                ElfProcedureLinkageSemanticTarget::PltEntry { logical_ordinal } => (
                    ElfPlacedDynamicSectionKind::ProcedureLinkage,
                    checked::checked_sum_u64(
                        procedure_linkage_header_size(layout)?,
                        checked::checked_product_u64(
                            u64::from(logical_ordinal),
                            PROCEDURE_LINKAGE_ENTRY_SIZE,
                            "procedure-linkage entry offset",
                            "Elf64_Xword",
                        )?,
                        "procedure-linkage entry offset",
                        "Elf64_Addr",
                    )?,
                ),
                ElfProcedureLinkageSemanticTarget::PltLazyTail { logical_ordinal } => (
                    ElfPlacedDynamicSectionKind::ProcedureLinkage,
                    checked::checked_sum_u64(
                        checked::checked_sum_u64(
                            procedure_linkage_header_size(layout)?,
                            checked::checked_product_u64(
                                u64::from(logical_ordinal),
                                PROCEDURE_LINKAGE_ENTRY_SIZE,
                                "procedure-linkage lazy-tail offset",
                                "Elf64_Xword",
                            )?,
                            "procedure-linkage lazy-tail offset",
                            "Elf64_Addr",
                        )?,
                        X86_PLT_LAZY_TAIL_OFFSET,
                        "procedure-linkage lazy-tail offset",
                        "Elf64_Addr",
                    )?,
                ),
                ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index } => (
                    ElfPlacedDynamicSectionKind::ProcedureGot,
                    checked::checked_product_u64(
                        u64::from(word_index),
                        ELF64_GOT_WORD_SIZE,
                        "procedure GOT header-word offset",
                        "Elf64_Xword",
                    )?,
                ),
                ElfProcedureLinkageSemanticTarget::GotPltSlot { logical_ordinal } => (
                    ElfPlacedDynamicSectionKind::ProcedureGot,
                    checked::checked_product_u64(
                        checked::checked_sum_u64(
                            GOT_PLT_HEADER_WORDS,
                            u64::from(logical_ordinal),
                            "procedure GOT slot index",
                            "Elf64_Addr",
                        )?,
                        ELF64_GOT_WORD_SIZE,
                        "procedure GOT slot offset",
                        "Elf64_Xword",
                    )?,
                ),
                ElfProcedureLinkageSemanticTarget::RelocatedImageSection { .. } => {
                    unreachable!("image-section targets resolve through image memory placement")
                }
            };
            require(
                section.kind() == expected_kind,
                "procedure fixup semantic target kind drifted from absolute layout",
            )?;
            require(
                checked::checked_sum_u64(offset, 1, "procedure fixup target extent", "Elf64_Addr")?
                    <= section.byte_size(),
                "procedure fixup semantic target lies outside its exact section",
            )?;
            checked::checked_sum_u64(
                section.virtual_address().ok_or_else(|| {
                    Diagnostic::error("procedure fixup semantic target has no allocated address")
                })?,
                offset,
                "procedure fixup target address",
                "Elf64_Addr",
            )
        }

        fn procedure_linkage_header_size(
            layout: &ValidatedElfDynamicLoadLayout,
        ) -> Result<u64, Diagnostic> {
            match layout.target() {
                target::TargetProfile::LinuxX64 => Ok(X86_PLT_HEADER_SIZE),
                target::TargetProfile::LinuxArm64 => Ok(AARCH64_PLT_HEADER_SIZE),
                _ => Err(Diagnostic::error(
                    "procedure-linkage application requires Linux x86-64 or AArch64",
                )),
            }
        }

        fn exact_section(
            layout: &ValidatedElfDynamicLoadLayout,
            index: u32,
        ) -> Result<&ElfPlacedDynamicSection, Diagnostic> {
            layout
                .sections()
                .get(index as usize)
                .filter(|section| section.index() == index)
                .ok_or_else(|| {
                    Diagnostic::error("procedure fixup references a missing placed section")
                })
        }

        pub(crate) fn encode_field(
            kind: ElfProcedureLinkageFixupKind,
            original: u64,
            mutable_mask: u64,
            source_address: u64,
            target_address: u64,
        ) -> Result<u64, Diagnostic> {
            let mutable_bits = match kind {
                ElfProcedureLinkageFixupKind::X86PcRelative32 => {
                    let next_instruction = checked::checked_sum_u64(
                        source_address,
                        4,
                        "x86-64 relocation PC",
                        "Elf64_Addr",
                    )?;
                    let delta = i128::from(target_address) - i128::from(next_instruction);
                    u64::from(u32::from_le_bytes(
                        i32::try_from(delta)
                            .map_err(|_| {
                                Diagnostic::error(format!(
                                    "x86-64 procedure relocation is out of signed-32 range: {delta} byte(s)"
                                ))
                            })?
                            .to_le_bytes(),
                    ))
                }
                ElfProcedureLinkageFixupKind::Aarch64Page21 => {
                    let source_page = source_address & !0xfff;
                    let target_page = target_address & !0xfff;
                    let page_delta = (i128::from(target_page) - i128::from(source_page)) / 4096;
                    require(
                        (-(1_i128 << 20)..(1_i128 << 20)).contains(&page_delta),
                        "AArch64 procedure ADRP relocation is out of signed page range",
                    )?;
                    let immediate = (page_delta as u32) & 0x1f_ffff;
                    u64::from(((immediate & 0b11) << 29) | (((immediate >> 2) & 0x7ffff) << 5))
                }
                ElfProcedureLinkageFixupKind::Aarch64Load64Low12 => {
                    let page_offset = target_address & 0xfff;
                    require(
                        page_offset.is_multiple_of(8),
                        "AArch64 procedure LDR target is not eight-byte aligned",
                    )?;
                    (page_offset / 8) << 10
                }
                ElfProcedureLinkageFixupKind::Aarch64AddLow12 => (target_address & 0xfff) << 10,
                ElfProcedureLinkageFixupKind::Aarch64Branch26 => {
                    let delta = i128::from(target_address) - i128::from(source_address);
                    require(
                        delta % 4 == 0,
                        "AArch64 procedure branch target is not instruction-aligned",
                    )?;
                    let immediate = delta / 4;
                    require(
                        (-(1_i128 << 25)..(1_i128 << 25)).contains(&immediate),
                        "AArch64 procedure branch relocation is out of signed branch range",
                    )?;
                    (immediate as u64) & 0x03ff_ffff
                }
                ElfProcedureLinkageFixupKind::Absolute64
                | ElfProcedureLinkageFixupKind::Elf64RelaOffset => target_address,
            };
            require(
                mutable_bits & !mutable_mask == 0,
                "procedure relocation encoding exceeds its typed mutable mask",
            )?;
            Ok((original & !mutable_mask) | mutable_bits)
        }

        pub(crate) fn decode_rejoins_target(
            kind: ElfProcedureLinkageFixupKind,
            field: u64,
            source_address: u64,
            target_address: u64,
        ) -> Result<(), Diagnostic> {
            let rejoins = match kind {
                ElfProcedureLinkageFixupKind::X86PcRelative32 => {
                    let displacement = i32::from_le_bytes((field as u32).to_le_bytes());
                    checked_add_signed(
                        checked::checked_sum_u64(
                            source_address,
                            4,
                            "decoded x86-64 relocation PC",
                            "Elf64_Addr",
                        )?,
                        i128::from(displacement),
                        "decoded x86-64 procedure target",
                    )? == target_address
                }
                ElfProcedureLinkageFixupKind::Aarch64Page21 => {
                    let word = field as u32;
                    let immediate = ((word >> 29) & 0b11) | (((word >> 5) & 0x7ffff) << 2);
                    let page_delta = sign_extend(u64::from(immediate), 21) * 4096;
                    checked_add_signed(
                        source_address & !0xfff,
                        page_delta,
                        "decoded AArch64 ADRP target page",
                    )? == target_address & !0xfff
                }
                ElfProcedureLinkageFixupKind::Aarch64Load64Low12 => {
                    (((field >> 10) & 0xfff) * 8) == target_address & 0xfff
                }
                ElfProcedureLinkageFixupKind::Aarch64AddLow12 => {
                    ((field >> 10) & 0xfff) == target_address & 0xfff
                }
                ElfProcedureLinkageFixupKind::Aarch64Branch26 => {
                    let immediate = sign_extend(field & 0x03ff_ffff, 26) * 4;
                    checked_add_signed(source_address, immediate, "decoded AArch64 branch target")?
                        == target_address
                }
                ElfProcedureLinkageFixupKind::Absolute64
                | ElfProcedureLinkageFixupKind::Elf64RelaOffset => field == target_address,
            };
            require(
                rejoins,
                "decoded procedure-linkage field does not rejoin its exact semantic target",
            )
        }

        const fn sign_extend(value: u64, bits: u32) -> i128 {
            let shift = 128 - bits;
            ((value as i128) << shift) >> shift
        }

        fn checked_add_signed(
            base: u64,
            delta: i128,
            context: &'static str,
        ) -> Result<u64, Diagnostic> {
            let value = i128::from(base)
                .checked_add(delta)
                .filter(|value| (0..=i128::from(u64::MAX)).contains(value))
                .ok_or_else(|| Diagnostic::error(format!("{context} overflows Elf64_Addr")))?;
            Ok(value as u64)
        }

        pub(crate) fn upstream_storage_bytes<'a>(
            envelope: &'a ValidatedElfDynamicFileEnvelope,
            relocated: &'a ElfImageLocalRelocatedMemory,
            storage: ElfAppliedProcedureLinkageStorage,
        ) -> Result<&'a [u8], Diagnostic> {
            let payloads = indexed_payloads(envelope);
            match storage {
                ElfAppliedProcedureLinkageStorage::SourceText => Ok(&relocated.text),
                ElfAppliedProcedureLinkageStorage::ProcedureLinkage => {
                    indexed_row_bytes(payloads, 8, ElfDynamicRosterSectionKind::ProcedureLinkage)
                }
                ElfAppliedProcedureLinkageStorage::ProcedureGot => {
                    indexed_row_bytes(payloads, 9, ElfDynamicRosterSectionKind::ProcedureGot)
                }
                ElfAppliedProcedureLinkageStorage::ProcedureRelocation => indexed_row_bytes(
                    payloads,
                    10,
                    ElfDynamicRosterSectionKind::ProcedureRelocation,
                ),
                ElfAppliedProcedureLinkageStorage::GeneralRelocation => {
                    indexed_row_bytes(payloads, 11, ElfDynamicRosterSectionKind::GeneralRelocation)
                }
            }
        }

        pub(crate) fn indexed_row_bytes(
            payloads: &ElfIndexedSectionPayloadContents,
            index: u32,
            kind: ElfDynamicRosterSectionKind,
        ) -> Result<&[u8], Diagnostic> {
            payloads
                .rows
                .get(index as usize)
                .filter(|row| row.index == index && row.kind == kind)
                .map(|row| row.bytes.as_slice())
                .ok_or_else(|| Diagnostic::error("procedure-linkage storage row is missing"))
        }

        pub(crate) fn storage_bytes(
            contents: &ElfResolvedProcedureLinkageContents,
            storage: ElfAppliedProcedureLinkageStorage,
        ) -> &[u8] {
            match storage {
                ElfAppliedProcedureLinkageStorage::SourceText => &contents.source_text_bytes,
                ElfAppliedProcedureLinkageStorage::ProcedureLinkage => {
                    &contents.procedure_linkage_bytes
                }
                ElfAppliedProcedureLinkageStorage::ProcedureGot => &contents.procedure_got_bytes,
                ElfAppliedProcedureLinkageStorage::ProcedureRelocation => {
                    &contents.procedure_relocation_bytes
                }
                ElfAppliedProcedureLinkageStorage::GeneralRelocation => {
                    &contents.general_relocation_bytes
                }
            }
        }

        pub(crate) fn storage_bytes_mut(
            contents: &mut ElfResolvedProcedureLinkageContents,
            storage: ElfAppliedProcedureLinkageStorage,
        ) -> &mut [u8] {
            match storage {
                ElfAppliedProcedureLinkageStorage::SourceText => &mut contents.source_text_bytes,
                ElfAppliedProcedureLinkageStorage::ProcedureLinkage => {
                    &mut contents.procedure_linkage_bytes
                }
                ElfAppliedProcedureLinkageStorage::ProcedureGot => {
                    &mut contents.procedure_got_bytes
                }
                ElfAppliedProcedureLinkageStorage::ProcedureRelocation => {
                    &mut contents.procedure_relocation_bytes
                }
                ElfAppliedProcedureLinkageStorage::GeneralRelocation => {
                    &mut contents.general_relocation_bytes
                }
            }
        }

        pub(crate) fn write_field(
            bytes: &mut [u8],
            offset: usize,
            width: u8,
            value: u64,
        ) -> Result<(), Diagnostic> {
            let end =
                checked::checked_sum(offset, usize::from(width), "procedure fixup field end")?;
            let field = bytes.get_mut(offset..end).ok_or_else(|| {
                Diagnostic::error("procedure fixup field exceeds its exact storage")
            })?;
            match width {
                4 => field.copy_from_slice(
                    &u32::try_from(value)
                        .map_err(|_| {
                            Diagnostic::error("procedure fixup value exceeds 32-bit field")
                        })?
                        .to_le_bytes(),
                ),
                8 => field.copy_from_slice(&value.to_le_bytes()),
                _ => {
                    return Err(Diagnostic::error(
                        "procedure fixup has an unsupported field width",
                    ));
                }
            }
            Ok(())
        }

        pub(crate) fn public_storage(
            storage: ElfIndexedProcedureFixupStorage,
        ) -> Result<ElfAppliedProcedureLinkageStorage, Diagnostic> {
            match storage {
                ElfIndexedProcedureFixupStorage::SourceText => {
                    Ok(ElfAppliedProcedureLinkageStorage::SourceText)
                }
                ElfIndexedProcedureFixupStorage::Section { kind, .. } => match kind {
                    ElfDynamicRosterSectionKind::ProcedureLinkage => {
                        Ok(ElfAppliedProcedureLinkageStorage::ProcedureLinkage)
                    }
                    ElfDynamicRosterSectionKind::ProcedureGot => {
                        Ok(ElfAppliedProcedureLinkageStorage::ProcedureGot)
                    }
                    ElfDynamicRosterSectionKind::ProcedureRelocation => {
                        Ok(ElfAppliedProcedureLinkageStorage::ProcedureRelocation)
                    }
                    ElfDynamicRosterSectionKind::GeneralRelocation => {
                        Ok(ElfAppliedProcedureLinkageStorage::GeneralRelocation)
                    }
                    _ => Err(Diagnostic::error(
                        "procedure fixup names a non-procedure storage section",
                    )),
                },
            }
        }

        pub(crate) const fn public_kind(
            kind: ElfProcedureLinkageFixupKind,
        ) -> ElfAppliedProcedureLinkageKind {
            kind
        }

        pub(crate) const fn public_target(
            target: ElfProcedureLinkageSemanticTarget,
        ) -> ElfAppliedProcedureLinkageTarget {
            match target {
                ElfProcedureLinkageSemanticTarget::FutureDynamicSection => {
                    ElfAppliedProcedureLinkageTarget::DynamicSection
                }
                ElfProcedureLinkageSemanticTarget::PltHeader => {
                    ElfAppliedProcedureLinkageTarget::ProcedureLinkageHeader
                }
                ElfProcedureLinkageSemanticTarget::PltEntry { logical_ordinal } => {
                    ElfAppliedProcedureLinkageTarget::ProcedureLinkageEntry { logical_ordinal }
                }
                ElfProcedureLinkageSemanticTarget::PltLazyTail { logical_ordinal } => {
                    ElfAppliedProcedureLinkageTarget::ProcedureLinkageLazyTail { logical_ordinal }
                }
                ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index } => {
                    ElfAppliedProcedureLinkageTarget::ProcedureGotHeaderWord { word_index }
                }
                ElfProcedureLinkageSemanticTarget::GotPltSlot { logical_ordinal } => {
                    ElfAppliedProcedureLinkageTarget::ProcedureGotSlot { logical_ordinal }
                }
                ElfProcedureLinkageSemanticTarget::RelocatedImageSection {
                    section,
                    byte_offset,
                } => ElfAppliedProcedureLinkageTarget::RelocatedImageSection {
                    section,
                    byte_offset,
                },
            }
        }

        const fn public_section_kind(
            kind: ElfDynamicRosterSectionKind,
        ) -> ElfPlacedDynamicSectionKind {
            kind
        }

        pub(crate) fn load_layout(
            envelope: &ValidatedElfDynamicFileEnvelope,
        ) -> &ValidatedElfDynamicLoadLayout {
            envelope
                .resolved_dynamic_table()
                .placed_section_headers()
                .load_layout()
        }

        pub(crate) fn indexed_payloads(
            envelope: &ValidatedElfDynamicFileEnvelope,
        ) -> &ElfIndexedSectionPayloadContents {
            load_layout(envelope).relative().payloads().contents()
        }
    }

    pub(crate) use linkage_fixups::{
        ElfAppliedProcedureLinkageFixup, ElfAppliedProcedureLinkageKind,
        ElfAppliedProcedureLinkageStorage, ElfAppliedProcedureLinkageTarget,
        ElfProcedureLinkageApplicationError, ValidatedElfResolvedProcedureLinkage,
    };

    use crate::dynamic_executable::file_assembly::dynamic_file_envelope::ValidatedElfDynamicFileEnvelope;
    use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::compatibility_fingerprint::non_authoritative_resolved_linkage_compatibility_fingerprint;
    use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::candidates::{derive_contents, Candidate, validate_candidate};
    use super::super::checked;

    const X86_PLT_HEADER_SIZE: u64 = 16;

    const AARCH64_PLT_HEADER_SIZE: u64 = 32;

    const PROCEDURE_LINKAGE_ENTRY_SIZE: u64 = 16;

    const X86_PLT_LAZY_TAIL_OFFSET: u64 = 6;

    const GOT_PLT_HEADER_WORDS: u64 = 3;

    const ELF64_GOT_WORD_SIZE: u64 = 8;

    pub(crate) fn apply_elf_procedure_linkage_fixups(
        envelope: ValidatedElfDynamicFileEnvelope,
    ) -> Result<ValidatedElfResolvedProcedureLinkage, Box<ElfProcedureLinkageApplicationError>>
    {
        checked::plan_stage!(
            plan: envelope,
            derive: derive_contents,
            contents: contents,
            fingerprint: non_authoritative_resolved_linkage_compatibility_fingerprint,
            error: ElfProcedureLinkageApplicationError,
        )
    }

    #[cfg(test)]
    mod tests {
        use super::{
            ElfAppliedProcedureLinkageFixup, ElfAppliedProcedureLinkageKind,
            ElfAppliedProcedureLinkageStorage, ElfAppliedProcedureLinkageTarget,
            ValidatedElfDynamicFileEnvelope, apply_elf_procedure_linkage_fixups,
            non_authoritative_resolved_linkage_compatibility_fingerprint,
        };
        use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::candidates::{
            Candidate, derive_contents, validate_candidate, validate_nonoverlapping_applications,
        };
        use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::storage_fields::{
            encode_field, load_layout,
        };
        use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::ElfProcedureLinkageFixupKind;
        use crate::{
            apply_elf_dynamic_address_fixups, apply_elf_section_header_placements,
            plan_elf_dynamic_load_layout, plan_elf_indexed_section_payloads,
            plan_elf_relative_section_payload_layout, serialize_elf_dynamic_file_envelope,
        };
        use arena::Handle;
        use image::{FinalImage, FinalImageMemory, FinalImageRelocation, FinalImageSection};
        use object_file::RelocationKind;
        use target::TargetProfile;

        fn standard_envelope(target: TargetProfile) -> ValidatedElfDynamicFileEnvelope {
            envelope(
                target,
                [b"alpha_call".as_slice(), b"beta_call".as_slice()],
                &[],
            )
        }

        fn envelope(
            target: TargetProfile,
            imported_symbols: [&[u8]; 2],
            general_slots: &[(usize, i64)],
        ) -> ValidatedElfDynamicFileEnvelope {
            let mut image = FinalImage::with_capacity(
                target.native_target(),
                FinalImageMemory {
                    text: vec![0; 64],
                    data: vec![0x5a; 13],
                    bss_size: 23,
                    bss_alignment: 16,
                },
                Handle::invalid(),
                3,
                2,
                3 + general_slots.len(),
            );
            crate::test_support::text_entry(&mut image, 24);

            let mut imports = Vec::new();
            for (index, symbol) in imported_symbols.into_iter().enumerate() {
                imports.push(crate::test_support::versioned_import(
                    &mut image,
                    &format!("__omega_resolved_import_{index}"),
                    b"libresolved-procedure.so",
                    symbol,
                    b"RESOLVED_PROCEDURE_1",
                    target,
                    "versioned import locator",
                ));
            }
            crate::test_support::insert_text_call_relocations(
                &mut image,
                target,
                [(0, imports[0]), (8, imports[0]), (16, imports[1])],
            );
            for &(slot_offset, slot_addend) in general_slots {
                image
                    .relocation_table
                    .relocations
                    .insert(FinalImageRelocation {
                        section: FinalImageSection::Data,
                        offset: slot_offset,
                        byte_width: 8,
                        symbol_handle: imports[0],
                        addend: slot_addend,
                        kind: RelocationKind::Absolute64,
                    });
            }

            let linkage = crate::test_support::resolved_linkage(image, target);
            let headers = crate::test_support::section_header_table_from_linkage(linkage);
            let payloads = plan_elf_indexed_section_payloads(headers).unwrap();
            let relative = plan_elf_relative_section_payload_layout(payloads).unwrap();
            let load = plan_elf_dynamic_load_layout(relative).unwrap();
            let placed = apply_elf_section_header_placements(load).unwrap();
            let resolved = apply_elf_dynamic_address_fixups(placed).unwrap();
            serialize_elf_dynamic_file_envelope(resolved).unwrap()
        }

        fn candidate(target: TargetProfile) -> Candidate {
            let envelope = standard_envelope(target);
            let contents = derive_contents(&envelope).unwrap();
            let non_authoritative_resolved_linkage_compatibility_fingerprint =
                non_authoritative_resolved_linkage_compatibility_fingerprint(&envelope, &contents);
            Candidate {
                envelope,
                contents,
                non_authoritative_resolved_linkage_compatibility_fingerprint,
            }
        }

        fn refresh_fingerprint(candidate: &mut Candidate) {
            candidate.non_authoritative_resolved_linkage_compatibility_fingerprint =
                non_authoritative_resolved_linkage_compatibility_fingerprint(
                    &candidate.envelope,
                    &candidate.contents,
                );
        }

        fn assert_rejected_with_custody(candidate: Candidate) {
            let upstream = candidate
                .envelope
                .non_authoritative_envelope_compatibility_fingerprint();
            let error = validate_candidate(candidate).unwrap_err();
            assert_eq!(
                error
                    .candidate
                    .envelope
                    .non_authoritative_envelope_compatibility_fingerprint(),
                upstream,
            );
            assert!(!error.diagnostic.to_string().is_empty());
        }

        #[test]
        fn both_linux_targets_apply_every_exact_procedure_and_source_fixup() {
            for (target, expected_count) in [
                (TargetProfile::LinuxX64, 14),
                (TargetProfile::LinuxArm64, 16),
            ] {
                let envelope = standard_envelope(target);
                let original_text = load_layout(&envelope).retained_image().memory.text.clone();
                let resolved = apply_elf_procedure_linkage_fixups(envelope).unwrap();
                assert_eq!(resolved.applied_fixups().len(), expected_count);
                assert_ne!(
                    resolved.non_authoritative_resolved_linkage_compatibility_fingerprint(),
                    0,
                );
                assert_eq!(resolved.source_text_bytes().len(), original_text.len());
                assert_ne!(resolved.source_text_bytes(), original_text);
                assert_eq!(
                    resolved
                        .applied_fixups()
                        .iter()
                        .map(ElfAppliedProcedureLinkageFixup::ordinal)
                        .collect::<Vec<_>>(),
                    (0..expected_count as u32).collect::<Vec<_>>(),
                );
                for storage in [
                    ElfAppliedProcedureLinkageStorage::SourceText,
                    ElfAppliedProcedureLinkageStorage::ProcedureLinkage,
                    ElfAppliedProcedureLinkageStorage::ProcedureGot,
                    ElfAppliedProcedureLinkageStorage::ProcedureRelocation,
                ] {
                    assert!(
                        resolved
                            .applied_fixups()
                            .iter()
                            .any(|application| application.storage() == storage),
                    );
                }
                assert_eq!(
                    load_layout(resolved.envelope())
                        .retained_image()
                        .memory
                        .text,
                    original_text,
                    "the retained FinalImage must remain immutable",
                );
            }
        }

        #[test]
        fn data_slot_import_relocations_emit_and_apply_exact_general_rela_rows() {
            for (target, relocation_type) in [
                (TargetProfile::LinuxX64, 1_u64),
                (TargetProfile::LinuxArm64, 257_u64),
            ] {
                let resolved = apply_elf_procedure_linkage_fixups(envelope(
                    target,
                    [b"alpha_call", b"beta_call"],
                    &[(0, 7)],
                ))
                .unwrap();
                let rela = resolved.general_relocation_bytes();
                assert_eq!(rela.len(), 24);
                let r_offset = u64::from_le_bytes(rela[0..8].try_into().unwrap());
                let r_info = u64::from_le_bytes(rela[8..16].try_into().unwrap());
                let r_addend = i64::from_le_bytes(rela[16..24].try_into().unwrap());
                let layout = load_layout(resolved.envelope());
                assert_eq!(r_offset, layout.image_memory().data_virtual_address());
                assert_eq!(r_info, (1_u64 << 32) | relocation_type);
                assert_eq!(r_addend, 7);
                let fixup = resolved
                    .applied_fixups()
                    .iter()
                    .find(|fixup| {
                        fixup.storage() == ElfAppliedProcedureLinkageStorage::GeneralRelocation
                    })
                    .expect("one .rela.dyn r_offset fixup");
                assert_eq!(
                    fixup.kind(),
                    ElfAppliedProcedureLinkageKind::Elf64RelaOffset
                );
                assert_eq!(fixup.byte_offset(), 0);
                assert_eq!(fixup.byte_width(), 8);
                assert_eq!(
                    fixup.target(),
                    ElfAppliedProcedureLinkageTarget::RelocatedImageSection {
                        section: FinalImageSection::Data,
                        byte_offset: 0,
                    }
                );
                assert_eq!(
                    fixup.target_address(),
                    layout.image_memory().data_virtual_address()
                );
                assert_eq!(fixup.encoded_field(), r_offset);
            }
        }

        #[test]
        fn exact_replay_is_deterministic_and_target_or_import_bound() {
            let first =
                apply_elf_procedure_linkage_fixups(standard_envelope(TargetProfile::LinuxX64))
                    .unwrap();
            let replay =
                apply_elf_procedure_linkage_fixups(standard_envelope(TargetProfile::LinuxX64))
                    .unwrap();
            let target_change =
                apply_elf_procedure_linkage_fixups(standard_envelope(TargetProfile::LinuxArm64))
                    .unwrap();
            let import_change = apply_elf_procedure_linkage_fixups(envelope(
                TargetProfile::LinuxX64,
                [b"alpha_call", b"gamma_call"],
                &[],
            ))
            .unwrap();
            assert_eq!(first.source_text_bytes(), replay.source_text_bytes());
            assert_eq!(
                first.procedure_linkage_bytes(),
                replay.procedure_linkage_bytes(),
            );
            assert_eq!(first.applied_fixups(), replay.applied_fixups());
            assert_eq!(
                first.non_authoritative_resolved_linkage_compatibility_fingerprint(),
                replay.non_authoritative_resolved_linkage_compatibility_fingerprint(),
            );
            assert_ne!(
                first.non_authoritative_resolved_linkage_compatibility_fingerprint(),
                target_change.non_authoritative_resolved_linkage_compatibility_fingerprint(),
            );
            assert_ne!(
                first.non_authoritative_resolved_linkage_compatibility_fingerprint(),
                import_change.non_authoritative_resolved_linkage_compatibility_fingerprint(),
            );
        }

        #[test]
        fn byte_opcode_address_and_ledger_substitutions_reject_with_custody() {
            let mut mutable_field = candidate(TargetProfile::LinuxX64);
            mutable_field.contents.source_text_bytes[1] ^= 1;
            refresh_fingerprint(&mut mutable_field);
            assert_rejected_with_custody(mutable_field);

            let mut fixed_opcode = candidate(TargetProfile::LinuxX64);
            fixed_opcode.contents.source_text_bytes[0] ^= 1;
            refresh_fingerprint(&mut fixed_opcode);
            assert_rejected_with_custody(fixed_opcode);

            let mut target = candidate(TargetProfile::LinuxX64);
            target.contents.applications[0].target_address += 4;
            refresh_fingerprint(&mut target);
            assert_rejected_with_custody(target);

            let mut source = candidate(TargetProfile::LinuxX64);
            source.contents.applications[0].source_address += 4;
            refresh_fingerprint(&mut source);
            assert_rejected_with_custody(source);

            let mut kind = candidate(TargetProfile::LinuxX64);
            kind.contents.applications[0].kind = ElfAppliedProcedureLinkageKind::Absolute64;
            refresh_fingerprint(&mut kind);
            assert_rejected_with_custody(kind);
        }

        #[test]
        fn missing_duplicate_reordered_truncated_and_report_drift_reject_with_custody() {
            let mut missing = candidate(TargetProfile::LinuxArm64);
            missing.contents.applications.pop();
            refresh_fingerprint(&mut missing);
            assert_rejected_with_custody(missing);

            let mut duplicate = candidate(TargetProfile::LinuxArm64);
            duplicate.contents.applications[1] = duplicate.contents.applications[0];
            refresh_fingerprint(&mut duplicate);
            assert_rejected_with_custody(duplicate);

            let mut reordered = candidate(TargetProfile::LinuxArm64);
            reordered.contents.applications.swap(0, 1);
            refresh_fingerprint(&mut reordered);
            assert_rejected_with_custody(reordered);

            let mut truncated = candidate(TargetProfile::LinuxArm64);
            truncated.contents.procedure_linkage_bytes.pop();
            refresh_fingerprint(&mut truncated);
            assert_rejected_with_custody(truncated);

            let mut zero = candidate(TargetProfile::LinuxArm64);
            zero.non_authoritative_resolved_linkage_compatibility_fingerprint = 0;
            assert_rejected_with_custody(zero);

            let mut drifted = candidate(TargetProfile::LinuxArm64);
            drifted.non_authoritative_resolved_linkage_compatibility_fingerprint ^= 1;
            assert_rejected_with_custody(drifted);
        }

        #[test]
        fn range_alignment_mask_and_overlap_helpers_fail_closed() {
            assert!(
                encode_field(
                    ElfProcedureLinkageFixupKind::X86PcRelative32,
                    0,
                    0xffff_ffff,
                    0,
                    i32::MAX as u64 + 5,
                )
                .is_err(),
            );
            assert!(
                encode_field(
                    ElfProcedureLinkageFixupKind::Aarch64Branch26,
                    0x9400_0000,
                    0x03ff_ffff,
                    0x1000,
                    0x1002,
                )
                .is_err(),
            );
            assert!(
                encode_field(
                    ElfProcedureLinkageFixupKind::Aarch64Branch26,
                    0x9400_0000,
                    0x03ff_ffff,
                    0,
                    1 << 27,
                )
                .is_err(),
            );
            assert!(
                encode_field(
                    ElfProcedureLinkageFixupKind::Aarch64Page21,
                    0x9000_0010,
                    0x60ff_ffe0,
                    0,
                    1_u64 << 32,
                )
                .is_err(),
            );
            assert!(
                encode_field(
                    ElfProcedureLinkageFixupKind::Aarch64Load64Low12,
                    0xf940_0211,
                    0x003f_fc00,
                    0,
                    3,
                )
                .is_err(),
            );
            assert!(
                encode_field(
                    ElfProcedureLinkageFixupKind::Absolute64,
                    0,
                    0xffff,
                    0,
                    1 << 20,
                )
                .is_err(),
            );

            let first = candidate(TargetProfile::LinuxX64).contents.applications[0];
            let mut overlapping = first;
            overlapping.ordinal += 1;
            assert!(validate_nonoverlapping_applications(&[first, overlapping]).is_err());
        }
    }
}
