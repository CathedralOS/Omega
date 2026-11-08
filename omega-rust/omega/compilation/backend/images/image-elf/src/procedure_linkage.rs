/*
We execute stages 5 through 7 of the dynamic ELF pipeline: planning procedure
linkage relocations (`plan_elf_procedure_linkage_relocations`), generating
architecture-specific `.plt` and `.got.plt` templates for x86-64 and AArch64
(`plan_elf_procedure_linkage_templates`), and planning their section
descriptors (`plan_elf_procedure_linkage_section_descriptors`).
*/

pub(crate) mod dynamic_import_relocations {
    use super::super::checked::{self, CandidateValidationError};
    use crate::dynamic_executable::checked::{checked_u32, require};
    use crate::dynamic_executable::import_sections::dynamic_section_descriptors::ValidatedElfDynamicSectionDescriptorPlan;
    use crate::dynamic_executable::import_sections::dynamic_sections::ElfDynamicImportBinding;
    use crate::imports::{ElfImportLocator, ElfImportRequest};
    use diagnostics::Diagnostic;
    use image::{FinalImageRelocation, FinalImageSection};
    use object_file::{RelocationKind, relocation_kind_tag};
    use target::TargetProfile;

    const R_X86_64_JUMP_SLOT: u32 = 7;
    const R_AARCH64_JUMP_SLOT: u32 = 1026;
    const R_X86_64_64: u32 = 1;
    const R_AARCH64_ABS64: u32 = 257;
    const ELF64_GENERAL_SLOT_WIDTH: usize = 8;
    #[derive(Debug)]
    #[must_use = "validated procedure linkage retains the exact descriptor plan"]
    pub struct ValidatedElfProcedureLinkageRelocationPlan {
        descriptors: ValidatedElfDynamicSectionDescriptorPlan,
        contents: ElfProcedureLinkageRelocationContents,
        non_authoritative_linkage_compatibility_fingerprint: u64,
    }

    impl ValidatedElfProcedureLinkageRelocationPlan {
        pub const fn descriptors(&self) -> &ValidatedElfDynamicSectionDescriptorPlan {
            &self.descriptors
        }

        #[cfg(test)]
        pub(crate) fn logical_slot_count(&self) -> usize {
            self.contents.slots.len()
        }

        pub(crate) fn procedure_relocation_count(&self) -> usize {
            self.contents.jump_slot_relocations.len()
        }

        #[cfg(test)]
        pub(crate) fn direct_call_site_count(&self) -> usize {
            self.contents
                .slots
                .iter()
                .map(|slot| slot.call_sites.len())
                .sum()
        }

        pub(crate) fn general_dynamic_relocation_count(&self) -> usize {
            self.contents.general_relocations.len()
        }

        pub const fn non_authoritative_linkage_compatibility_fingerprint(&self) -> u64 {
            self.non_authoritative_linkage_compatibility_fingerprint
        }

        pub(crate) const fn contents(&self) -> &ElfProcedureLinkageRelocationContents {
            &self.contents
        }

        pub(crate) fn into_parts(
            self,
        ) -> (
            ValidatedElfDynamicSectionDescriptorPlan,
            ElfProcedureLinkageRelocationContents,
        ) {
            (self.descriptors, self.contents)
        }
    }

    #[derive(Debug)]
    #[must_use = "ELF procedure-linkage rejection retains the descriptor plan"]
    pub struct ElfProcedureLinkageRelocationPlanningError {
        descriptors: ValidatedElfDynamicSectionDescriptorPlan,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(
        ElfProcedureLinkageRelocationPlanningError;
        descriptors: ValidatedElfDynamicSectionDescriptorPlan,
    );

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfProcedureLinkageRelocationContents {
        pub(crate) slots: Vec<ElfLogicalProcedureLinkageSlot>,
        pub(crate) jump_slot_relocations: Vec<ElfSemanticJumpSlotRelocation>,
        pub(crate) general_relocations: Vec<ElfSemanticGeneralRelocation>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfLogicalProcedureLinkageSlot {
        pub(crate) logical_ordinal: u32,
        pub(crate) request_index: usize,
        pub(crate) compatibility_report_identity: u64,
        pub(crate) dynamic_symbol_index: u32,
        pub(crate) version_index: u16,
        pub(crate) call_sites: Vec<ElfDirectImportCallSite>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfDirectImportCallSite {
        pub(crate) instruction_offset: usize,
        pub(crate) relocation_offset: usize,
        pub(crate) byte_width: usize,
        pub(crate) kind: RelocationKind,
        pub(crate) addend: i64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfSemanticJumpSlotRelocation {
        pub(crate) logical_got_slot_ordinal: u32,
        pub(crate) dynamic_symbol_index: u32,
        pub(crate) relocation_type: u32,
        pub(crate) addend: i64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfSemanticGeneralRelocation {
        pub(crate) request_index: usize,
        pub(crate) dynamic_symbol_index: u32,
        pub(crate) relocation_type: u32,
        pub(crate) source_section: FinalImageSection,
        pub(crate) source_offset: usize,
        pub(crate) addend: i64,
    }

    struct Candidate {
        descriptors: ValidatedElfDynamicSectionDescriptorPlan,
        contents: ElfProcedureLinkageRelocationContents,
        non_authoritative_linkage_compatibility_fingerprint: u64,
    }

    pub(crate) fn plan_elf_procedure_linkage_relocations(
        descriptors: ValidatedElfDynamicSectionDescriptorPlan,
    ) -> Result<
        ValidatedElfProcedureLinkageRelocationPlan,
        Box<ElfProcedureLinkageRelocationPlanningError>,
    > {
        checked::plan_stage!(
            plan: descriptors,
            derive: derive_contents,
            contents: contents,
            fingerprint: non_authoritative_linkage_compatibility_fingerprint,
            error: ElfProcedureLinkageRelocationPlanningError,
        )
    }

    fn derive_contents(
        descriptors: &ValidatedElfDynamicSectionDescriptorPlan,
    ) -> Result<ElfProcedureLinkageRelocationContents, Diagnostic> {
        let plan = descriptors.payloads().plan();
        let bindings = &plan.contents().bindings;
        let inputs = plan.inputs();
        require(
            bindings.len() == inputs.imports().len(),
            "ELF dynamic bindings do not cover every canonical import request",
        )?;
        let (source_kind, relocation_type, general_type) =
            target_relocation_spec(inputs.interpreter().target())?;
        let mut slots = Vec::with_capacity(bindings.len());
        let mut jump_slot_relocations = Vec::with_capacity(bindings.len());
        let mut general_relocations = Vec::new();
        for (index, binding) in bindings.iter().enumerate() {
            let logical_ordinal = checked_u32(index, "logical procedure-linkage slot ordinal")?;
            let request = inputs.imports().get(binding.request_index).ok_or_else(|| {
                Diagnostic::error("ELF dynamic binding request index exceeds canonical imports")
            })?;
            let compatibility_report_identity = request_compatibility_report_identity(request)?;
            require(
                compatibility_report_identity == binding.compatibility_report_identity,
                "ELF dynamic binding report identity does not match its canonical import request",
            )?;
            let (call_sites, mut general) = canonical_relocation_classes(
                inputs.interpreter().target(),
                &inputs.image().memory.text,
                inputs.image().memory.data.len(),
                source_kind,
                general_type,
                binding,
                request,
            )?;
            general_relocations.append(&mut general);
            slots.push(ElfLogicalProcedureLinkageSlot {
                logical_ordinal,
                request_index: binding.request_index,
                compatibility_report_identity,
                dynamic_symbol_index: binding.dynamic_symbol_index,
                version_index: binding.version_index,
                call_sites,
            });
            jump_slot_relocations.push(ElfSemanticJumpSlotRelocation {
                logical_got_slot_ordinal: logical_ordinal,
                dynamic_symbol_index: binding.dynamic_symbol_index,
                relocation_type,
                addend: 0,
            });
        }
        general_relocations.sort_unstable_by_key(|relocation| relocation.source_offset);
        let contents = ElfProcedureLinkageRelocationContents {
            slots,
            jump_slot_relocations,
            general_relocations,
        };
        validate_nonoverlapping_call_sites(&contents)?;
        validate_nonoverlapping_general_slots(&contents)?;
        Ok(contents)
    }

    fn target_relocation_spec(
        target: TargetProfile,
    ) -> Result<(RelocationKind, u32, u32), Diagnostic> {
        match target {
            TargetProfile::LinuxX64 => Ok((
                RelocationKind::X86_64Relative32,
                R_X86_64_JUMP_SLOT,
                R_X86_64_64,
            )),
            TargetProfile::LinuxArm64 => Ok((
                RelocationKind::Aarch64Branch26,
                R_AARCH64_JUMP_SLOT,
                R_AARCH64_ABS64,
            )),
            _ => Err(Diagnostic::error(
                "ELF procedure linkage requires an exact Linux x86-64 or AArch64 profile",
            )),
        }
    }

    fn request_compatibility_report_identity(
        request: &ElfImportRequest,
    ) -> Result<u64, Diagnostic> {
        match &request.locator {
            ElfImportLocator::Versioned {
                compatibility_report_identity,
                ..
            } => Ok(*compatibility_report_identity),
            ElfImportLocator::Component { locator, .. } => {
                Ok(locator.non_authoritative_compatibility_fingerprint())
            }
            ElfImportLocator::StringBackedBootstrap { .. } => Err(Diagnostic::error(
                "string-backed import reached normalized ELF procedure linkage",
            )),
        }
    }

    fn canonical_relocation_classes(
        target: TargetProfile,
        text: &[u8],
        data_len: usize,
        expected_kind: RelocationKind,
        general_type: u32,
        binding: &ElfDynamicImportBinding,
        request: &ElfImportRequest,
    ) -> Result<
        (
            Vec<ElfDirectImportCallSite>,
            Vec<ElfSemanticGeneralRelocation>,
        ),
        Diagnostic,
    > {
        require(
            !request.relocations.is_empty(),
            "ELF procedure-linkage import has no retained relocations",
        )?;
        let mut sites = Vec::new();
        let mut general = Vec::new();
        for relocation in &request.relocations {
            if is_direct_call_shape(relocation, expected_kind) {
                sites.push(call_site(target, text, expected_kind, relocation)?);
            } else {
                general.push(general_slot(data_len, general_type, binding, relocation)?);
            }
        }
        sites.sort_unstable_by_key(|site| (site.instruction_offset, site.relocation_offset));
        validate_site_spans(&sites)?;
        Ok((sites, general))
    }

    fn is_direct_call_shape(
        relocation: &FinalImageRelocation,
        expected_kind: RelocationKind,
    ) -> bool {
        relocation.section == FinalImageSection::Text
            && relocation.kind == expected_kind
            && relocation.byte_width == 4
            && relocation.addend == 0
    }

    fn general_slot(
        data_len: usize,
        general_type: u32,
        binding: &ElfDynamicImportBinding,
        relocation: &FinalImageRelocation,
    ) -> Result<ElfSemanticGeneralRelocation, Diagnostic> {
        require(
            relocation.section == FinalImageSection::Data
                && relocation.kind == RelocationKind::Absolute64
                && relocation.byte_width == ELF64_GENERAL_SLOT_WIDTH
                && relocation.offset.is_multiple_of(ELF64_GENERAL_SLOT_WIDTH),
            "ELF dynamic import requires a non-procedure or malformed source relocation",
        )?;
        relocation
            .offset
            .checked_add(ELF64_GENERAL_SLOT_WIDTH)
            .filter(|end| *end <= data_len)
            .ok_or_else(|| Diagnostic::error("ELF dynamic import data slot exceeds .data"))?;
        Ok(ElfSemanticGeneralRelocation {
            request_index: binding.request_index,
            dynamic_symbol_index: binding.dynamic_symbol_index,
            relocation_type: general_type,
            source_section: relocation.section,
            source_offset: relocation.offset,
            addend: relocation.addend,
        })
    }

    fn call_site(
        target: TargetProfile,
        text: &[u8],
        expected_kind: RelocationKind,
        relocation: &FinalImageRelocation,
    ) -> Result<ElfDirectImportCallSite, Diagnostic> {
        require(
            relocation.section == FinalImageSection::Text
                && relocation.kind == expected_kind
                && relocation.byte_width == 4
                && relocation.addend == 0,
            "ELF dynamic import requires a non-procedure or malformed source relocation",
        )?;
        let end = relocation
            .offset
            .checked_add(relocation.byte_width)
            .filter(|end| *end <= text.len())
            .ok_or_else(|| Diagnostic::error("ELF dynamic import call relocation exceeds .text"))?;
        let instruction_offset = match target {
            TargetProfile::LinuxX64 => {
                let instruction_offset = relocation.offset.checked_sub(1).ok_or_else(|| {
                    Diagnostic::error("x86-64 import call relocation lacks its opcode byte")
                })?;
                require(
                    text.get(instruction_offset) == Some(&0xe8)
                        && text.get(relocation.offset..end) == Some(&[0, 0, 0, 0]),
                    "x86-64 import call does not retain the exact unresolved CALL rel32 placeholder",
                )?;
                instruction_offset
            }
            TargetProfile::LinuxArm64 => {
                require(
                    relocation.offset.is_multiple_of(4)
                        && text.get(relocation.offset..end) == Some(&[0, 0, 0, 0x94]),
                    "AArch64 import call does not retain the exact unresolved BL placeholder",
                )?;
                relocation.offset
            }
            _ => {
                return Err(Diagnostic::error(
                    "ELF import call uses a non-Linux target profile",
                ));
            }
        };
        Ok(ElfDirectImportCallSite {
            instruction_offset,
            relocation_offset: relocation.offset,
            byte_width: relocation.byte_width,
            kind: relocation.kind,
            addend: relocation.addend,
        })
    }

    fn validate_site_spans(sites: &[ElfDirectImportCallSite]) -> Result<(), Diagnostic> {
        for pair in sites.windows(2) {
            let left_end = site_end(&pair[0])?;
            require(
                left_end <= pair[1].instruction_offset,
                "ELF import call sites overlap or repeat one instruction span",
            )?;
        }
        Ok(())
    }

    fn validate_nonoverlapping_call_sites(
        contents: &ElfProcedureLinkageRelocationContents,
    ) -> Result<(), Diagnostic> {
        let mut spans = contents
            .slots
            .iter()
            .flat_map(|slot| &slot.call_sites)
            .map(|site| Ok((site.instruction_offset, site_end(site)?)))
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        spans.sort_unstable();
        for pair in spans.windows(2) {
            require(
                pair[0].1 <= pair[1].0,
                "ELF import call mapping contains overlapping instruction spans",
            )?;
        }
        Ok(())
    }

    fn validate_nonoverlapping_general_slots(
        contents: &ElfProcedureLinkageRelocationContents,
    ) -> Result<(), Diagnostic> {
        for pair in contents.general_relocations.windows(2) {
            require(
                pair[0].source_offset != pair[1].source_offset,
                "ELF import data slots overlap or repeat one eight-byte span",
            )?;
        }
        Ok(())
    }

    fn site_end(site: &ElfDirectImportCallSite) -> Result<usize, Diagnostic> {
        site.relocation_offset
            .checked_add(site.byte_width)
            .ok_or_else(|| Diagnostic::error("ELF import call-site span overflows usize"))
    }

    checked::seal_stage! {
        field: descriptors,
        contents: contents,
        validated: ValidatedElfProcedureLinkageRelocationPlan,
        validate: validate_contents,
        fingerprint: non_authoritative_linkage_compatibility_fingerprint,
        message: "ELF procedure-linkage compatibility fingerprint does not replay",
    }

    fn validate_contents(
        descriptors: &ValidatedElfDynamicSectionDescriptorPlan,
        contents: &ElfProcedureLinkageRelocationContents,
    ) -> Result<(), Diagnostic> {
        let plan = descriptors.payloads().plan();
        let bindings = &plan.contents().bindings;
        let inputs = plan.inputs();
        require(
            contents.slots.len() == bindings.len()
                && contents.jump_slot_relocations.len() == bindings.len()
                && bindings.len() == inputs.imports().len(),
            "ELF procedure-linkage slots, relocations, bindings, and imports do not correspond",
        )?;
        let (source_kind, relocation_type, general_type) =
            target_relocation_spec(inputs.interpreter().target())?;
        let mut expected_general = Vec::new();
        for (index, binding) in bindings.iter().enumerate() {
            let logical_ordinal = checked_u32(index, "validated logical PLT/GOT slot ordinal")?;
            let request = inputs.imports().get(binding.request_index).ok_or_else(|| {
                Diagnostic::error("validated ELF binding exceeds canonical import requests")
            })?;
            let compatibility_report_identity = request_compatibility_report_identity(request)?;
            let (expected_sites, mut binding_general) = canonical_relocation_classes(
                inputs.interpreter().target(),
                &inputs.image().memory.text,
                inputs.image().memory.data.len(),
                source_kind,
                general_type,
                binding,
                request,
            )?;
            expected_general.append(&mut binding_general);
            let expected_slot = ElfLogicalProcedureLinkageSlot {
                logical_ordinal,
                request_index: binding.request_index,
                compatibility_report_identity,
                dynamic_symbol_index: binding.dynamic_symbol_index,
                version_index: binding.version_index,
                call_sites: expected_sites,
            };
            let expected_relocation = ElfSemanticJumpSlotRelocation {
                logical_got_slot_ordinal: logical_ordinal,
                dynamic_symbol_index: binding.dynamic_symbol_index,
                relocation_type,
                addend: 0,
            };
            require(
                compatibility_report_identity == binding.compatibility_report_identity
                    && contents.slots.get(index) == Some(&expected_slot)
                    && contents.jump_slot_relocations.get(index) == Some(&expected_relocation),
                "ELF procedure-linkage slot or JUMP_SLOT row drifted from its exact import binding",
            )?;
        }
        expected_general.sort_unstable_by_key(|relocation| relocation.source_offset);
        require(
            contents.general_relocations == expected_general,
            "ELF general RELA rows drifted from their exact import bindings",
        )?;
        require_unique_semantic_rows(contents)?;
        validate_nonoverlapping_call_sites(contents)?;
        validate_nonoverlapping_general_slots(contents)
    }

    fn require_unique_semantic_rows(
        contents: &ElfProcedureLinkageRelocationContents,
    ) -> Result<(), Diagnostic> {
        for (index, slot) in contents.slots.iter().enumerate() {
            require(
                contents
                    .slots
                    .iter()
                    .enumerate()
                    .all(|(other_index, other)| {
                        other_index == index
                            || (other.logical_ordinal != slot.logical_ordinal
                                && other.compatibility_report_identity
                                    != slot.compatibility_report_identity
                                && other.dynamic_symbol_index != slot.dynamic_symbol_index)
                    }),
                "ELF procedure-linkage slots duplicate an ordinal, import identity, or dynamic symbol",
            )?;
        }
        Ok(())
    }

    fn non_authoritative_linkage_compatibility_fingerprint(
        descriptors: &ValidatedElfDynamicSectionDescriptorPlan,
        contents: &ElfProcedureLinkageRelocationContents,
    ) -> u64 {
        let mut hash = checked::stage_fingerprint(
            b"omega.elf-procedure-linkage-relocations.v1",
            descriptors.non_authoritative_descriptor_compatibility_fingerprint(),
        );
        hash.bytes(
            descriptors
                .payloads()
                .plan()
                .inputs()
                .interpreter()
                .target()
                .target_name()
                .as_bytes(),
        );
        hash.bytes(&(contents.slots.len() as u64).to_le_bytes());
        for slot in &contents.slots {
            hash.bytes(&slot.logical_ordinal.to_le_bytes());
            hash.bytes(&slot.compatibility_report_identity.to_le_bytes());
            hash.bytes(&slot.dynamic_symbol_index.to_le_bytes());
            hash.bytes(&slot.version_index.to_le_bytes());
            hash.bytes(&(slot.call_sites.len() as u64).to_le_bytes());
            for site in &slot.call_sites {
                hash.bytes(&(site.instruction_offset as u64).to_le_bytes());
                hash.bytes(&(site.relocation_offset as u64).to_le_bytes());
                hash.bytes(&(site.byte_width as u64).to_le_bytes());
                hash.byte(relocation_kind_tag(site.kind));
                hash.bytes(&site.addend.to_le_bytes());
            }
        }
        hash.bytes(&(contents.jump_slot_relocations.len() as u64).to_le_bytes());
        for relocation in &contents.jump_slot_relocations {
            hash.bytes(&relocation.logical_got_slot_ordinal.to_le_bytes());
            hash.bytes(&relocation.dynamic_symbol_index.to_le_bytes());
            hash.bytes(&relocation.relocation_type.to_le_bytes());
            hash.bytes(&relocation.addend.to_le_bytes());
        }
        hash.bytes(&(contents.general_relocations.len() as u64).to_le_bytes());
        for relocation in &contents.general_relocations {
            hash.bytes(&(relocation.request_index as u64).to_le_bytes());
            hash.bytes(&relocation.dynamic_symbol_index.to_le_bytes());
            hash.bytes(&relocation.relocation_type.to_le_bytes());
            hash.byte(section_tag(relocation.source_section));
            hash.bytes(&(relocation.source_offset as u64).to_le_bytes());
            hash.bytes(&relocation.addend.to_le_bytes());
        }
        hash.finish()
    }

    const fn section_tag(section: FinalImageSection) -> u8 {
        match section {
            FinalImageSection::Text => 1,
            FinalImageSection::Data => 2,
            FinalImageSection::Bss => 3,
            FinalImageSection::None => 4,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            Candidate, ElfDirectImportCallSite, FinalImageSection, R_AARCH64_ABS64,
            R_AARCH64_JUMP_SLOT, R_X86_64_64, R_X86_64_JUMP_SLOT, RelocationKind, TargetProfile,
            ValidatedElfDynamicSectionDescriptorPlan, call_site, checked_u32, derive_contents,
            non_authoritative_linkage_compatibility_fingerprint,
            plan_elf_procedure_linkage_relocations, site_end, validate_candidate,
            validate_contents, validate_site_spans,
        };
        use crate::{
            plan_elf_dynamic_link_inputs, plan_elf_dynamic_section_descriptors,
            plan_elf_dynamic_sections, serialize_elf_dynamic_sections,
        };
        use arena::Handle;
        use image::{FinalImage, FinalImageMemory, FinalImageRelocation};
        use target::normalize_elf_interpreter_plan;

        use crate::test_support::{IMPORTS, ImportFixture, import_fixture_interpreter_path};

        fn image(
            target: TargetProfile,
            imports: &[ImportFixture],
        ) -> (FinalImage, Vec<Handle<FinalImageRelocation>>) {
            let native_target = target.native_target();
            let relocation_count = imports
                .iter()
                .map(|fixture| fixture.instruction_offsets.len())
                .sum();
            let mut image = FinalImage::with_capacity(
                native_target,
                FinalImageMemory {
                    text: vec![0; 64],
                    ..FinalImageMemory::default()
                },
                Handle::invalid(),
                imports.len(),
                imports.len(),
                relocation_count,
            );
            let mut relocation_handles = Vec::with_capacity(relocation_count);
            for (index, fixture) in imports.iter().enumerate() {
                let symbol_handle = crate::test_support::insert_import_fixture(
                    &mut image,
                    "__omega_procedure_import_",
                    index,
                    fixture.object,
                    fixture.symbol,
                    fixture.version,
                    target,
                    "valid procedure-linkage locator",
                );
                for instruction_offset in fixture.instruction_offsets {
                    let (relocation_offset, kind) = match target {
                        TargetProfile::LinuxX64 => {
                            image.memory.text[*instruction_offset] = 0xe8;
                            (instruction_offset + 1, RelocationKind::X86_64Relative32)
                        }
                        TargetProfile::LinuxArm64 => {
                            image.memory.text[*instruction_offset..*instruction_offset + 4]
                                .copy_from_slice(&[0, 0, 0, 0x94]);
                            (*instruction_offset, RelocationKind::Aarch64Branch26)
                        }
                        _ => unreachable!("fixture uses a Linux target"),
                    };
                    relocation_handles.push(image.relocation_table.relocations.insert(
                        FinalImageRelocation {
                            section: FinalImageSection::Text,
                            offset: relocation_offset,
                            byte_width: 4,
                            symbol_handle,
                            addend: 0,
                            kind,
                        },
                    ));
                }
            }
            (image, relocation_handles)
        }

        fn image_with_general_relocations(
            target: TargetProfile,
            imports: &[ImportFixture],
            general_uses: &[(usize, usize)],
        ) -> (FinalImage, Vec<Handle<FinalImageRelocation>>) {
            let (mut image, _) = image(target, imports);
            image.memory.data = vec![0; 48];
            let symbol_handles = image
                .symbol_table
                .imports
                .iter()
                .map(|(_, import)| import.symbol_handle)
                .collect::<Vec<_>>();
            let mut handles = Vec::with_capacity(general_uses.len());
            for &(import_index, offset) in general_uses {
                handles.push(
                    image
                        .relocation_table
                        .relocations
                        .insert(FinalImageRelocation {
                            section: FinalImageSection::Data,
                            offset,
                            byte_width: 8,
                            symbol_handle: symbol_handles[import_index],
                            addend: 0,
                            kind: RelocationKind::Absolute64,
                        }),
                );
            }
            (image, handles)
        }

        fn descriptors_from_image(
            target: TargetProfile,
            image: FinalImage,
        ) -> ValidatedElfDynamicSectionDescriptorPlan {
            let interpreter = normalize_elf_interpreter_plan(
                import_fixture_interpreter_path(target).to_vec(),
                target,
            )
            .expect("valid procedure-linkage interpreter");
            let inputs = plan_elf_dynamic_link_inputs(image, interpreter)
                .expect("valid dynamic-link preflight");
            let sections = plan_elf_dynamic_sections(inputs).expect("valid dynamic sections");
            let payloads =
                serialize_elf_dynamic_sections(sections).expect("valid dynamic payloads");
            plan_elf_dynamic_section_descriptors(payloads).expect("valid dynamic descriptors")
        }

        fn descriptors(
            target: TargetProfile,
            imports: &[ImportFixture],
        ) -> ValidatedElfDynamicSectionDescriptorPlan {
            descriptors_from_image(target, image(target, imports).0)
        }

        fn candidate(target: TargetProfile) -> Candidate {
            let descriptors = descriptors_from_image(
                target,
                image_with_general_relocations(target, &IMPORTS, &[(0, 8)]).0,
            );
            let contents = derive_contents(&descriptors).expect("derived procedure linkage");
            let non_authoritative_linkage_compatibility_fingerprint =
                non_authoritative_linkage_compatibility_fingerprint(&descriptors, &contents);
            Candidate {
                descriptors,
                contents,
                non_authoritative_linkage_compatibility_fingerprint,
            }
        }

        #[test]
        fn both_linux_targets_plan_exact_slots_jump_relocations_and_call_sites() {
            for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                let plan = plan_elf_procedure_linkage_relocations(descriptors(target, &IMPORTS))
                    .expect("validated procedure-linkage plan");
                assert_eq!(plan.logical_slot_count(), 3);
                assert_eq!(plan.procedure_relocation_count(), 3);
                assert_eq!(plan.direct_call_site_count(), 4);
                assert_eq!(plan.general_dynamic_relocation_count(), 0);
                assert_ne!(
                    plan.non_authoritative_linkage_compatibility_fingerprint(),
                    0
                );

                let expected_source_kind = match target {
                    TargetProfile::LinuxX64 => RelocationKind::X86_64Relative32,
                    TargetProfile::LinuxArm64 => RelocationKind::Aarch64Branch26,
                    _ => unreachable!(),
                };
                let expected_dynamic_type = match target {
                    TargetProfile::LinuxX64 => R_X86_64_JUMP_SLOT,
                    TargetProfile::LinuxArm64 => R_AARCH64_JUMP_SLOT,
                    _ => unreachable!(),
                };
                assert_eq!(
                    plan.contents
                        .slots
                        .iter()
                        .map(|slot| slot.logical_ordinal)
                        .collect::<Vec<_>>(),
                    [0, 1, 2],
                );
                assert_eq!(
                    plan.contents
                        .slots
                        .iter()
                        .map(|slot| slot.dynamic_symbol_index)
                        .collect::<Vec<_>>(),
                    [1, 2, 3],
                );
                assert!(plan.contents.slots.iter().all(|slot| {
                    !slot.call_sites.is_empty()
                        && slot.call_sites.iter().all(|site| {
                            site.kind == expected_source_kind
                                && site.byte_width == 4
                                && site.addend == 0
                        })
                }));
                assert!(plan.contents.jump_slot_relocations.iter().enumerate().all(
                    |(index, relocation)| {
                        relocation.logical_got_slot_ordinal == index as u32
                            && relocation.dynamic_symbol_index == index as u32 + 1
                            && relocation.relocation_type == expected_dynamic_type
                            && relocation.addend == 0
                    }
                ));
                validate_contents(plan.descriptors(), &plan.contents)
                    .expect("independent procedure-linkage replay");
            }
        }

        #[test]
        fn import_permutation_preserves_identity_and_multiple_calls_share_one_slot() {
            let forward = plan_elf_procedure_linkage_relocations(descriptors(
                TargetProfile::LinuxX64,
                &IMPORTS,
            ))
            .expect("forward procedure linkage");
            let reversed = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
            let reverse = plan_elf_procedure_linkage_relocations(descriptors(
                TargetProfile::LinuxX64,
                &reversed,
            ))
            .expect("reverse procedure linkage");
            let arm = plan_elf_procedure_linkage_relocations(descriptors(
                TargetProfile::LinuxArm64,
                &IMPORTS,
            ))
            .expect("AArch64 procedure linkage");

            assert_eq!(
                forward.non_authoritative_linkage_compatibility_fingerprint(),
                reverse.non_authoritative_linkage_compatibility_fingerprint()
            );
            assert_ne!(
                forward.non_authoritative_linkage_compatibility_fingerprint(),
                arm.non_authoritative_linkage_compatibility_fingerprint()
            );
            assert_eq!(forward.logical_slot_count(), 3);
            assert_eq!(forward.direct_call_site_count(), 4);
            assert_eq!(
                forward
                    .contents
                    .slots
                    .iter()
                    .map(|slot| slot.call_sites.len())
                    .collect::<Vec<_>>(),
                [2, 1, 1],
            );
        }

        #[test]
        fn nonprocedure_source_relocations_and_tampered_placeholders_reject_with_custody() {
            type Mutation = Box<dyn Fn(&mut FinalImage, &[Handle<FinalImageRelocation>])>;
            let mutations: Vec<Mutation> = vec![
                Box::new(|image, handles| {
                    image
                        .relocation_table
                        .relocations
                        .get_mut(handles[0])
                        .section = FinalImageSection::Data;
                }),
                Box::new(|image, handles| {
                    image.relocation_table.relocations.get_mut(handles[0]).kind =
                        RelocationKind::Absolute64;
                }),
                Box::new(|image, handles| {
                    image
                        .relocation_table
                        .relocations
                        .get_mut(handles[0])
                        .byte_width = 8;
                }),
                Box::new(|image, handles| {
                    image
                        .relocation_table
                        .relocations
                        .get_mut(handles[0])
                        .addend = 1;
                }),
                Box::new(|image, handles| {
                    image
                        .relocation_table
                        .relocations
                        .get_mut(handles[0])
                        .offset = usize::MAX;
                }),
                Box::new(|image, _| image.memory.text[0] = 0x90),
                Box::new(|image, handles| {
                    let first = image.relocation_table.relocations.get(handles[0]).offset;
                    image
                        .relocation_table
                        .relocations
                        .get_mut(handles[2])
                        .offset = first;
                }),
            ];

            for mutate in mutations {
                let (mut image, handles) = image(TargetProfile::LinuxX64, &IMPORTS);
                mutate(&mut image, &handles);
                let descriptors = descriptors_from_image(TargetProfile::LinuxX64, image);
                let expected_identity =
                    descriptors.non_authoritative_descriptor_compatibility_fingerprint();
                let error = plan_elf_procedure_linkage_relocations(descriptors)
                    .expect_err("invalid imported call must reject before linkage sealing");
                let (returned, _) = error.into_parts();
                assert_eq!(
                    returned.non_authoritative_descriptor_compatibility_fingerprint(),
                    expected_identity
                );
            }

            let (mut arm_image, arm_handles) = image(TargetProfile::LinuxArm64, &IMPORTS);
            arm_image
                .relocation_table
                .relocations
                .get_mut(arm_handles[0])
                .offset = 1;
            let descriptors = descriptors_from_image(TargetProfile::LinuxArm64, arm_image);
            let expected_identity =
                descriptors.non_authoritative_descriptor_compatibility_fingerprint();
            let error = plan_elf_procedure_linkage_relocations(descriptors)
                .expect_err("misaligned AArch64 BL relocation must reject");
            assert_eq!(
                error
                    .into_parts()
                    .0
                    .non_authoritative_descriptor_compatibility_fingerprint(),
                expected_identity
            );
        }

        #[test]
        fn independent_validation_rejects_every_slot_relocation_site_and_identity_corruption() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|candidate| {
                    candidate.contents.slots.pop();
                }),
                Box::new(|candidate| {
                    candidate.contents.jump_slot_relocations.pop();
                }),
                Box::new(|candidate| candidate.contents.slots.swap(0, 1)),
                Box::new(|candidate| candidate.contents.slots[0].logical_ordinal += 1),
                Box::new(|candidate| candidate.contents.slots[0].request_index = usize::MAX),
                Box::new(|candidate| {
                    candidate.contents.slots[0].compatibility_report_identity ^= 1;
                }),
                Box::new(|candidate| candidate.contents.slots[0].dynamic_symbol_index += 1),
                Box::new(|candidate| candidate.contents.slots[0].version_index += 1),
                Box::new(|candidate| {
                    candidate.contents.slots[0].call_sites.pop();
                }),
                Box::new(|candidate| {
                    candidate.contents.slots[0].call_sites[0].instruction_offset += 1
                }),
                Box::new(|candidate| {
                    candidate.contents.slots[0].call_sites[0].relocation_offset += 1
                }),
                Box::new(|candidate| candidate.contents.slots[0].call_sites[0].byte_width += 1),
                Box::new(|candidate| {
                    candidate.contents.slots[0].call_sites[0].kind = RelocationKind::Absolute64
                }),
                Box::new(|candidate| candidate.contents.slots[0].call_sites[0].addend += 1),
                Box::new(|candidate| {
                    candidate.contents.jump_slot_relocations[0].logical_got_slot_ordinal += 1
                }),
                Box::new(|candidate| {
                    candidate.contents.jump_slot_relocations[0].dynamic_symbol_index += 1
                }),
                Box::new(|candidate| {
                    candidate.contents.jump_slot_relocations[0].relocation_type += 1
                }),
                Box::new(|candidate| candidate.contents.jump_slot_relocations[0].addend += 1),
                Box::new(|candidate| {
                    candidate.contents.general_relocations.pop();
                }),
                Box::new(|candidate| candidate.contents.general_relocations[0].request_index += 1),
                Box::new(|candidate| candidate.contents.general_relocations[0].source_offset += 8),
                Box::new(|candidate| {
                    candidate.contents.general_relocations[0].dynamic_symbol_index += 1
                }),
                Box::new(|candidate| {
                    candidate.contents.general_relocations[0].relocation_type += 1
                }),
                Box::new(|candidate| candidate.contents.general_relocations[0].addend += 1),
                Box::new(|candidate| {
                    candidate.contents.general_relocations[0].source_section =
                        FinalImageSection::Text
                }),
                Box::new(|candidate| {
                    candidate.non_authoritative_linkage_compatibility_fingerprint ^= 1
                }),
            ];

            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxX64);
                let expected_identity = candidate
                    .descriptors
                    .non_authoritative_descriptor_compatibility_fingerprint();
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("corrupt procedure-linkage candidate must reject");
                assert_eq!(
                    error
                        .candidate
                        .descriptors
                        .non_authoritative_descriptor_compatibility_fingerprint(),
                    expected_identity,
                    "validation failure retains exact descriptor custody",
                );
            }
        }

        #[test]
        fn malformed_offsets_ordinals_and_spans_reject_without_panicking() {
            assert!(checked_u32(usize::MAX, "ordinal").is_err());
            let overflow_site = ElfDirectImportCallSite {
                instruction_offset: usize::MAX,
                relocation_offset: usize::MAX,
                byte_width: 4,
                kind: RelocationKind::X86_64Relative32,
                addend: 0,
            };
            assert!(site_end(&overflow_site).is_err());
            assert!(validate_site_spans(&[overflow_site, overflow_site]).is_err());

            let relocation = FinalImageRelocation {
                section: FinalImageSection::Text,
                offset: usize::MAX,
                byte_width: 4,
                symbol_handle: Handle::invalid(),
                addend: 0,
                kind: RelocationKind::X86_64Relative32,
            };
            assert!(
                call_site(
                    TargetProfile::LinuxX64,
                    &[0xe8, 0, 0, 0, 0],
                    RelocationKind::X86_64Relative32,
                    &relocation,
                )
                .is_err()
            );
        }

        #[test]
        fn absolute_data_references_admit_general_relocation_rows() {
            for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                let (image, _) =
                    image_with_general_relocations(target, &IMPORTS, &[(0, 8), (2, 24)]);
                let plan =
                    plan_elf_procedure_linkage_relocations(descriptors_from_image(target, image))
                        .expect("validated procedure linkage with general rows");
                assert_eq!(plan.logical_slot_count(), 3);
                assert_eq!(plan.procedure_relocation_count(), 3);
                assert_eq!(plan.direct_call_site_count(), 4);
                assert_eq!(plan.general_dynamic_relocation_count(), 2);
                let expected_type = match target {
                    TargetProfile::LinuxX64 => R_X86_64_64,
                    TargetProfile::LinuxArm64 => R_AARCH64_ABS64,
                    _ => unreachable!(),
                };
                let rows = &plan.contents.general_relocations;
                assert_eq!(
                    rows.iter().map(|row| row.source_offset).collect::<Vec<_>>(),
                    [8, 24]
                );
                assert_eq!(
                    rows.iter().map(|row| row.request_index).collect::<Vec<_>>(),
                    [0, 2]
                );
                assert_eq!(
                    rows.iter()
                        .map(|row| row.dynamic_symbol_index)
                        .collect::<Vec<_>>(),
                    [1, 3]
                );
                assert!(rows.iter().all(|row| {
                    row.source_section == FinalImageSection::Data
                        && row.relocation_type == expected_type
                        && row.addend == 0
                }));
                validate_contents(plan.descriptors(), &plan.contents)
                    .expect("independent replay admits general rows");
            }
        }

        #[test]
        fn malformed_general_relocation_shapes_reject_with_custody() {
            type Mutation = Box<dyn Fn(&mut FinalImage, Handle<FinalImageRelocation>)>;
            let mutations: Vec<Mutation> = vec![
                Box::new(|image, handle| {
                    image.relocation_table.relocations.get_mut(handle).section =
                        FinalImageSection::Bss;
                }),
                Box::new(|image, handle| {
                    image
                        .relocation_table
                        .relocations
                        .get_mut(handle)
                        .byte_width = 4;
                }),
                Box::new(|image, handle| {
                    image.relocation_table.relocations.get_mut(handle).offset = 48;
                }),
                Box::new(|image, handle| {
                    image.relocation_table.relocations.get_mut(handle).offset = 4;
                }),
                Box::new(|image, handle| {
                    image.relocation_table.relocations.get_mut(handle).kind =
                        RelocationKind::X86_64Relative32;
                }),
            ];
            for mutate in mutations {
                let (mut image, handles) =
                    image_with_general_relocations(TargetProfile::LinuxX64, &IMPORTS, &[(0, 8)]);
                mutate(&mut image, handles[0]);
                let descriptors = descriptors_from_image(TargetProfile::LinuxX64, image);
                let expected_identity =
                    descriptors.non_authoritative_descriptor_compatibility_fingerprint();
                let error = plan_elf_procedure_linkage_relocations(descriptors)
                    .expect_err("malformed general relocation must reject with custody");
                assert_eq!(
                    error
                        .into_parts()
                        .0
                        .non_authoritative_descriptor_compatibility_fingerprint(),
                    expected_identity
                );
            }

            let (overlapping, _) = image_with_general_relocations(
                TargetProfile::LinuxX64,
                &IMPORTS,
                &[(0, 8), (1, 12)],
            );
            assert!(
                plan_elf_procedure_linkage_relocations(descriptors_from_image(
                    TargetProfile::LinuxX64,
                    overlapping
                ))
                .is_err()
            );
        }
    }
}

pub(crate) mod dynamic_linkage_descriptors {
    use super::super::checked::{self, CandidateValidationError};
    use crate::bytes::require_name;
    use crate::dynamic_executable::checked::{checked_sum, checked_u32, require};
    use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::ValidatedElfProcedureLinkageTemplatePlan;
    use diagnostics::Diagnostic;
    use target::TargetProfile;

    use crate::constants::{SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE};
    const ELF64_RELA_SIZE: u64 = crate::constants::ELF64_RELA_SIZE as u64;
    const SHT_PROGBITS: u32 = 1;
    const SHT_RELA: u32 = 4;
    const SHF_INFO_LINK: u64 = 0x40;
    const SHF_AARCH64_PURECODE: u64 = 0x2000_0000;
    const PROCEDURE_LINKAGE_ALIGNMENT: u64 = 16;
    const PROCEDURE_GOT_ALIGNMENT: u64 = 8;
    const PROCEDURE_RELOCATION_ALIGNMENT: u64 = 8;
    const UPSTREAM_DESCRIPTOR_COUNT: usize = 7;
    const APPENDED_DESCRIPTOR_COUNT: usize = 4;
    const PROCEDURE_LINKAGE_NAME_SUFFIX: &[u8] = b".plt\0.got.plt\0.rela.plt\0.rela.dyn\0";
    #[derive(Debug)]
    #[must_use = "validated linkage descriptors retain the exact template carrier"]
    pub struct ValidatedElfProcedureLinkageSectionDescriptorPlan {
        templates: ValidatedElfProcedureLinkageTemplatePlan,
        contents: ElfProcedureLinkageSectionDescriptorContents,
        non_authoritative_descriptor_compatibility_fingerprint: u64,
    }

    impl ValidatedElfProcedureLinkageSectionDescriptorPlan {
        pub const fn templates(&self) -> &ValidatedElfProcedureLinkageTemplatePlan {
            &self.templates
        }

        pub fn descriptor_count(&self) -> usize {
            UPSTREAM_DESCRIPTOR_COUNT + self.contents.descriptors.len()
        }

        #[cfg(test)]
        pub(crate) fn appended_descriptor_count(&self) -> usize {
            self.contents.descriptors.len()
        }

        pub fn section_name_seed_byte_count(&self) -> usize {
            self.contents.section_name_table_seed.len()
        }

        pub const fn non_authoritative_descriptor_compatibility_fingerprint(&self) -> u64 {
            self.non_authoritative_descriptor_compatibility_fingerprint
        }

        pub(crate) const fn contents(&self) -> &ElfProcedureLinkageSectionDescriptorContents {
            &self.contents
        }

        pub(crate) fn into_parts(
            self,
        ) -> (
            ValidatedElfProcedureLinkageTemplatePlan,
            ElfProcedureLinkageSectionDescriptorContents,
        ) {
            (self.templates, self.contents)
        }
    }

    #[derive(Debug)]
    #[must_use = "ELF linkage-descriptor rejection retains the validated templates"]
    pub struct ElfProcedureLinkageSectionDescriptorPlanningError {
        templates: ValidatedElfProcedureLinkageTemplatePlan,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(
        ElfProcedureLinkageSectionDescriptorPlanningError;
        templates: ValidatedElfProcedureLinkageTemplatePlan,
    );

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfProcedureLinkageSectionDescriptorContents {
        pub(crate) section_name_table_seed: Vec<u8>,
        pub(crate) descriptors: Vec<ElfProcedureLinkageSectionDescriptor>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    #[allow(
        clippy::enum_variant_names,
        reason = "the repeated Procedure prefix preserves exact ELF section-domain terminology"
    )]
    pub(crate) enum ElfProcedureLinkageSectionKind {
        ProcedureLinkage = 1,
        ProcedureGot = 2,
        ProcedureRelocation = 3,
        GeneralRelocation = 4,
    }

    impl ElfProcedureLinkageSectionKind {
        const fn name(self) -> &'static [u8] {
            match self {
                Self::ProcedureLinkage => b".plt",
                Self::ProcedureGot => b".got.plt",
                Self::ProcedureRelocation => b".rela.plt",
                Self::GeneralRelocation => b".rela.dyn",
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    pub(crate) enum ElfProcedureLinkageSectionLink {
        None = 0,
        DynamicSymbol = 1,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ElfProcedureLinkageSectionInfo {
        None,
        RelocatedSection(ElfProcedureLinkageSectionKind),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfProcedureLinkageSectionDescriptor {
        pub(crate) kind: ElfProcedureLinkageSectionKind,
        pub(crate) name_offset: u32,
        pub(crate) section_type: u32,
        pub(crate) flags: u64,
        pub(crate) payload_size: u64,
        pub(crate) alignment: u64,
        pub(crate) entry_size: u64,
        pub(crate) link: ElfProcedureLinkageSectionLink,
        pub(crate) info: ElfProcedureLinkageSectionInfo,
    }

    struct Candidate {
        templates: ValidatedElfProcedureLinkageTemplatePlan,
        contents: ElfProcedureLinkageSectionDescriptorContents,
        non_authoritative_descriptor_compatibility_fingerprint: u64,
    }

    pub(crate) fn plan_elf_procedure_linkage_section_descriptors(
        templates: ValidatedElfProcedureLinkageTemplatePlan,
    ) -> Result<
        ValidatedElfProcedureLinkageSectionDescriptorPlan,
        Box<ElfProcedureLinkageSectionDescriptorPlanningError>,
    > {
        checked::plan_stage!(
            plan: templates,
            derive: derive_contents,
            contents: contents,
            fingerprint: non_authoritative_descriptor_compatibility_fingerprint,
            error: ElfProcedureLinkageSectionDescriptorPlanningError,
        )
    }

    fn derive_contents(
        templates: &ValidatedElfProcedureLinkageTemplatePlan,
    ) -> Result<ElfProcedureLinkageSectionDescriptorContents, Diagnostic> {
        let base_seed = templates.linkage().descriptors().section_name_table_seed();
        let plt_name_offset = checked_u32(base_seed.len(), "procedure-linkage name offset")?;
        let got_name_offset = checked_u32(
            checked_sum(
                base_seed.len(),
                ElfProcedureLinkageSectionKind::ProcedureLinkage
                    .name()
                    .len()
                    + 1,
                "procedure GOT name offset",
            )?,
            "procedure GOT name offset",
        )?;
        let rela_name_offset = checked_u32(
            checked_sum(
                usize::try_from(got_name_offset)
                    .map_err(|_| Diagnostic::error("procedure GOT name offset exceeds usize"))?,
                ElfProcedureLinkageSectionKind::ProcedureGot.name().len() + 1,
                "procedure relocation name offset",
            )?,
            "procedure relocation name offset",
        )?;
        let rela_dyn_name_offset = checked_u32(
            checked_sum(
                usize::try_from(rela_name_offset).map_err(|_| {
                    Diagnostic::error("procedure relocation name offset exceeds usize")
                })?,
                ElfProcedureLinkageSectionKind::ProcedureRelocation
                    .name()
                    .len()
                    + 1,
                "general relocation name offset",
            )?,
            "general relocation name offset",
        )?;
        let mut section_name_table_seed = Vec::with_capacity(checked_sum(
            base_seed.len(),
            PROCEDURE_LINKAGE_NAME_SUFFIX.len(),
            "extended ELF section-name seed size",
        )?);
        section_name_table_seed.extend_from_slice(base_seed);
        section_name_table_seed.extend_from_slice(PROCEDURE_LINKAGE_NAME_SUFFIX);

        let target = target(templates);
        let plt_flags = match target {
            TargetProfile::LinuxX64 => SHF_ALLOC | SHF_EXECINSTR,
            TargetProfile::LinuxArm64 => SHF_ALLOC | SHF_EXECINSTR | SHF_AARCH64_PURECODE,
            _ => {
                return Err(Diagnostic::error(
                    "ELF linkage descriptors require an exact Linux x86-64 or AArch64 profile",
                ));
            }
        };
        let bytes = &templates.contents().bytes;
        let descriptors = vec![
            descriptor(
                ElfProcedureLinkageSectionKind::ProcedureLinkage,
                plt_name_offset,
                SHT_PROGBITS,
                plt_flags,
                bytes.plt.len(),
                PROCEDURE_LINKAGE_ALIGNMENT,
                0,
                ElfProcedureLinkageSectionLink::None,
                ElfProcedureLinkageSectionInfo::None,
            )?,
            descriptor(
                ElfProcedureLinkageSectionKind::ProcedureGot,
                got_name_offset,
                SHT_PROGBITS,
                SHF_ALLOC | SHF_WRITE,
                bytes.got_plt.len(),
                PROCEDURE_GOT_ALIGNMENT,
                0,
                ElfProcedureLinkageSectionLink::None,
                ElfProcedureLinkageSectionInfo::None,
            )?,
            descriptor(
                ElfProcedureLinkageSectionKind::ProcedureRelocation,
                rela_name_offset,
                SHT_RELA,
                SHF_ALLOC | SHF_INFO_LINK,
                bytes.rela_plt.len(),
                PROCEDURE_RELOCATION_ALIGNMENT,
                ELF64_RELA_SIZE,
                ElfProcedureLinkageSectionLink::DynamicSymbol,
                ElfProcedureLinkageSectionInfo::RelocatedSection(
                    ElfProcedureLinkageSectionKind::ProcedureGot,
                ),
            )?,
            descriptor(
                ElfProcedureLinkageSectionKind::GeneralRelocation,
                rela_dyn_name_offset,
                SHT_RELA,
                SHF_ALLOC | SHF_INFO_LINK,
                bytes.rela_dyn.len(),
                PROCEDURE_RELOCATION_ALIGNMENT,
                ELF64_RELA_SIZE,
                ElfProcedureLinkageSectionLink::DynamicSymbol,
                ElfProcedureLinkageSectionInfo::None,
            )?,
        ];
        Ok(ElfProcedureLinkageSectionDescriptorContents {
            section_name_table_seed,
            descriptors,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn descriptor(
        kind: ElfProcedureLinkageSectionKind,
        name_offset: u32,
        section_type: u32,
        flags: u64,
        payload_size: usize,
        alignment: u64,
        entry_size: u64,
        link: ElfProcedureLinkageSectionLink,
        info: ElfProcedureLinkageSectionInfo,
    ) -> Result<ElfProcedureLinkageSectionDescriptor, Diagnostic> {
        Ok(ElfProcedureLinkageSectionDescriptor {
            kind,
            name_offset,
            section_type,
            flags,
            payload_size: u64::try_from(payload_size)
                .map_err(|_| Diagnostic::error("ELF linkage payload size exceeds Elf64_Xword"))?,
            alignment,
            entry_size,
            link,
            info,
        })
    }

    fn target(templates: &ValidatedElfProcedureLinkageTemplatePlan) -> TargetProfile {
        templates
            .linkage()
            .descriptors()
            .payloads()
            .plan()
            .inputs()
            .interpreter()
            .target()
    }

    checked::seal_stage! {
        field: templates,
        contents: contents,
        validated: ValidatedElfProcedureLinkageSectionDescriptorPlan,
        validate: validate_contents,
        fingerprint: non_authoritative_descriptor_compatibility_fingerprint,
        message: "ELF linkage-descriptor compatibility fingerprint does not replay",
    }

    fn validate_contents(
        templates: &ValidatedElfProcedureLinkageTemplatePlan,
        contents: &ElfProcedureLinkageSectionDescriptorContents,
    ) -> Result<(), Diagnostic> {
        let base_descriptors = templates.linkage().descriptors();
        require(
            base_descriptors.descriptor_count() == UPSTREAM_DESCRIPTOR_COUNT,
            "ELF linkage descriptors require the exact sealed seven-row base",
        )?;
        let base_seed = base_descriptors.section_name_table_seed();
        let expected_seed_len = checked_sum(
            base_seed.len(),
            PROCEDURE_LINKAGE_NAME_SUFFIX.len(),
            "validated linkage name-seed size",
        )?;
        require(
            contents.section_name_table_seed.len() == expected_seed_len
                && contents.section_name_table_seed.get(..base_seed.len()) == Some(base_seed)
                && contents.section_name_table_seed.get(base_seed.len()..)
                    == Some(PROCEDURE_LINKAGE_NAME_SUFFIX),
            "ELF linkage section-name seed is not an exact append-only extension",
        )?;
        require(
            contents.descriptors.len() == APPENDED_DESCRIPTOR_COUNT,
            "ELF linkage descriptor row count is not exact",
        )?;

        let bytes = &templates.contents().bytes;
        let target = target(templates);
        for (index, row) in contents.descriptors.iter().enumerate() {
            let expected_kind = match index {
                0 => ElfProcedureLinkageSectionKind::ProcedureLinkage,
                1 => ElfProcedureLinkageSectionKind::ProcedureGot,
                2 => ElfProcedureLinkageSectionKind::ProcedureRelocation,
                3 => ElfProcedureLinkageSectionKind::GeneralRelocation,
                _ => unreachable!("descriptor count checked above"),
            };
            require(
                row.kind == expected_kind,
                "ELF linkage descriptors are missing, duplicated, or reordered",
            )?;
            validate_name(&contents.section_name_table_seed, row)?;
            let (section_type, flags, payload_size, alignment, entry_size, link, info) = match row
                .kind
            {
                ElfProcedureLinkageSectionKind::ProcedureLinkage => (
                    SHT_PROGBITS,
                    match target {
                        TargetProfile::LinuxX64 => SHF_ALLOC | SHF_EXECINSTR,
                        TargetProfile::LinuxArm64 => {
                            SHF_ALLOC | SHF_EXECINSTR | SHF_AARCH64_PURECODE
                        }
                        _ => {
                            return Err(Diagnostic::error("unsupported linkage descriptor target"));
                        }
                    },
                    bytes.plt.len(),
                    PROCEDURE_LINKAGE_ALIGNMENT,
                    0,
                    ElfProcedureLinkageSectionLink::None,
                    ElfProcedureLinkageSectionInfo::None,
                ),
                ElfProcedureLinkageSectionKind::ProcedureGot => (
                    SHT_PROGBITS,
                    SHF_ALLOC | SHF_WRITE,
                    bytes.got_plt.len(),
                    PROCEDURE_GOT_ALIGNMENT,
                    0,
                    ElfProcedureLinkageSectionLink::None,
                    ElfProcedureLinkageSectionInfo::None,
                ),
                ElfProcedureLinkageSectionKind::ProcedureRelocation => (
                    SHT_RELA,
                    SHF_ALLOC | SHF_INFO_LINK,
                    bytes.rela_plt.len(),
                    PROCEDURE_RELOCATION_ALIGNMENT,
                    ELF64_RELA_SIZE,
                    ElfProcedureLinkageSectionLink::DynamicSymbol,
                    ElfProcedureLinkageSectionInfo::RelocatedSection(
                        ElfProcedureLinkageSectionKind::ProcedureGot,
                    ),
                ),
                ElfProcedureLinkageSectionKind::GeneralRelocation => (
                    SHT_RELA,
                    SHF_ALLOC | SHF_INFO_LINK,
                    bytes.rela_dyn.len(),
                    PROCEDURE_RELOCATION_ALIGNMENT,
                    ELF64_RELA_SIZE,
                    ElfProcedureLinkageSectionLink::DynamicSymbol,
                    ElfProcedureLinkageSectionInfo::None,
                ),
            };
            require(
                row.section_type == section_type
                    && row.flags == flags
                    && row.payload_size
                        == u64::try_from(payload_size).map_err(|_| {
                            Diagnostic::error("validated linkage payload size exceeds Elf64_Xword")
                        })?
                    && row.alignment == alignment
                    && row.entry_size == entry_size
                    && row.link == link
                    && row.info == info,
                "ELF linkage descriptor metadata drifted from its target template",
            )?;
        }
        Ok(())
    }

    fn validate_name(
        seed: &[u8],
        row: &ElfProcedureLinkageSectionDescriptor,
    ) -> Result<(), Diagnostic> {
        require_name(
            seed,
            row.name_offset,
            row.kind.name(),
            "ELF linkage sh_name",
        )
    }

    fn non_authoritative_descriptor_compatibility_fingerprint(
        templates: &ValidatedElfProcedureLinkageTemplatePlan,
        contents: &ElfProcedureLinkageSectionDescriptorContents,
    ) -> u64 {
        let mut hash = checked::stage_fingerprint(
            b"omega.elf-procedure-linkage-section-descriptors.v1",
            templates.non_authoritative_template_compatibility_fingerprint(),
        );
        hash.bytes(&contents.section_name_table_seed);
        hash.bytes(&(contents.descriptors.len() as u64).to_le_bytes());
        for row in &contents.descriptors {
            hash.byte(row.kind as u8);
            hash.bytes(&row.name_offset.to_le_bytes());
            hash.bytes(&row.section_type.to_le_bytes());
            hash.bytes(&row.flags.to_le_bytes());
            hash.bytes(&row.payload_size.to_le_bytes());
            hash.bytes(&row.alignment.to_le_bytes());
            hash.bytes(&row.entry_size.to_le_bytes());
            hash.byte(row.link as u8);
            match row.info {
                ElfProcedureLinkageSectionInfo::None => hash.byte(0),
                ElfProcedureLinkageSectionInfo::RelocatedSection(kind) => {
                    hash.byte(1);
                    hash.byte(kind as u8);
                }
            }
        }
        hash.finish()
    }

    #[cfg(test)]
    mod tests {
        use super::{
            Candidate, ElfProcedureLinkageSectionDescriptor,
            ElfProcedureLinkageSectionDescriptorContents, ElfProcedureLinkageSectionInfo,
            ElfProcedureLinkageSectionKind, ElfProcedureLinkageSectionLink,
            PROCEDURE_LINKAGE_NAME_SUFFIX, SHF_AARCH64_PURECODE, SHF_ALLOC, SHF_EXECINSTR,
            SHF_INFO_LINK, SHF_WRITE, SHT_PROGBITS, SHT_RELA, TargetProfile,
            ValidatedElfProcedureLinkageTemplatePlan, checked_sum, checked_u32, derive_contents,
            non_authoritative_descriptor_compatibility_fingerprint,
            plan_elf_procedure_linkage_section_descriptors, target, validate_candidate,
            validate_contents, validate_name,
        };
        use crate::{
            plan_elf_dynamic_section_descriptors, plan_elf_dynamic_sections,
            plan_elf_procedure_linkage_relocations, plan_elf_procedure_linkage_templates,
            serialize_elf_dynamic_sections,
        };

        const IMPORTS: [crate::test_support::DynamicImportFixture; 2] =
            crate::test_support::PAIR_IMPORTS;

        use crate::test_support::DynamicImportFixture as ImportFixture;
        fn templates(
            target: TargetProfile,
            imports: &[ImportFixture],
        ) -> ValidatedElfProcedureLinkageTemplatePlan {
            let inputs = crate::test_support::dynamic_import_inputs(
                target,
                imports,
                "__omega_linkage_descriptor_import_",
                "valid descriptor locator",
                "valid descriptor interpreter",
            );
            let sections = plan_elf_dynamic_sections(inputs).expect("valid dynamic sections");
            let payloads =
                serialize_elf_dynamic_sections(sections).expect("valid dynamic payloads");
            let descriptors =
                plan_elf_dynamic_section_descriptors(payloads).expect("valid base descriptors");
            let linkage = plan_elf_procedure_linkage_relocations(descriptors)
                .expect("valid semantic procedure linkage");
            plan_elf_procedure_linkage_templates(linkage).expect("valid target templates")
        }

        fn candidate(target: TargetProfile) -> Candidate {
            let templates = templates(target, &IMPORTS);
            let contents = derive_contents(&templates).expect("derived linkage descriptors");
            let non_authoritative_descriptor_compatibility_fingerprint =
                non_authoritative_descriptor_compatibility_fingerprint(&templates, &contents);
            Candidate {
                templates,
                contents,
                non_authoritative_descriptor_compatibility_fingerprint,
            }
        }

        fn row(
            contents: &ElfProcedureLinkageSectionDescriptorContents,
            kind: ElfProcedureLinkageSectionKind,
        ) -> &ElfProcedureLinkageSectionDescriptor {
            contents
                .descriptors
                .iter()
                .find(|row| row.kind == kind)
                .expect("descriptor row")
        }

        #[test]
        fn both_targets_append_exact_names_and_address_free_metadata() {
            for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                let plan =
                    plan_elf_procedure_linkage_section_descriptors(templates(target, &IMPORTS))
                        .expect("validated linkage descriptors");
                assert_eq!(plan.templates.linkage().descriptors().descriptor_count(), 7);
                assert_eq!(
                    plan.templates
                        .linkage()
                        .descriptors()
                        .section_name_seed_byte_count(),
                    79,
                );
                assert_eq!(plan.descriptor_count(), 11);
                assert_eq!(plan.appended_descriptor_count(), 4);
                assert_eq!(plan.section_name_seed_byte_count(), 113);
                let base = plan
                    .templates
                    .linkage()
                    .descriptors()
                    .section_name_table_seed();
                assert_eq!(&plan.contents.section_name_table_seed[..base.len()], base);
                assert_eq!(
                    &plan.contents.section_name_table_seed[79..],
                    PROCEDURE_LINKAGE_NAME_SUFFIX,
                );

                let plt = row(
                    &plan.contents,
                    ElfProcedureLinkageSectionKind::ProcedureLinkage,
                );
                assert_eq!(plt.name_offset, 79);
                assert_eq!(plt.section_type, SHT_PROGBITS);
                assert_eq!(plt.alignment, 16);
                assert_eq!(plt.entry_size, 0);
                assert_eq!(plt.link, ElfProcedureLinkageSectionLink::None);
                assert_eq!(plt.info, ElfProcedureLinkageSectionInfo::None);
                assert_eq!(
                    plt.flags,
                    match target {
                        TargetProfile::LinuxX64 => SHF_ALLOC | SHF_EXECINSTR,
                        TargetProfile::LinuxArm64 => {
                            SHF_ALLOC | SHF_EXECINSTR | SHF_AARCH64_PURECODE
                        }
                        _ => unreachable!(),
                    },
                );
                assert_eq!(
                    plt.payload_size,
                    match target {
                        TargetProfile::LinuxX64 => 48,
                        TargetProfile::LinuxArm64 => 64,
                        _ => unreachable!(),
                    },
                );

                assert_eq!(
                    *row(&plan.contents, ElfProcedureLinkageSectionKind::ProcedureGot,),
                    ElfProcedureLinkageSectionDescriptor {
                        kind: ElfProcedureLinkageSectionKind::ProcedureGot,
                        name_offset: 84,
                        section_type: SHT_PROGBITS,
                        flags: SHF_ALLOC | SHF_WRITE,
                        payload_size: 40,
                        alignment: 8,
                        entry_size: 0,
                        link: ElfProcedureLinkageSectionLink::None,
                        info: ElfProcedureLinkageSectionInfo::None,
                    },
                );
                assert_eq!(
                    *row(
                        &plan.contents,
                        ElfProcedureLinkageSectionKind::ProcedureRelocation,
                    ),
                    ElfProcedureLinkageSectionDescriptor {
                        kind: ElfProcedureLinkageSectionKind::ProcedureRelocation,
                        name_offset: 93,
                        section_type: SHT_RELA,
                        flags: SHF_ALLOC | SHF_INFO_LINK,
                        payload_size: 48,
                        alignment: 8,
                        entry_size: 24,
                        link: ElfProcedureLinkageSectionLink::DynamicSymbol,
                        info: ElfProcedureLinkageSectionInfo::RelocatedSection(
                            ElfProcedureLinkageSectionKind::ProcedureGot,
                        ),
                    },
                );
                assert_eq!(
                    *row(
                        &plan.contents,
                        ElfProcedureLinkageSectionKind::GeneralRelocation,
                    ),
                    ElfProcedureLinkageSectionDescriptor {
                        kind: ElfProcedureLinkageSectionKind::GeneralRelocation,
                        name_offset: 103,
                        section_type: SHT_RELA,
                        flags: SHF_ALLOC | SHF_INFO_LINK,
                        payload_size: 0,
                        alignment: 8,
                        entry_size: 24,
                        link: ElfProcedureLinkageSectionLink::DynamicSymbol,
                        info: ElfProcedureLinkageSectionInfo::None,
                    },
                );
                assert_ne!(
                    plan.non_authoritative_descriptor_compatibility_fingerprint(),
                    0
                );
                validate_contents(plan.templates(), &plan.contents)
                    .expect("independent linkage-descriptor replay");
            }
        }

        #[test]
        fn import_permutation_preserves_identity_and_target_policy_changes_it() {
            let forward = plan_elf_procedure_linkage_section_descriptors(templates(
                TargetProfile::LinuxX64,
                &IMPORTS,
            ))
            .unwrap();
            let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
            let reverse = plan_elf_procedure_linkage_section_descriptors(templates(
                TargetProfile::LinuxX64,
                &reverse_imports,
            ))
            .unwrap();
            let arm = plan_elf_procedure_linkage_section_descriptors(templates(
                TargetProfile::LinuxArm64,
                &IMPORTS,
            ))
            .unwrap();

            assert_eq!(
                forward.non_authoritative_descriptor_compatibility_fingerprint(),
                reverse.non_authoritative_descriptor_compatibility_fingerprint()
            );
            assert_eq!(forward.contents, reverse.contents);
            assert_ne!(
                forward.non_authoritative_descriptor_compatibility_fingerprint(),
                arm.non_authoritative_descriptor_compatibility_fingerprint()
            );
            assert_ne!(
                row(
                    &forward.contents,
                    ElfProcedureLinkageSectionKind::ProcedureLinkage,
                )
                .flags,
                row(
                    &arm.contents,
                    ElfProcedureLinkageSectionKind::ProcedureLinkage,
                )
                .flags,
            );
        }

        #[test]
        fn independent_replay_rejects_every_descriptor_field_and_identity_corruption() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|candidate| candidate.contents.section_name_table_seed[0] ^= 1),
                Box::new(|candidate| {
                    let base_len = candidate
                        .templates
                        .linkage()
                        .descriptors()
                        .section_name_seed_byte_count();
                    candidate.contents.section_name_table_seed[base_len] ^= 1;
                }),
                Box::new(|candidate| {
                    *candidate
                        .contents
                        .section_name_table_seed
                        .last_mut()
                        .unwrap() ^= 1
                }),
                Box::new(|candidate| {
                    candidate.contents.section_name_table_seed.pop();
                }),
                Box::new(|candidate| candidate.contents.section_name_table_seed.push(0)),
                Box::new(|candidate| {
                    candidate.contents.descriptors.pop();
                }),
                Box::new(|candidate| {
                    candidate
                        .contents
                        .descriptors
                        .push(candidate.contents.descriptors[0])
                }),
                Box::new(|candidate| candidate.contents.descriptors.swap(0, 1)),
                Box::new(|candidate| {
                    candidate.contents.descriptors[0].kind =
                        ElfProcedureLinkageSectionKind::ProcedureGot
                }),
                Box::new(|candidate| candidate.contents.descriptors[0].name_offset = u32::MAX),
                Box::new(|candidate| candidate.contents.descriptors[0].section_type = SHT_RELA),
                Box::new(|candidate| candidate.contents.descriptors[0].flags ^= SHF_WRITE),
                Box::new(|candidate| candidate.contents.descriptors[0].payload_size += 1),
                Box::new(|candidate| candidate.contents.descriptors[0].alignment = 3),
                Box::new(|candidate| candidate.contents.descriptors[0].entry_size = 16),
                Box::new(|candidate| {
                    candidate.contents.descriptors[0].link =
                        ElfProcedureLinkageSectionLink::DynamicSymbol
                }),
                Box::new(|candidate| {
                    candidate.contents.descriptors[0].info =
                        ElfProcedureLinkageSectionInfo::RelocatedSection(
                            ElfProcedureLinkageSectionKind::ProcedureGot,
                        )
                }),
                Box::new(|candidate| {
                    candidate.contents.descriptors[2].link = ElfProcedureLinkageSectionLink::None
                }),
                Box::new(|candidate| {
                    candidate.contents.descriptors[2].info =
                        ElfProcedureLinkageSectionInfo::RelocatedSection(
                            ElfProcedureLinkageSectionKind::ProcedureLinkage,
                        )
                }),
                Box::new(|candidate| {
                    candidate.non_authoritative_descriptor_compatibility_fingerprint ^= 1
                }),
            ];

            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxX64);
                let expected_identity = candidate
                    .templates
                    .non_authoritative_template_compatibility_fingerprint();
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("corrupt linkage descriptor candidate must reject");
                assert_eq!(
                    error
                        .candidate
                        .templates
                        .non_authoritative_template_compatibility_fingerprint(),
                    expected_identity,
                    "linkage-descriptor rejection retains exact template custody",
                );
            }
        }

        #[test]
        fn aarch64_purecode_and_typed_relocation_semantics_are_exact() {
            let mut candidate = candidate(TargetProfile::LinuxArm64);
            let plt = row(
                &candidate.contents,
                ElfProcedureLinkageSectionKind::ProcedureLinkage,
            );
            assert_eq!(plt.flags, SHF_ALLOC | SHF_EXECINSTR | SHF_AARCH64_PURECODE,);
            let rela = row(
                &candidate.contents,
                ElfProcedureLinkageSectionKind::ProcedureRelocation,
            );
            assert_eq!(rela.link, ElfProcedureLinkageSectionLink::DynamicSymbol);
            assert_eq!(
                rela.info,
                ElfProcedureLinkageSectionInfo::RelocatedSection(
                    ElfProcedureLinkageSectionKind::ProcedureGot,
                ),
            );

            candidate.contents.descriptors[0].flags &= !SHF_AARCH64_PURECODE;
            let error = validate_candidate(candidate)
                .expect_err("AArch64 PLT without pure-code identity must reject");
            assert_eq!(
                target(&error.candidate.templates),
                TargetProfile::LinuxArm64,
            );
        }

        #[test]
        fn malformed_offsets_lengths_and_arithmetic_reject_without_panicking() {
            assert!(checked_sum(usize::MAX, 1, "sum").is_err());
            assert!(checked_u32(usize::MAX, "word").is_err());
            let seed = b"\0.plt\0";
            let mut row = ElfProcedureLinkageSectionDescriptor {
                kind: ElfProcedureLinkageSectionKind::ProcedureLinkage,
                name_offset: u32::MAX,
                section_type: SHT_PROGBITS,
                flags: SHF_ALLOC | SHF_EXECINSTR,
                payload_size: 16,
                alignment: 16,
                entry_size: 0,
                link: ElfProcedureLinkageSectionLink::None,
                info: ElfProcedureLinkageSectionInfo::None,
            };
            assert!(validate_name(seed, &row).is_err());
            row.name_offset = 1;
            assert!(validate_name(b"\0.plt", &row).is_err());
            row.name_offset = 2;
            assert!(validate_name(seed, &row).is_err());
        }
    }
}

pub(crate) mod dynamic_linkage_templates {
    use crate::constants::ELF64_RELA_SIZE;
    mod candidates {
        use crate::bytes::write_u64;
        use crate::dynamic_executable::checked::{checked_product, checked_sum, checked_u32};
        use crate::dynamic_executable::procedure_linkage::dynamic_import_relocations::{
            ElfProcedureLinkageRelocationContents, ValidatedElfProcedureLinkageRelocationPlan,
        };
        use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::template_plans::{
            ElfProcedureLinkageFixup, ElfProcedureLinkageFixupKind, ElfProcedureLinkageFixupStorage,
            ElfProcedureLinkagePlacementConstraint, ElfProcedureLinkagePlacementConstraintKind,
            ElfProcedureLinkageSemanticTarget, ElfProcedureLinkageTemplateBytes,
            ElfProcedureLinkageTemplateContents, ElfProcedureLinkageTemplatePolicy,
        };
        use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::{
            AARCH64_PLT_ENTRY, AARCH64_PLT_ENTRY_SIZE, AARCH64_PLT_HEADER, AARCH64_PLT_HEADER_SIZE,
            ELF64_GOT_WORD_SIZE, ELF64_RELA_SIZE, GOT_PLT_HEADER_WORDS, X86_PLT_ENTRY_SIZE, X86_PLT_HEADER,
            X86_PLT_HEADER_SIZE,
        };
        use diagnostics::Diagnostic;
        use target::TargetProfile;

        pub(crate) struct Candidate {
            pub(super) linkage: ValidatedElfProcedureLinkageRelocationPlan,
            pub(super) contents: ElfProcedureLinkageTemplateContents,
            pub(super) non_authoritative_template_compatibility_fingerprint: u64,
        }

        pub(crate) use super::super::super::checked::CandidateValidationError;

        pub(crate) fn derive_contents(
            linkage: &ValidatedElfProcedureLinkageRelocationPlan,
        ) -> Result<ElfProcedureLinkageTemplateContents, Diagnostic> {
            let target = target(linkage);
            let policy = match target {
                TargetProfile::LinuxX64 => ElfProcedureLinkageTemplatePolicy::X86_64SmallMediumLazy,
                TargetProfile::LinuxArm64 => ElfProcedureLinkageTemplatePolicy::Aarch64StandardLazy,
                _ => {
                    return Err(Diagnostic::error(
                        "ELF GOT/PLT templates require an exact Linux x86-64 or AArch64 profile",
                    ));
                }
            };
            let bytes = encode_template_bytes(target, linkage.contents())?;
            let fixups = derive_fixups(target, linkage.contents())?;
            let constraints = derive_constraints(&fixups)?;
            Ok(ElfProcedureLinkageTemplateContents {
                policy,
                bytes,
                fixups,
                constraints,
            })
        }

        pub(crate) fn target(
            linkage: &ValidatedElfProcedureLinkageRelocationPlan,
        ) -> TargetProfile {
            linkage
                .descriptors()
                .payloads()
                .plan()
                .inputs()
                .interpreter()
                .target()
        }

        fn encode_template_bytes(
            target: TargetProfile,
            linkage: &ElfProcedureLinkageRelocationContents,
        ) -> Result<ElfProcedureLinkageTemplateBytes, Diagnostic> {
            let slot_count = linkage.slots.len();
            let mut plt = match target {
                TargetProfile::LinuxX64 => Vec::with_capacity(checked_sum(
                    X86_PLT_HEADER_SIZE,
                    checked_product(slot_count, X86_PLT_ENTRY_SIZE, "x86-64 PLT entries")?,
                    "x86-64 PLT template size",
                )?),
                TargetProfile::LinuxArm64 => Vec::with_capacity(checked_sum(
                    AARCH64_PLT_HEADER_SIZE,
                    checked_product(slot_count, AARCH64_PLT_ENTRY_SIZE, "AArch64 PLT entries")?,
                    "AArch64 PLT template size",
                )?),
                _ => return Err(Diagnostic::error("unsupported ELF GOT/PLT template target")),
            };
            match target {
                TargetProfile::LinuxX64 => {
                    plt.extend(X86_PLT_HEADER);
                    for slot in &linkage.slots {
                        let relocation_index =
                            i32::try_from(slot.logical_ordinal).map_err(|_| {
                                Diagnostic::error(
                                    "x86-64 lazy-binding relocation index exceeds signed imm32",
                                )
                            })?;
                        plt.extend([0xff, 0x25, 0, 0, 0, 0, 0x68]);
                        plt.extend(relocation_index.to_le_bytes());
                        plt.extend([0xe9, 0, 0, 0, 0]);
                    }
                }
                TargetProfile::LinuxArm64 => {
                    plt.extend(AARCH64_PLT_HEADER);
                    for _ in &linkage.slots {
                        plt.extend(AARCH64_PLT_ENTRY);
                    }
                }
                _ => unreachable!("target was checked above"),
            }

            let got_word_count =
                checked_sum(GOT_PLT_HEADER_WORDS, slot_count, "procedure GOT word count")?;
            let got_plt = vec![
                0;
                checked_product(
                    got_word_count,
                    ELF64_GOT_WORD_SIZE,
                    "procedure GOT template size",
                )?
            ];
            let mut rela_plt = Vec::with_capacity(checked_product(
                linkage.jump_slot_relocations.len(),
                ELF64_RELA_SIZE,
                "procedure RELA template size",
            )?);
            for relocation in &linkage.jump_slot_relocations {
                write_u64(&mut rela_plt, 0);
                let info = (u64::from(relocation.dynamic_symbol_index) << 32)
                    | u64::from(relocation.relocation_type);
                write_u64(&mut rela_plt, info);
                write_u64(&mut rela_plt, relocation.addend as u64);
            }
            let mut rela_dyn = Vec::with_capacity(checked_product(
                linkage.general_relocations.len(),
                ELF64_RELA_SIZE,
                "general RELA template size",
            )?);
            for relocation in &linkage.general_relocations {
                write_u64(&mut rela_dyn, 0);
                let info = (u64::from(relocation.dynamic_symbol_index) << 32)
                    | u64::from(relocation.relocation_type);
                write_u64(&mut rela_dyn, info);
                write_u64(&mut rela_dyn, relocation.addend as u64);
            }
            Ok(ElfProcedureLinkageTemplateBytes {
                plt,
                got_plt,
                rela_plt,
                rela_dyn,
            })
        }

        pub(crate) fn derive_fixups(
            target: TargetProfile,
            linkage: &ElfProcedureLinkageRelocationContents,
        ) -> Result<Vec<ElfProcedureLinkageFixup>, Diagnostic> {
            let mut fixups = Vec::new();
            match target {
                TargetProfile::LinuxX64 => {
                    push_fixup(
                        &mut fixups,
                        ElfProcedureLinkageFixupStorage::Plt,
                        2,
                        ElfProcedureLinkageFixupKind::X86PcRelative32,
                        ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index: 1 },
                    );
                    push_fixup(
                        &mut fixups,
                        ElfProcedureLinkageFixupStorage::Plt,
                        8,
                        ElfProcedureLinkageFixupKind::X86PcRelative32,
                        ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index: 2 },
                    );
                    for slot in &linkage.slots {
                        let entry = checked_sum(
                            X86_PLT_HEADER_SIZE,
                            checked_product(
                                slot.logical_ordinal as usize,
                                X86_PLT_ENTRY_SIZE,
                                "x86-64 PLT entry offset",
                            )?,
                            "x86-64 PLT entry offset",
                        )?;
                        push_fixup(
                            &mut fixups,
                            ElfProcedureLinkageFixupStorage::Plt,
                            checked_sum(entry, 2, "x86-64 GOT displacement offset")?,
                            ElfProcedureLinkageFixupKind::X86PcRelative32,
                            ElfProcedureLinkageSemanticTarget::GotPltSlot {
                                logical_ordinal: slot.logical_ordinal,
                            },
                        );
                        push_fixup(
                            &mut fixups,
                            ElfProcedureLinkageFixupStorage::Plt,
                            checked_sum(entry, 12, "x86-64 PLT0 displacement offset")?,
                            ElfProcedureLinkageFixupKind::X86PcRelative32,
                            ElfProcedureLinkageSemanticTarget::PltHeader,
                        );
                    }
                    push_fixup(
                        &mut fixups,
                        ElfProcedureLinkageFixupStorage::GotPlt,
                        0,
                        ElfProcedureLinkageFixupKind::Absolute64,
                        ElfProcedureLinkageSemanticTarget::FutureDynamicSection,
                    );
                    for slot in &linkage.slots {
                        push_fixup(
                            &mut fixups,
                            ElfProcedureLinkageFixupStorage::GotPlt,
                            got_slot_byte_offset(slot.logical_ordinal)?,
                            ElfProcedureLinkageFixupKind::Absolute64,
                            ElfProcedureLinkageSemanticTarget::PltLazyTail {
                                logical_ordinal: slot.logical_ordinal,
                            },
                        );
                    }
                }
                TargetProfile::LinuxArm64 => {
                    for (offset, kind) in [
                        (4, ElfProcedureLinkageFixupKind::Aarch64Page21),
                        (8, ElfProcedureLinkageFixupKind::Aarch64Load64Low12),
                        (12, ElfProcedureLinkageFixupKind::Aarch64AddLow12),
                    ] {
                        push_fixup(
                            &mut fixups,
                            ElfProcedureLinkageFixupStorage::Plt,
                            offset,
                            kind,
                            ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index: 2 },
                        );
                    }
                    for slot in &linkage.slots {
                        let entry = checked_sum(
                            AARCH64_PLT_HEADER_SIZE,
                            checked_product(
                                slot.logical_ordinal as usize,
                                AARCH64_PLT_ENTRY_SIZE,
                                "AArch64 PLT entry offset",
                            )?,
                            "AArch64 PLT entry offset",
                        )?;
                        for (field, kind) in [
                            (0, ElfProcedureLinkageFixupKind::Aarch64Page21),
                            (4, ElfProcedureLinkageFixupKind::Aarch64Load64Low12),
                            (8, ElfProcedureLinkageFixupKind::Aarch64AddLow12),
                        ] {
                            push_fixup(
                                &mut fixups,
                                ElfProcedureLinkageFixupStorage::Plt,
                                checked_sum(entry, field, "AArch64 PLT fixup offset")?,
                                kind,
                                ElfProcedureLinkageSemanticTarget::GotPltSlot {
                                    logical_ordinal: slot.logical_ordinal,
                                },
                            );
                        }
                    }
                    for slot in &linkage.slots {
                        push_fixup(
                            &mut fixups,
                            ElfProcedureLinkageFixupStorage::GotPlt,
                            got_slot_byte_offset(slot.logical_ordinal)?,
                            ElfProcedureLinkageFixupKind::Absolute64,
                            ElfProcedureLinkageSemanticTarget::PltHeader,
                        );
                    }
                }
                _ => return Err(Diagnostic::error("unsupported ELF GOT/PLT fixup target")),
            }

            for (index, relocation) in linkage.jump_slot_relocations.iter().enumerate() {
                push_fixup(
                    &mut fixups,
                    ElfProcedureLinkageFixupStorage::RelaPlt,
                    checked_product(index, ELF64_RELA_SIZE, "procedure RELA r_offset")?,
                    ElfProcedureLinkageFixupKind::Elf64RelaOffset,
                    ElfProcedureLinkageSemanticTarget::GotPltSlot {
                        logical_ordinal: relocation.logical_got_slot_ordinal,
                    },
                );
            }
            for (index, relocation) in linkage.general_relocations.iter().enumerate() {
                push_fixup(
                    &mut fixups,
                    ElfProcedureLinkageFixupStorage::RelaDyn,
                    checked_product(index, ELF64_RELA_SIZE, "general RELA r_offset")?,
                    ElfProcedureLinkageFixupKind::Elf64RelaOffset,
                    ElfProcedureLinkageSemanticTarget::RelocatedImageSection {
                        section: relocation.source_section,
                        byte_offset: relocation.source_offset,
                    },
                );
            }
            for slot in &linkage.slots {
                for call_site in &slot.call_sites {
                    push_fixup(
                        &mut fixups,
                        ElfProcedureLinkageFixupStorage::SourceText,
                        call_site.relocation_offset,
                        match target {
                            TargetProfile::LinuxX64 => {
                                ElfProcedureLinkageFixupKind::X86PcRelative32
                            }
                            TargetProfile::LinuxArm64 => {
                                ElfProcedureLinkageFixupKind::Aarch64Branch26
                            }
                            _ => unreachable!("target was checked above"),
                        },
                        ElfProcedureLinkageSemanticTarget::PltEntry {
                            logical_ordinal: slot.logical_ordinal,
                        },
                    );
                }
            }
            Ok(fixups)
        }

        fn push_fixup(
            fixups: &mut Vec<ElfProcedureLinkageFixup>,
            storage: ElfProcedureLinkageFixupStorage,
            byte_offset: usize,
            kind: ElfProcedureLinkageFixupKind,
            target: ElfProcedureLinkageSemanticTarget,
        ) {
            let (byte_width, mutable_mask) = fixup_field(kind);
            fixups.push(ElfProcedureLinkageFixup {
                storage,
                byte_offset,
                byte_width,
                mutable_mask,
                kind,
                target,
            });
        }

        pub(crate) const fn fixup_field(kind: ElfProcedureLinkageFixupKind) -> (u8, u64) {
            match kind {
                ElfProcedureLinkageFixupKind::X86PcRelative32 => (4, 0xffff_ffff),
                ElfProcedureLinkageFixupKind::Aarch64Page21 => (4, 0x60ff_ffe0),
                ElfProcedureLinkageFixupKind::Aarch64Load64Low12
                | ElfProcedureLinkageFixupKind::Aarch64AddLow12 => (4, 0x003f_fc00),
                ElfProcedureLinkageFixupKind::Aarch64Branch26 => (4, 0x03ff_ffff),
                ElfProcedureLinkageFixupKind::Absolute64
                | ElfProcedureLinkageFixupKind::Elf64RelaOffset => (8, u64::MAX),
            }
        }

        pub(crate) fn derive_constraints(
            fixups: &[ElfProcedureLinkageFixup],
        ) -> Result<Vec<ElfProcedureLinkagePlacementConstraint>, Diagnostic> {
            let mut constraints = Vec::new();
            for (index, fixup) in fixups.iter().enumerate() {
                let kind = match fixup.kind {
                    ElfProcedureLinkageFixupKind::X86PcRelative32 => {
                        Some(ElfProcedureLinkagePlacementConstraintKind::X86Signed32)
                    }
                    ElfProcedureLinkageFixupKind::Aarch64Page21 => {
                        Some(ElfProcedureLinkagePlacementConstraintKind::Aarch64PageDelta21)
                    }
                    ElfProcedureLinkageFixupKind::Aarch64Load64Low12 => {
                        Some(ElfProcedureLinkagePlacementConstraintKind::Aarch64Load64Low12Aligned)
                    }
                    ElfProcedureLinkageFixupKind::Aarch64Branch26 => {
                        Some(ElfProcedureLinkagePlacementConstraintKind::Aarch64Branch26)
                    }
                    ElfProcedureLinkageFixupKind::Aarch64AddLow12
                    | ElfProcedureLinkageFixupKind::Absolute64
                    | ElfProcedureLinkageFixupKind::Elf64RelaOffset => None,
                };
                if let Some(kind) = kind {
                    constraints.push(ElfProcedureLinkagePlacementConstraint {
                        fixup_ordinal: checked_u32(index, "procedure-linkage fixup ordinal")?,
                        kind,
                    });
                }
            }
            Ok(constraints)
        }

        fn got_slot_byte_offset(logical_ordinal: u32) -> Result<usize, Diagnostic> {
            checked_product(
                checked_sum(
                    GOT_PLT_HEADER_WORDS,
                    logical_ordinal as usize,
                    "procedure GOT slot word index",
                )?,
                ELF64_GOT_WORD_SIZE,
                "procedure GOT slot byte offset",
            )
        }
    }

    mod compatibility_fingerprint {
        use target::compatibility_fingerprint::Fnv1a;

        use crate::dynamic_executable::procedure_linkage::dynamic_import_relocations::ValidatedElfProcedureLinkageRelocationPlan;
        use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::template_plans::{
            ElfProcedureLinkageSemanticTarget, ElfProcedureLinkageTemplateContents,
        };

        pub(crate) fn non_authoritative_template_compatibility_fingerprint(
            linkage: &ValidatedElfProcedureLinkageRelocationPlan,
            contents: &ElfProcedureLinkageTemplateContents,
        ) -> u64 {
            let mut hash = crate::dynamic_executable::checked::stage_fingerprint(
                b"omega.elf-procedure-linkage-templates.v1",
                linkage.non_authoritative_linkage_compatibility_fingerprint(),
            );
            hash.byte(contents.policy as u8);
            hash.bytes(&contents.bytes.plt);
            hash.bytes(&contents.bytes.got_plt);
            hash.bytes(&contents.bytes.rela_plt);
            hash.bytes(&contents.bytes.rela_dyn);
            hash.bytes(&(contents.fixups.len() as u64).to_le_bytes());
            for fixup in &contents.fixups {
                hash.byte(fixup.storage as u8);
                hash.bytes(&(fixup.byte_offset as u64).to_le_bytes());
                hash.byte(fixup.byte_width);
                hash.bytes(&fixup.mutable_mask.to_le_bytes());
                hash.byte(fixup.kind as u8);
                hash_semantic_target(&mut hash, fixup.target);
            }
            hash.bytes(&(contents.constraints.len() as u64).to_le_bytes());
            for constraint in &contents.constraints {
                hash.bytes(&constraint.fixup_ordinal.to_le_bytes());
                hash.byte(constraint.kind as u8);
            }
            hash.finish()
        }

        fn hash_semantic_target(hash: &mut Fnv1a, target: ElfProcedureLinkageSemanticTarget) {
            match target {
                ElfProcedureLinkageSemanticTarget::FutureDynamicSection => hash.byte(1),
                ElfProcedureLinkageSemanticTarget::PltHeader => hash.byte(2),
                ElfProcedureLinkageSemanticTarget::PltEntry { logical_ordinal } => {
                    hash.byte(3);
                    hash.bytes(&logical_ordinal.to_le_bytes());
                }
                ElfProcedureLinkageSemanticTarget::PltLazyTail { logical_ordinal } => {
                    hash.byte(4);
                    hash.bytes(&logical_ordinal.to_le_bytes());
                }
                ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index } => {
                    hash.byte(5);
                    hash.byte(word_index);
                }
                ElfProcedureLinkageSemanticTarget::GotPltSlot { logical_ordinal } => {
                    hash.byte(6);
                    hash.bytes(&logical_ordinal.to_le_bytes());
                }
                ElfProcedureLinkageSemanticTarget::RelocatedImageSection {
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

    mod template_plans {
        use crate::dynamic_executable::procedure_linkage::dynamic_import_relocations::ValidatedElfProcedureLinkageRelocationPlan;
        use diagnostics::Diagnostic;
        use image::FinalImageSection;

        #[derive(Debug)]
        #[must_use = "validated GOT/PLT templates retain the exact linkage plan"]
        pub struct ValidatedElfProcedureLinkageTemplatePlan {
            pub(crate) linkage: ValidatedElfProcedureLinkageRelocationPlan,
            pub(crate) contents: ElfProcedureLinkageTemplateContents,
            pub(crate) non_authoritative_template_compatibility_fingerprint: u64,
        }

        impl ValidatedElfProcedureLinkageTemplatePlan {
            pub const fn linkage(&self) -> &ValidatedElfProcedureLinkageRelocationPlan {
                &self.linkage
            }

            #[cfg(test)]
            pub(crate) fn procedure_linkage_byte_count(&self) -> usize {
                self.contents.bytes.plt.len()
            }

            #[cfg(test)]
            pub(crate) fn procedure_got_byte_count(&self) -> usize {
                self.contents.bytes.got_plt.len()
            }

            pub(crate) fn procedure_relocation_byte_count(&self) -> usize {
                self.contents.bytes.rela_plt.len()
            }

            pub(crate) fn general_relocation_byte_count(&self) -> usize {
                self.contents.bytes.rela_dyn.len()
            }

            #[cfg(test)]
            pub(crate) fn fixup_count(&self) -> usize {
                self.contents.fixups.len()
            }

            #[cfg(test)]
            pub(crate) fn placement_constraint_count(&self) -> usize {
                self.contents.constraints.len()
            }

            pub const fn non_authoritative_template_compatibility_fingerprint(&self) -> u64 {
                self.non_authoritative_template_compatibility_fingerprint
            }

            pub(crate) const fn contents(&self) -> &ElfProcedureLinkageTemplateContents {
                &self.contents
            }

            pub(crate) fn into_parts(
                self,
            ) -> (
                ValidatedElfProcedureLinkageRelocationPlan,
                ElfProcedureLinkageTemplateContents,
            ) {
                (self.linkage, self.contents)
            }
        }

        #[derive(Debug)]
        #[must_use = "ELF GOT/PLT template rejection retains the procedure-linkage plan"]
        pub struct ElfProcedureLinkageTemplatePlanningError {
            pub(crate) linkage: ValidatedElfProcedureLinkageRelocationPlan,
            pub(crate) diagnostic: Diagnostic,
        }

        image::rejection_carrier!(
            ElfProcedureLinkageTemplatePlanningError;
            linkage: ValidatedElfProcedureLinkageRelocationPlan,
        );

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct ElfProcedureLinkageTemplateContents {
            pub(crate) policy: ElfProcedureLinkageTemplatePolicy,
            pub(crate) bytes: ElfProcedureLinkageTemplateBytes,
            pub(crate) fixups: Vec<ElfProcedureLinkageFixup>,
            pub(crate) constraints: Vec<ElfProcedureLinkagePlacementConstraint>,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u8)]
        pub(crate) enum ElfProcedureLinkageTemplatePolicy {
            X86_64SmallMediumLazy = 1,
            Aarch64StandardLazy = 2,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct ElfProcedureLinkageTemplateBytes {
            pub(crate) plt: Vec<u8>,
            pub(crate) got_plt: Vec<u8>,
            pub(crate) rela_plt: Vec<u8>,
            pub(crate) rela_dyn: Vec<u8>,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct ElfProcedureLinkageFixup {
            pub(crate) storage: ElfProcedureLinkageFixupStorage,
            pub(crate) byte_offset: usize,
            pub(crate) byte_width: u8,
            pub(crate) mutable_mask: u64,
            pub(crate) kind: ElfProcedureLinkageFixupKind,
            pub(crate) target: ElfProcedureLinkageSemanticTarget,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u8)]
        pub(crate) enum ElfProcedureLinkageFixupStorage {
            SourceText = 1,
            Plt = 2,
            GotPlt = 3,
            RelaPlt = 4,
            RelaDyn = 5,
        }

        pub(crate) type ElfProcedureLinkageFixupKind = crate::ElfAppliedProcedureLinkageKind;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum ElfProcedureLinkageSemanticTarget {
            FutureDynamicSection,
            PltHeader,
            PltEntry {
                logical_ordinal: u32,
            },
            PltLazyTail {
                logical_ordinal: u32,
            },
            GotPltHeaderWord {
                word_index: u8,
            },
            GotPltSlot {
                logical_ordinal: u32,
            },
            RelocatedImageSection {
                section: FinalImageSection,
                byte_offset: usize,
            },
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct ElfProcedureLinkagePlacementConstraint {
            pub(crate) fixup_ordinal: u32,
            pub(crate) kind: ElfProcedureLinkagePlacementConstraintKind,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u8)]
        pub(crate) enum ElfProcedureLinkagePlacementConstraintKind {
            X86Signed32 = 1,
            Aarch64PageDelta21 = 2,
            Aarch64Load64Low12Aligned = 3,
            Aarch64Branch26 = 4,
        }
    }

    mod template_validation {
        use crate::dynamic_executable::procedure_linkage::dynamic_import_relocations::{ElfProcedureLinkageRelocationContents, ValidatedElfProcedureLinkageRelocationPlan};
        use diagnostics::{Diagnostic};
        use target::{TargetProfile};
        use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::{ValidatedElfProcedureLinkageTemplatePlan, AARCH64_PLT_ENTRY, AARCH64_PLT_ENTRY_SIZE, AARCH64_PLT_HEADER, AARCH64_PLT_HEADER_SIZE, ELF64_GOT_WORD_SIZE, ELF64_RELA_SIZE, GOT_PLT_HEADER_WORDS, X86_PLT_ENTRY_SIZE, X86_PLT_HEADER, X86_PLT_HEADER_SIZE};
        use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::candidates::{Candidate, CandidateValidationError, derive_constraints, derive_fixups, fixup_field, target};
        use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::compatibility_fingerprint::{non_authoritative_template_compatibility_fingerprint};
        use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::template_plans::{ElfProcedureLinkageFixup, ElfProcedureLinkageFixupStorage, ElfProcedureLinkageSemanticTarget, ElfProcedureLinkageTemplateBytes, ElfProcedureLinkageTemplateContents, ElfProcedureLinkageTemplatePolicy};
        use crate::dynamic_executable::checked::{checked_product, checked_sum, read_field, read_u32, read_u64, require};
        use super::super::super::checked;

        checked::seal_stage! {
            field: linkage,
            contents: contents,
            validated: ValidatedElfProcedureLinkageTemplatePlan,
            validate: validate_contents,
            fingerprint: non_authoritative_template_compatibility_fingerprint,
            message: "ELF GOT/PLT template compatibility fingerprint does not replay",
            vis: pub(crate),
        }

        pub(crate) fn validate_contents(
            linkage: &ValidatedElfProcedureLinkageRelocationPlan,
            contents: &ElfProcedureLinkageTemplateContents,
        ) -> Result<(), Diagnostic> {
            let target = target(linkage);
            let expected_policy = match target {
                TargetProfile::LinuxX64 => ElfProcedureLinkageTemplatePolicy::X86_64SmallMediumLazy,
                TargetProfile::LinuxArm64 => ElfProcedureLinkageTemplatePolicy::Aarch64StandardLazy,
                _ => return Err(Diagnostic::error("unsupported validated GOT/PLT target")),
            };
            require(
                contents.policy == expected_policy,
                "ELF procedure-linkage template policy drifted from its exact target",
            )?;
            validate_fixed_template_bytes(target, linkage.contents(), &contents.bytes)?;
            let expected_fixups = derive_fixups(target, linkage.contents())?;
            require(
                contents.fixups == expected_fixups,
                "ELF procedure-linkage semantic fixups are missing, duplicated, or reordered",
            )?;
            validate_fixup_placeholders(linkage, contents)?;
            validate_fixup_coverage(&contents.fixups)?;
            require(
                contents.constraints == derive_constraints(&contents.fixups)?,
                "ELF procedure-linkage placement constraints drifted from exact fixups",
            )
        }

        fn validate_fixed_template_bytes(
            target: TargetProfile,
            linkage: &ElfProcedureLinkageRelocationContents,
            bytes: &ElfProcedureLinkageTemplateBytes,
        ) -> Result<(), Diagnostic> {
            let slot_count = linkage.slots.len();
            let expected_plt_size = match target {
                TargetProfile::LinuxX64 => checked_sum(
                    X86_PLT_HEADER_SIZE,
                    checked_product(
                        slot_count,
                        X86_PLT_ENTRY_SIZE,
                        "validated x86-64 PLT entries",
                    )?,
                    "validated x86-64 PLT size",
                )?,
                TargetProfile::LinuxArm64 => checked_sum(
                    AARCH64_PLT_HEADER_SIZE,
                    checked_product(
                        slot_count,
                        AARCH64_PLT_ENTRY_SIZE,
                        "validated AArch64 PLT entries",
                    )?,
                    "validated AArch64 PLT size",
                )?,
                _ => return Err(Diagnostic::error("unsupported fixed-template target")),
            };
            let expected_got_size = checked_product(
                checked_sum(GOT_PLT_HEADER_WORDS, slot_count, "validated GOT words")?,
                ELF64_GOT_WORD_SIZE,
                "validated GOT size",
            )?;
            let expected_rela_size = checked_product(
                linkage.jump_slot_relocations.len(),
                ELF64_RELA_SIZE,
                "validated procedure RELA size",
            )?;
            let expected_rela_dyn_size = checked_product(
                linkage.general_relocations.len(),
                ELF64_RELA_SIZE,
                "validated general RELA size",
            )?;
            require(
                bytes.plt.len() == expected_plt_size
                    && bytes.got_plt.len() == expected_got_size
                    && bytes.rela_plt.len() == expected_rela_size
                    && bytes.rela_dyn.len() == expected_rela_dyn_size,
                "ELF procedure-linkage template byte lengths do not match exact rows",
            )?;
            match target {
                TargetProfile::LinuxX64 => replay_x86_plt(linkage, &bytes.plt)?,
                TargetProfile::LinuxArm64 => replay_aarch64_plt(linkage, &bytes.plt)?,
                _ => unreachable!(),
            }
            require(
                bytes.got_plt.iter().all(|byte| *byte == 0),
                "ELF procedure GOT template contains a premature address or loader value",
            )?;
            replay_rela(linkage, &bytes.rela_plt)?;
            replay_general_rela(linkage, &bytes.rela_dyn)
        }

        fn replay_x86_plt(
            linkage: &ElfProcedureLinkageRelocationContents,
            bytes: &[u8],
        ) -> Result<(), Diagnostic> {
            require(
                bytes.get(..X86_PLT_HEADER_SIZE) == Some(&X86_PLT_HEADER),
                "x86-64 PLT0 template bytes drifted",
            )?;
            for (index, slot) in linkage.slots.iter().enumerate() {
                let start = checked_sum(
                    X86_PLT_HEADER_SIZE,
                    checked_product(index, X86_PLT_ENTRY_SIZE, "decoded x86-64 PLT entry")?,
                    "decoded x86-64 PLT entry",
                )?;
                let end = checked_sum(start, X86_PLT_ENTRY_SIZE, "decoded x86-64 PLT entry end")?;
                let entry = bytes
                    .get(start..end)
                    .ok_or_else(|| Diagnostic::error("truncated x86-64 PLT entry"))?;
                require(
                    entry.get(..7) == Some(&[0xff, 0x25, 0, 0, 0, 0, 0x68])
                        && read_u32(entry, 7, "x86-64 PLT relocation ordinal")?
                            == slot.logical_ordinal
                        && entry.get(11..) == Some(&[0xe9, 0, 0, 0, 0]),
                    "x86-64 PLT entry opcode, ordinal, or placeholder drifted",
                )?;
            }
            Ok(())
        }

        fn replay_aarch64_plt(
            linkage: &ElfProcedureLinkageRelocationContents,
            bytes: &[u8],
        ) -> Result<(), Diagnostic> {
            require(
                bytes.get(..AARCH64_PLT_HEADER_SIZE) == Some(&AARCH64_PLT_HEADER),
                "AArch64 PLT0 template bytes drifted",
            )?;
            for index in 0..linkage.slots.len() {
                let start = checked_sum(
                    AARCH64_PLT_HEADER_SIZE,
                    checked_product(index, AARCH64_PLT_ENTRY_SIZE, "decoded AArch64 PLT entry")?,
                    "decoded AArch64 PLT entry",
                )?;
                let end = checked_sum(
                    start,
                    AARCH64_PLT_ENTRY_SIZE,
                    "decoded AArch64 PLT entry end",
                )?;
                require(
                    bytes.get(start..end) == Some(&AARCH64_PLT_ENTRY),
                    "AArch64 PLT entry opcode or zero placeholder drifted",
                )?;
            }
            Ok(())
        }

        macro_rules! replay_rela_rows {
            ($name:ident, $rows:ident, $domain:literal) => {
                fn $name(
                    linkage: &ElfProcedureLinkageRelocationContents,
                    bytes: &[u8],
                ) -> Result<(), Diagnostic> {
                    for (index, relocation) in linkage.$rows.iter().enumerate() {
                        let start = checked_product(
                            index,
                            ELF64_RELA_SIZE,
                            concat!("decoded ", $domain, " RELA row"),
                        )?;
                        let expected_info = (u64::from(relocation.dynamic_symbol_index) << 32)
                            | u64::from(relocation.relocation_type);
                        require(
                            read_u64(bytes, start, concat!($domain, " RELA r_offset"))? == 0
                                && read_u64(
                                    bytes,
                                    checked_sum(start, 8, concat!($domain, " RELA r_info offset"))?,
                                    concat!($domain, " RELA r_info"),
                                )? == expected_info
                                && read_u64(
                                    bytes,
                                    checked_sum(
                                        start,
                                        16,
                                        concat!($domain, " RELA addend offset"),
                                    )?,
                                    concat!($domain, " RELA addend"),
                                )? == relocation.addend as u64,
                            concat!(
                                "ELF ",
                                $domain,
                                " RELA fixed row or address placeholder drifted"
                            ),
                        )?;
                    }
                    Ok(())
                }
            };
        }

        replay_rela_rows!(replay_rela, jump_slot_relocations, "procedure");
        replay_rela_rows!(replay_general_rela, general_relocations, "general");

        fn validate_fixup_placeholders(
            linkage: &ValidatedElfProcedureLinkageRelocationPlan,
            contents: &ElfProcedureLinkageTemplateContents,
        ) -> Result<(), Diagnostic> {
            let source_text = &linkage
                .descriptors()
                .payloads()
                .plan()
                .inputs()
                .image()
                .memory
                .text;
            for fixup in &contents.fixups {
                let storage = match fixup.storage {
                    ElfProcedureLinkageFixupStorage::SourceText => source_text,
                    ElfProcedureLinkageFixupStorage::Plt => &contents.bytes.plt,
                    ElfProcedureLinkageFixupStorage::GotPlt => &contents.bytes.got_plt,
                    ElfProcedureLinkageFixupStorage::RelaPlt => &contents.bytes.rela_plt,
                    ElfProcedureLinkageFixupStorage::RelaDyn => &contents.bytes.rela_dyn,
                };
                let field = read_field(
                    storage,
                    fixup.byte_offset,
                    fixup.byte_width,
                    "procedure-linkage fixup field",
                )?;
                require(
                    field & fixup.mutable_mask == 0,
                    "ELF procedure-linkage fixup field is not an exact zero placeholder",
                )?;
                require(
                    fixup_field(fixup.kind) == (fixup.byte_width, fixup.mutable_mask),
                    "ELF procedure-linkage fixup width or mutable mask drifted from its kind",
                )?;
                validate_semantic_target(linkage.contents(), fixup.target)?;
            }
            Ok(())
        }

        fn validate_semantic_target(
            linkage: &ElfProcedureLinkageRelocationContents,
            target: ElfProcedureLinkageSemanticTarget,
        ) -> Result<(), Diagnostic> {
            let valid_ordinal = |ordinal: u32| {
                linkage
                    .slots
                    .get(ordinal as usize)
                    .is_some_and(|slot| slot.logical_ordinal == ordinal)
            };
            require(
                match target {
                    ElfProcedureLinkageSemanticTarget::FutureDynamicSection
                    | ElfProcedureLinkageSemanticTarget::PltHeader => true,
                    ElfProcedureLinkageSemanticTarget::PltEntry { logical_ordinal }
                    | ElfProcedureLinkageSemanticTarget::PltLazyTail { logical_ordinal }
                    | ElfProcedureLinkageSemanticTarget::GotPltSlot { logical_ordinal } => {
                        valid_ordinal(logical_ordinal)
                    }
                    ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index } => {
                        usize::from(word_index) < GOT_PLT_HEADER_WORDS
                    }
                    ElfProcedureLinkageSemanticTarget::RelocatedImageSection {
                        section,
                        byte_offset,
                    } => linkage.general_relocations.iter().any(|relocation| {
                        relocation.source_section == section
                            && relocation.source_offset == byte_offset
                    }),
                },
                "ELF procedure-linkage fixup target does not resolve semantically",
            )
        }

        pub(crate) fn validate_fixup_coverage(
            fixups: &[ElfProcedureLinkageFixup],
        ) -> Result<(), Diagnostic> {
            for (index, fixup) in fixups.iter().enumerate() {
                for other in &fixups[index + 1..] {
                    if fixup.storage != other.storage {
                        continue;
                    }
                    let end = checked_sum(
                        fixup.byte_offset,
                        usize::from(fixup.byte_width),
                        "procedure-linkage fixup end",
                    )?;
                    let other_end = checked_sum(
                        other.byte_offset,
                        usize::from(other.byte_width),
                        "procedure-linkage fixup end",
                    )?;
                    require(
                        end <= other.byte_offset || other_end <= fixup.byte_offset,
                        "ELF procedure-linkage fixups overlap or duplicate one field",
                    )?;
                }
            }
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{AARCH64_PLT_ENTRY, AARCH64_PLT_HEADER, ELF64_RELA_SIZE, ElfProcedureLinkageFixup, ElfProcedureLinkageFixupKind, ElfProcedureLinkageFixupStorage, ElfProcedureLinkagePlacementConstraintKind, ElfProcedureLinkageSemanticTarget, ElfProcedureLinkageTemplatePolicy, ValidatedElfProcedureLinkageRelocationPlan, non_authoritative_template_compatibility_fingerprint, plan_elf_procedure_linkage_templates};
        use crate::{
            plan_elf_dynamic_section_descriptors, plan_elf_dynamic_sections,
            plan_elf_procedure_linkage_relocations, serialize_elf_dynamic_sections,
        };
        use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::candidates::{Candidate, derive_contents};
        use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::template_validation::{validate_candidate, validate_contents, validate_fixup_coverage};
        use target::TargetProfile;
        use crate::dynamic_executable::checked::{checked_product, checked_sum, checked_u32, read_field, read_u32, read_u64};

        use crate::test_support::{IMPORTS, ImportFixture};

        fn linkage(
            target: TargetProfile,
            imports: &[ImportFixture],
        ) -> ValidatedElfProcedureLinkageRelocationPlan {
            let inputs = crate::test_support::dynamic_import_inputs(
                target,
                imports,
                "__omega_template_import_",
                "valid template locator",
                "valid template interpreter",
            );
            let sections = plan_elf_dynamic_sections(inputs).expect("valid dynamic sections");
            let payloads =
                serialize_elf_dynamic_sections(sections).expect("valid dynamic payloads");
            let descriptors =
                plan_elf_dynamic_section_descriptors(payloads).expect("valid dynamic descriptors");
            plan_elf_procedure_linkage_relocations(descriptors)
                .expect("valid semantic procedure linkage")
        }

        fn candidate(target: TargetProfile) -> Candidate {
            let linkage = linkage(target, &IMPORTS);
            let contents = derive_contents(&linkage).expect("derived templates");
            let non_authoritative_template_compatibility_fingerprint =
                non_authoritative_template_compatibility_fingerprint(&linkage, &contents);
            Candidate {
                linkage,
                contents,
                non_authoritative_template_compatibility_fingerprint,
            }
        }

        #[test]
        fn both_targets_emit_exact_fixed_templates_fixups_and_constraints() {
            for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                let plan = plan_elf_procedure_linkage_templates(linkage(target, &IMPORTS))
                    .expect("validated linkage templates");
                assert_eq!(plan.procedure_got_byte_count(), 48);
                assert_eq!(plan.procedure_relocation_byte_count(), 72);
                assert_eq!(
                    plan.procedure_linkage_byte_count(),
                    match target {
                        TargetProfile::LinuxX64 => 64,
                        TargetProfile::LinuxArm64 => 80,
                        _ => unreachable!(),
                    }
                );
                assert_eq!(
                    plan.fixup_count(),
                    match target {
                        TargetProfile::LinuxX64 => 19,
                        TargetProfile::LinuxArm64 => 22,
                        _ => unreachable!(),
                    }
                );
                assert_eq!(plan.placement_constraint_count(), 12);
                assert_ne!(
                    plan.non_authoritative_template_compatibility_fingerprint(),
                    0
                );
                assert!(plan.contents.bytes.got_plt.iter().all(|byte| *byte == 0));

                for (index, relocation) in plan
                    .linkage
                    .contents()
                    .jump_slot_relocations
                    .iter()
                    .enumerate()
                {
                    let start = index * ELF64_RELA_SIZE;
                    assert_eq!(
                        read_u64(&plan.contents.bytes.rela_plt, start, "offset").unwrap(),
                        0
                    );
                    assert_eq!(
                        read_u64(&plan.contents.bytes.rela_plt, start + 8, "info").unwrap(),
                        (u64::from(relocation.dynamic_symbol_index) << 32)
                            | u64::from(relocation.relocation_type),
                    );
                    assert_eq!(
                        read_u64(&plan.contents.bytes.rela_plt, start + 16, "addend").unwrap(),
                        0,
                    );
                }
                validate_contents(plan.linkage(), &plan.contents)
                    .expect("independent template replay");
            }
        }

        #[test]
        fn aarch64_keeps_all_three_header_words_zero_and_targets_plt_zero() {
            let plan =
                plan_elf_procedure_linkage_templates(linkage(TargetProfile::LinuxArm64, &IMPORTS))
                    .expect("validated AArch64 templates");
            assert_eq!(&plan.contents.bytes.plt[..32], &AARCH64_PLT_HEADER);
            assert_eq!(&plan.contents.bytes.plt[32..48], &AARCH64_PLT_ENTRY);
            assert_eq!(&plan.contents.bytes.got_plt[..24], &[0; 24]);
            assert!(!plan.contents.fixups.iter().any(|fixup| {
                fixup.storage == ElfProcedureLinkageFixupStorage::GotPlt && fixup.byte_offset < 24
            }));
            assert_eq!(
                plan.contents
                    .fixups
                    .iter()
                    .filter(|fixup| {
                        fixup.storage == ElfProcedureLinkageFixupStorage::GotPlt
                            && matches!(fixup.target, ElfProcedureLinkageSemanticTarget::PltHeader)
                    })
                    .count(),
                3,
            );
            assert!(!plan.contents.fixups.iter().any(|fixup| matches!(
                fixup.target,
                ElfProcedureLinkageSemanticTarget::FutureDynamicSection
            )));
        }

        #[test]
        fn import_permutation_and_repeated_calls_preserve_canonical_template_identity() {
            let forward =
                plan_elf_procedure_linkage_templates(linkage(TargetProfile::LinuxX64, &IMPORTS))
                    .unwrap();
            let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
            let reverse = plan_elf_procedure_linkage_templates(linkage(
                TargetProfile::LinuxX64,
                &reverse_imports,
            ))
            .unwrap();
            let arm =
                plan_elf_procedure_linkage_templates(linkage(TargetProfile::LinuxArm64, &IMPORTS))
                    .unwrap();

            assert_eq!(
                forward.non_authoritative_template_compatibility_fingerprint(),
                reverse.non_authoritative_template_compatibility_fingerprint()
            );
            assert_eq!(forward.contents, reverse.contents);
            assert_ne!(
                forward.non_authoritative_template_compatibility_fingerprint(),
                arm.non_authoritative_template_compatibility_fingerprint()
            );
            assert_eq!(forward.linkage().logical_slot_count(), 3);
            assert_eq!(forward.linkage().direct_call_site_count(), 4);
        }

        #[test]
        fn independent_replay_rejects_every_template_fixup_constraint_and_identity_corruption() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|candidate| {
                    candidate.contents.policy =
                        ElfProcedureLinkageTemplatePolicy::Aarch64StandardLazy
                }),
                Box::new(|candidate| candidate.contents.bytes.plt[0] ^= 1),
                Box::new(|candidate| candidate.contents.bytes.plt[2] = 1),
                Box::new(|candidate| {
                    candidate.contents.bytes.plt.pop();
                }),
                Box::new(|candidate| candidate.contents.bytes.plt.push(0)),
                Box::new(|candidate| candidate.contents.bytes.got_plt[0] = 1),
                Box::new(|candidate| {
                    candidate.contents.bytes.got_plt.pop();
                }),
                Box::new(|candidate| candidate.contents.bytes.got_plt.push(0)),
                Box::new(|candidate| candidate.contents.bytes.rela_plt[0] = 1),
                Box::new(|candidate| candidate.contents.bytes.rela_plt[8] ^= 1),
                Box::new(|candidate| candidate.contents.bytes.rela_plt[16] = 1),
                Box::new(|candidate| {
                    candidate.contents.bytes.rela_plt.pop();
                }),
                Box::new(|candidate| candidate.contents.bytes.rela_plt.push(0)),
                Box::new(|candidate| {
                    candidate.contents.fixups.pop();
                }),
                Box::new(|candidate| candidate.contents.fixups.push(candidate.contents.fixups[0])),
                Box::new(|candidate| candidate.contents.fixups.swap(0, 1)),
                Box::new(|candidate| {
                    candidate.contents.fixups[0].storage = ElfProcedureLinkageFixupStorage::RelaPlt
                }),
                Box::new(|candidate| candidate.contents.fixups[0].byte_offset = usize::MAX),
                Box::new(|candidate| candidate.contents.fixups[0].byte_width = 8),
                Box::new(|candidate| candidate.contents.fixups[0].mutable_mask ^= 1),
                Box::new(|candidate| {
                    candidate.contents.fixups[0].kind = ElfProcedureLinkageFixupKind::Aarch64Page21
                }),
                Box::new(|candidate| {
                    candidate.contents.fixups[0].target =
                        ElfProcedureLinkageSemanticTarget::GotPltSlot {
                            logical_ordinal: u32::MAX,
                        }
                }),
                Box::new(|candidate| {
                    candidate.contents.constraints.pop();
                }),
                Box::new(|candidate| candidate.contents.constraints[0].fixup_ordinal = u32::MAX),
                Box::new(|candidate| {
                    candidate.contents.constraints[0].kind =
                        ElfProcedureLinkagePlacementConstraintKind::Aarch64Branch26
                }),
                Box::new(|candidate| {
                    candidate.non_authoritative_template_compatibility_fingerprint ^= 1
                }),
            ];

            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxX64);
                let expected_identity = candidate
                    .linkage
                    .non_authoritative_linkage_compatibility_fingerprint();
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("corrupt linkage template candidate must reject");
                assert_eq!(
                    error
                        .candidate
                        .linkage
                        .non_authoritative_linkage_compatibility_fingerprint(),
                    expected_identity,
                    "template rejection retains exact linkage custody",
                );
            }
        }

        #[test]
        fn aarch64_opcode_placeholder_and_reserved_header_corruption_rejects() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|candidate| candidate.contents.bytes.plt[0] ^= 1),
                Box::new(|candidate| candidate.contents.bytes.plt[4] |= 0x20),
                Box::new(|candidate| candidate.contents.bytes.plt[8] |= 0x04),
                Box::new(|candidate| candidate.contents.bytes.plt[12] |= 0x04),
                Box::new(|candidate| candidate.contents.bytes.plt[32] |= 0x20),
                Box::new(|candidate| candidate.contents.bytes.got_plt[0] = 1),
                Box::new(|candidate| candidate.contents.bytes.got_plt[8] = 1),
                Box::new(|candidate| candidate.contents.bytes.got_plt[16] = 1),
                Box::new(|candidate| {
                    candidate.contents.fixups[0].target =
                        ElfProcedureLinkageSemanticTarget::FutureDynamicSection
                }),
            ];
            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxArm64);
                let expected_identity = candidate
                    .linkage
                    .non_authoritative_linkage_compatibility_fingerprint();
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("corrupt AArch64 template must reject");
                assert_eq!(
                    error
                        .candidate
                        .linkage
                        .non_authoritative_linkage_compatibility_fingerprint(),
                    expected_identity
                );
            }
        }

        #[test]
        fn malformed_lengths_offsets_masks_and_arithmetic_reject_without_panicking() {
            assert!(checked_sum(usize::MAX, 1, "sum").is_err());
            assert!(checked_product(usize::MAX, 2, "product").is_err());
            assert!(checked_u32(usize::MAX, "word").is_err());
            assert!(read_u32(&[], usize::MAX, "u32").is_err());
            assert!(read_u64(&[0; 7], 0, "u64").is_err());
            assert!(read_field(&[0; 8], 0, 3, "test fixup field").is_err());

            let overflowing = ElfProcedureLinkageFixup {
                storage: ElfProcedureLinkageFixupStorage::Plt,
                byte_offset: usize::MAX,
                byte_width: 8,
                mutable_mask: u64::MAX,
                kind: ElfProcedureLinkageFixupKind::Absolute64,
                target: ElfProcedureLinkageSemanticTarget::PltHeader,
            };
            assert!(validate_fixup_coverage(&[overflowing, overflowing]).is_err());
        }
    }

    #[cfg(test)]
    pub(crate) use template_plans::{ElfProcedureLinkageFixup, ElfProcedureLinkageTemplatePolicy};
    pub(crate) use template_plans::{
        ElfProcedureLinkageFixupKind, ElfProcedureLinkageFixupStorage,
        ElfProcedureLinkagePlacementConstraint, ElfProcedureLinkagePlacementConstraintKind,
        ElfProcedureLinkageSemanticTarget,
    };

    pub(crate) use template_plans::{
        ElfProcedureLinkageTemplatePlanningError, ValidatedElfProcedureLinkageTemplatePlan,
    };

    use crate::dynamic_executable::procedure_linkage::dynamic_import_relocations::{ValidatedElfProcedureLinkageRelocationPlan};
    use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::compatibility_fingerprint::non_authoritative_template_compatibility_fingerprint;
    use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::candidates::{Candidate, derive_contents};
    use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::template_validation::validate_candidate;
    use super::super::checked;

    const ELF64_GOT_WORD_SIZE: usize = 8;

    const X86_PLT_HEADER_SIZE: usize = 16;

    const X86_PLT_ENTRY_SIZE: usize = 16;

    const AARCH64_PLT_HEADER_SIZE: usize = 32;

    const AARCH64_PLT_ENTRY_SIZE: usize = 16;

    const GOT_PLT_HEADER_WORDS: usize = 3;

    const X86_PLT_HEADER: [u8; X86_PLT_HEADER_SIZE] = [
        0xff, 0x35, 0, 0, 0, 0, // pushq GOT[1](%rip)
        0xff, 0x25, 0, 0, 0, 0, // jmpq *GOT[2](%rip)
        0x0f, 0x1f, 0x40, 0x00, // nopl 0(%rax)
    ];

    const AARCH64_PLT_HEADER: [u8; AARCH64_PLT_HEADER_SIZE] = [
        0xf0, 0x7b, 0xbf, 0xa9, // stp x16, x30, [sp,#-16]!
        0x10, 0x00, 0x00, 0x90, // adrp x16, .got.plt[2]
        0x11, 0x02, 0x40, 0xf9, // ldr x17, [x16, :lo12:.got.plt[2]]
        0x10, 0x02, 0x00, 0x91, // add x16, x16, :lo12:.got.plt[2]
        0x20, 0x02, 0x1f, 0xd6, // br x17
        0x1f, 0x20, 0x03, 0xd5, // nop
        0x1f, 0x20, 0x03, 0xd5, // nop
        0x1f, 0x20, 0x03, 0xd5, // nop
    ];

    const AARCH64_PLT_ENTRY: [u8; AARCH64_PLT_ENTRY_SIZE] = [
        0x10, 0x00, 0x00, 0x90, // adrp x16, .got.plt[N + 3]
        0x11, 0x02, 0x40, 0xf9, // ldr x17, [x16, :lo12:.got.plt[N + 3]]
        0x10, 0x02, 0x00, 0x91, // add x16, x16, :lo12:.got.plt[N + 3]
        0x20, 0x02, 0x1f, 0xd6, // br x17
    ];

    pub(crate) fn plan_elf_procedure_linkage_templates(
        linkage: ValidatedElfProcedureLinkageRelocationPlan,
    ) -> Result<
        ValidatedElfProcedureLinkageTemplatePlan,
        Box<ElfProcedureLinkageTemplatePlanningError>,
    > {
        checked::plan_stage!(
            plan: linkage,
            derive: derive_contents,
            contents: contents,
            fingerprint: non_authoritative_template_compatibility_fingerprint,
            error: ElfProcedureLinkageTemplatePlanningError,
        )
    }
}
