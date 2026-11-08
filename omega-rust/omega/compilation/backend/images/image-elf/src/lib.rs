/*
We emit 64-bit ELF static executables (`emit_elf_executable`), 22-stage
dynamically linked executables (`emit_elf_dynamic_executable`), and `ET_DYN`
position-independent shared components (`emit_elf_shared_component`) for Linux
x86-64 (`EM_X86_64 = 62`) and AArch64 (`EM_AARCH64 = 183`). Static executables
emit a compact 64-byte ELF header and two program headers (`PT_LOAD` for
`.text` and `.data`/`.bss` at `IMAGE_BASE = 0x400000`) with zero section
headers, failing closed before image mutation if any referenced import exists.

Dynamically linked executables run a linear 22-stage pipeline grouped into six
phase modules (`import_sections`, `procedure_linkage`, `dynamic_table`,
`section_headers`, `load_placement`, `file_assembly`), where each stage
consumes the previous stage's `ValidatedElf*` carrier by value and returns
either the next sealed carrier or a stage-specific rejection carrier retaining
the exact failing owner. We keep three distinct page constants explicit:
`PAGE_SIZE` (`0x1000`) for static segments, `DYNAMIC_MAX_PAGE_SIZE` (`0x1_0000`)
for dynamic segments portable across 64 KiB AArch64 translation granules, and
`AARCH64_RELOCATION_PAGE_SIZE` (`0x1000`) for 4 KiB ADRP page arithmetic.
*/

/*
Representing the dynamic ELF pipeline as 22 distinct `ValidatedElf*` carrier
types rather than a single mutable builder struct makes stage ordering a type
invariant: no stage can be invoked out of order or on partially validated
state. Likewise, refusing referenced imports outright in `emit_elf_executable`
prevents producing a partially linked static binary that would fault only when
control reaches an unbound call site at runtime.
*/

/*
`image` supplies `FinalImage`, `apply_relocations`, and region placement;
`image-emission` invokes `emit_elf_executable`, `emit_elf_dynamic_executable`,
and `emit_elf_shared_component`. Because `emit_elf_dynamic_executable` binds
intermediate stage carriers via inferred `let` bindings, those `ValidatedElf*`
types remain load-bearing even when their type names are not spelled outside
their defining phase module.
*/

mod shared_component;
pub use shared_component::{
    ElfComponentExport, ElfSharedComponentEmissionError, ElfSharedComponentImage,
    emit_elf_shared_component,
};

mod bytes {
    use diagnostics::Diagnostic;
    pub(crate) use image::codec_bytes::{write_u16, write_u32, write_u64};

    pub(crate) fn name_at<'a>(
        bytes: &'a [u8],
        offset: u32,
        context: &'static str,
    ) -> Result<&'a [u8], Diagnostic> {
        let tail = bytes
            .get(offset as usize..)
            .ok_or_else(|| Diagnostic::error(format!("{context} is outside its name table")))?;
        let terminator = tail
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(|| Diagnostic::error(format!("{context} is not NUL-terminated")))?;
        Ok(&tail[..terminator])
    }

    pub(crate) fn require_name(
        bytes: &[u8],
        offset: u32,
        expected: &[u8],
        context: &'static str,
    ) -> Result<(), Diagnostic> {
        if name_at(bytes, offset, context)? == expected {
            Ok(())
        } else {
            Err(Diagnostic::error(format!(
                "{context} does not select its exact name"
            )))
        }
    }
}

mod constants {
    pub(crate) const ELF_HEADER_SIZE: usize = 64;
    pub(crate) const PROGRAM_HEADER_SIZE: usize = 56;
    pub(crate) const PROGRAM_HEADER_COUNT: usize = 2;
    pub(crate) const IMAGE_BASE: u64 = 0x400000;
    pub(crate) const PAGE_SIZE: usize = 0x1000;

    pub(crate) const SECTION_COUNT: usize = 14;
    pub(crate) const ELF64_SECTION_HEADER_SIZE: usize = 64;
    pub(crate) const ELF64_DYNAMIC_ROW_SIZE: usize = 16;
    pub(crate) const ELF64_DYNAMIC_VALUE_OFFSET: usize = 8;
    pub(crate) const ELF64_DYNAMIC_VALUE_SIZE: u8 = 8;
    pub(crate) const ELF64_RELA_SIZE: usize = 24;
    pub(crate) const SHF_WRITE: u64 = 0x1;
    pub(crate) const SHF_ALLOC: u64 = 0x2;
    pub(crate) const SHF_EXECINSTR: u64 = 0x4;
}

mod dynamic_executable;
pub(crate) mod dynamic_table;
pub(crate) mod file_assembly;
pub(crate) mod import_sections;
pub(crate) mod load_placement;
pub(crate) mod procedure_linkage;
pub(crate) mod section_headers;
pub(crate) use dynamic_executable::{checked, checked_u64};
mod entry_symbol {
    use diagnostics::Diagnostic;
    use image::FinalImage;

    pub(crate) fn elf_entry_address(
        image: &FinalImage,
        text_address: u64,
    ) -> Result<u64, Diagnostic> {
        Ok(text_address + image::final_image_entry_symbol(image, "ELF")?.offset as u64)
    }

    #[cfg(test)]
    mod tests {}
}

mod imports;
mod static_executable;

pub use dynamic_executable::{ElfDynamicExecutableEmissionError, emit_elf_dynamic_executable};
pub use static_executable::emit_elf_executable;

pub use dynamic_executable::file_assembly::assembled_dynamic_file::ValidatedElfDynamicExecutable;

pub(crate) use dynamic_executable::file_assembly::resolved_procedure_linkage::ElfAppliedProcedureLinkageKind;

pub(crate) use dynamic_executable::load_placement::load_layout::{
    ElfPlacedDynamicSectionKind, ElfSectionPlacementResolutionKind,
};

pub(crate) use dynamic_executable::load_placement::resolved_dynamic_table::ElfDynamicAddressApplicationTarget;

#[cfg(test)]
pub(crate) use dynamic_executable::dynamic_table::dynamic_table_descriptor::plan_elf_dynamic_table_section_descriptor;
#[cfg(test)]
pub(crate) use dynamic_executable::dynamic_table::dynamic_tag_bytes::serialize_elf_dynamic_table;
#[cfg(test)]
pub(crate) use dynamic_executable::dynamic_table::dynamic_tags::{
    ValidatedElfDynamicTagPlan, plan_elf_dynamic_tags,
};
#[cfg(test)]
pub(crate) use dynamic_executable::file_assembly::dynamic_file_envelope::{
    ValidatedElfDynamicFileEnvelope, serialize_elf_dynamic_file_envelope,
};
#[cfg(test)]
pub(crate) use dynamic_executable::file_assembly::resolved_procedure_linkage::{
    ElfAppliedProcedureLinkageFixup, apply_elf_procedure_linkage_fixups,
};
#[cfg(test)]
pub(crate) use dynamic_executable::import_sections::dynamic_link::{
    PlannedElfDynamicLinkInputs, plan_elf_dynamic_link_inputs,
};
#[cfg(test)]
pub(crate) use dynamic_executable::import_sections::dynamic_section_bytes::serialize_elf_dynamic_sections;
#[cfg(test)]
pub(crate) use dynamic_executable::import_sections::dynamic_section_descriptors::plan_elf_dynamic_section_descriptors;
#[cfg(test)]
pub(crate) use dynamic_executable::import_sections::dynamic_sections::plan_elf_dynamic_sections;
#[cfg(test)]
pub(crate) use dynamic_executable::load_placement::load_layout::{
    ElfLoadProgramHeaderKind, ValidatedElfDynamicLoadLayout, plan_elf_dynamic_load_layout,
};
#[cfg(test)]
pub(crate) use dynamic_executable::load_placement::placed_section_headers::apply_elf_section_header_placements;
#[cfg(test)]
pub(crate) use dynamic_executable::load_placement::resolved_dynamic_table::apply_elf_dynamic_address_fixups;
#[cfg(test)]
pub(crate) use dynamic_executable::procedure_linkage::dynamic_import_relocations::{
    ValidatedElfProcedureLinkageRelocationPlan, plan_elf_procedure_linkage_relocations,
};
#[cfg(test)]
pub(crate) use dynamic_executable::procedure_linkage::dynamic_linkage_descriptors::{
    ValidatedElfProcedureLinkageSectionDescriptorPlan,
    plan_elf_procedure_linkage_section_descriptors,
};
#[cfg(test)]
pub(crate) use dynamic_executable::procedure_linkage::dynamic_linkage_templates::plan_elf_procedure_linkage_templates;
#[cfg(test)]
pub(crate) use dynamic_executable::section_headers::relative_section_layout::plan_elf_relative_section_payload_layout;
#[cfg(test)]
pub(crate) use dynamic_executable::section_headers::section_header_bytes::{
    ValidatedElfSectionHeaderTableTemplate, serialize_elf_section_header_table,
};
#[cfg(test)]
pub(crate) use dynamic_executable::section_headers::section_name_table::{
    ValidatedElfSectionNameTablePlan, plan_elf_section_name_table,
};
#[cfg(test)]
pub(crate) use dynamic_executable::section_headers::section_payload_roster::{
    ValidatedElfIndexedSectionPayloadPlan, plan_elf_indexed_section_payloads,
};
#[cfg(test)]
pub(crate) use dynamic_executable::section_headers::section_roster::plan_elf_dynamic_section_roster;

#[cfg(test)]
pub(crate) mod test_support {
    use crate::{
        ValidatedElfProcedureLinkageRelocationPlan, plan_elf_dynamic_link_inputs,
        plan_elf_dynamic_section_descriptors, plan_elf_dynamic_sections,
        plan_elf_procedure_linkage_relocations, serialize_elf_dynamic_sections,
    };
    use image::FinalImage;
    use target::{NormalizedElfInterpreterPlan, TargetProfile, normalize_elf_interpreter_plan};

    pub(crate) fn interpreter(target: TargetProfile) -> NormalizedElfInterpreterPlan {
        let interpreter_path = match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-x86-64.so.2".as_slice(),
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-aarch64.so.1".as_slice(),
            _ => unreachable!(),
        };
        normalize_elf_interpreter_plan(interpreter_path.to_vec(), target).unwrap()
    }

    pub(crate) fn import_image(
        target: TargetProfile,
        bss_alignment: usize,
        symbol_capacity: usize,
    ) -> FinalImage {
        FinalImage::with_capacity(
            target.native_target(),
            image::FinalImageMemory {
                text: vec![0; 32],
                data: vec![0x5a; 13],
                bss_size: 23,
                bss_alignment,
            },
            arena::Handle::invalid(),
            symbol_capacity,
            1,
            1,
        )
    }

    pub(crate) fn text_entry(image: &mut FinalImage, offset: usize) {
        use image::{FinalImageSection, FinalImageSymbol};
        use object_file::SymbolKind;

        let entry = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "_start".to_owned(),
            section: FinalImageSection::Text,
            offset,
            size: 4,
            kind: SymbolKind::Function,
        });
        image.symbol_table.entry_symbol = entry;
    }

    pub(crate) fn relocation_image(target: target::NativeTarget, text_size: usize) -> FinalImage {
        FinalImage::with_capacity(
            target,
            image::FinalImageMemory {
                text: vec![0; text_size],
                ..image::FinalImageMemory::default()
            },
            arena::Handle::invalid(),
            1,
            1,
            2,
        )
    }

    pub(crate) fn resolved_linkage(
        image: FinalImage,
        target: TargetProfile,
    ) -> ValidatedElfProcedureLinkageRelocationPlan {
        let inputs = plan_elf_dynamic_link_inputs(image, interpreter(target)).unwrap();
        let sections = plan_elf_dynamic_sections(inputs).unwrap();
        let payloads = serialize_elf_dynamic_sections(sections).unwrap();
        let descriptors = plan_elf_dynamic_section_descriptors(payloads).unwrap();
        plan_elf_procedure_linkage_relocations(descriptors).unwrap()
    }
    pub(crate) fn import_with_text_relocations(
        image: &mut FinalImage,
        name: &str,
        import: image::FinalImageImportPlan,
        offsets: &[usize],
    ) -> image::FinalImageSymbolHandle {
        use image::{FinalImageImport, FinalImageRelocation, FinalImageSection, FinalImageSymbol};
        use object_file::{RelocationKind, SymbolKind};

        let imported = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: name.into(),
            section: FinalImageSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: imported,
            import,
        });
        for &offset in offsets {
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    section: FinalImageSection::Text,
                    offset,
                    byte_width: 4,
                    symbol_handle: imported,
                    addend: 0,
                    kind: RelocationKind::X86_64Relative32,
                });
        }
        imported
    }

    pub(crate) type DynamicImportFixture = ImportFixture;

    pub(crate) fn dynamic_import_inputs(
        target: TargetProfile,
        imports: &[DynamicImportFixture],
        symbol_prefix: &'static str,
        locator_expect: &'static str,
        interpreter_expect: &'static str,
    ) -> crate::PlannedElfDynamicLinkInputs {
        use arena::Handle;
        use image::FinalImageMemory;

        let relocation_count = imports
            .iter()
            .map(|fixture| fixture.instruction_offsets.len())
            .sum();
        let mut image = FinalImage::with_capacity(
            target.native_target(),
            FinalImageMemory {
                text: vec![0; 64],
                ..FinalImageMemory::default()
            },
            Handle::invalid(),
            imports.len(),
            imports.len(),
            relocation_count,
        );
        for (index, fixture) in imports.iter().enumerate() {
            let symbol_handle = insert_import_fixture(
                &mut image,
                symbol_prefix,
                index,
                fixture.object,
                fixture.symbol,
                fixture.version,
                target,
                locator_expect,
            );
            insert_text_call_relocations(
                &mut image,
                target,
                fixture
                    .instruction_offsets
                    .iter()
                    .map(|offset| (*offset, symbol_handle)),
            );
        }
        let interpreter_path = match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2".as_slice(),
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1".as_slice(),
            _ => unreachable!("dynamic-table fixtures use a Linux target"),
        };
        let interpreter = normalize_elf_interpreter_plan(interpreter_path.to_vec(), target)
            .expect(interpreter_expect);
        plan_elf_dynamic_link_inputs(image, interpreter).expect("valid dynamic-link inputs")
    }

    pub(crate) fn dynamic_tag_plan(
        inputs: crate::PlannedElfDynamicLinkInputs,
    ) -> crate::ValidatedElfDynamicTagPlan {
        use crate::{
            plan_elf_dynamic_section_descriptors, plan_elf_dynamic_sections, plan_elf_dynamic_tags,
            plan_elf_procedure_linkage_relocations, plan_elf_procedure_linkage_section_descriptors,
            plan_elf_procedure_linkage_templates, serialize_elf_dynamic_sections,
        };
        let sections = plan_elf_dynamic_sections(inputs).expect("valid dynamic sections");
        let payloads = serialize_elf_dynamic_sections(sections).expect("valid dynamic payloads");
        let base = plan_elf_dynamic_section_descriptors(payloads).expect("valid base descriptors");
        let linkage =
            plan_elf_procedure_linkage_relocations(base).expect("valid procedure linkage");
        let templates =
            plan_elf_procedure_linkage_templates(linkage).expect("valid linkage templates");
        let descriptors = plan_elf_procedure_linkage_section_descriptors(templates)
            .expect("valid linkage descriptors");
        plan_elf_dynamic_tags(descriptors).expect("valid semantic dynamic tags")
    }

    #[derive(Clone, Copy)]
    pub(crate) struct SingleSiteImportFixture {
        pub(crate) object: &'static [u8],
        pub(crate) symbol: &'static [u8],
        pub(crate) version: &'static [u8],
    }

    pub(crate) const SINGLE_SITE_IMPORTS: [SingleSiteImportFixture; 4] = [
        SingleSiteImportFixture {
            object: b"libalpha\xff.so.1",
            symbol: b"alpha\xfe",
            version: b"OMEGA_1\xfd",
        },
        SingleSiteImportFixture {
            object: b"libalpha\xff.so.1",
            symbol: b"beta",
            version: b"OMEGA_1\xfd",
        },
        SingleSiteImportFixture {
            object: b"libalpha\xff.so.1",
            symbol: b"alpha\xfe",
            version: b"OMEGA_2",
        },
        SingleSiteImportFixture {
            object: b"libbeta.so.2",
            symbol: b"gamma",
            version: b"OMEGA_1\xfd",
        },
    ];

    pub(crate) fn single_site_import_inputs(
        target: TargetProfile,
        imports: &[SingleSiteImportFixture],
        symbol_prefix: &str,
        text_size: usize,
        interpreter_path: &[u8],
        locator_expect: &str,
        interpreter_expect: &str,
    ) -> crate::PlannedElfDynamicLinkInputs {
        use arena::Handle;
        use image::{FinalImage, FinalImageMemory, FinalImageRelocation, FinalImageSection};
        use object_file::RelocationKind;
        use target::NativeTarget;

        let native_target = target.native_target();
        let mut image = FinalImage::with_capacity(
            native_target,
            FinalImageMemory {
                text: vec![0; text_size],
                ..FinalImageMemory::default()
            },
            Handle::invalid(),
            imports.len(),
            imports.len(),
            imports.len(),
        );
        for (index, fixture) in imports.iter().enumerate() {
            let symbol_handle = insert_import_fixture(
                &mut image,
                symbol_prefix,
                index,
                fixture.object,
                fixture.symbol,
                fixture.version,
                target,
                locator_expect,
            );
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    section: FinalImageSection::Text,
                    offset: index * 8,
                    byte_width: 4,
                    symbol_handle,
                    addend: 0,
                    kind: if native_target == NativeTarget::linux_arm64() {
                        RelocationKind::Aarch64Branch26
                    } else {
                        RelocationKind::X86_64Relative32
                    },
                });
        }
        let interpreter = normalize_elf_interpreter_plan(interpreter_path.to_vec(), target)
            .expect(interpreter_expect);
        plan_elf_dynamic_link_inputs(image, interpreter).expect("valid dynamic-link preflight")
    }

    pub(crate) fn linkage_descriptors(
        inputs: crate::PlannedElfDynamicLinkInputs,
    ) -> crate::ValidatedElfProcedureLinkageSectionDescriptorPlan {
        let sections = plan_elf_dynamic_sections(inputs).expect("valid dynamic sections");
        let payloads = serialize_elf_dynamic_sections(sections).expect("valid dynamic payloads");
        let base = plan_elf_dynamic_section_descriptors(payloads).expect("valid base descriptors");
        let linkage =
            plan_elf_procedure_linkage_relocations(base).expect("valid procedure linkage");
        let templates =
            crate::plan_elf_procedure_linkage_templates(linkage).expect("valid linkage templates");
        crate::plan_elf_procedure_linkage_section_descriptors(templates)
            .expect("valid linkage descriptors")
    }

    pub(crate) fn dynamic_section_names(
        inputs: crate::PlannedElfDynamicLinkInputs,
    ) -> crate::ValidatedElfSectionNameTablePlan {
        let tags = dynamic_tag_plan(inputs);
        let dynamic = crate::serialize_elf_dynamic_table(tags).expect("valid dynamic table");
        let descriptor = crate::plan_elf_dynamic_table_section_descriptor(dynamic)
            .expect("valid dynamic descriptor");
        crate::plan_elf_section_name_table(descriptor).expect("valid section-name table")
    }

    pub(crate) fn section_header_table(
        inputs: crate::PlannedElfDynamicLinkInputs,
    ) -> crate::ValidatedElfSectionHeaderTableTemplate {
        let names = dynamic_section_names(inputs);
        let roster = crate::plan_elf_dynamic_section_roster(names).expect("valid section roster");
        crate::serialize_elf_section_header_table(roster).expect("valid section headers")
    }

    pub(crate) fn section_header_table_from_linkage(
        linkage: crate::ValidatedElfProcedureLinkageRelocationPlan,
    ) -> crate::ValidatedElfSectionHeaderTableTemplate {
        let templates =
            crate::plan_elf_procedure_linkage_templates(linkage).expect("valid linkage templates");
        let descriptors = crate::plan_elf_procedure_linkage_section_descriptors(templates)
            .expect("valid linkage descriptors");
        let tags = crate::plan_elf_dynamic_tags(descriptors).expect("valid dynamic tags");
        let dynamic = crate::serialize_elf_dynamic_table(tags).expect("valid dynamic table");
        let descriptor = crate::plan_elf_dynamic_table_section_descriptor(dynamic)
            .expect("valid dynamic descriptor");
        let names =
            crate::plan_elf_section_name_table(descriptor).expect("valid section-name table");
        let roster = crate::plan_elf_dynamic_section_roster(names).expect("valid section roster");
        crate::serialize_elf_section_header_table(roster).expect("valid section headers")
    }

    pub(crate) fn import_call_indexed_payloads(
        mut image: image::FinalImage,
        symbol_handle: image::FinalImageSymbolHandle,
        target: TargetProfile,
    ) -> crate::ValidatedElfIndexedSectionPayloadPlan {
        insert_text_call_relocations(&mut image, target, [(0, symbol_handle)]);
        let linkage = resolved_linkage(image, target);
        let templates = crate::plan_elf_procedure_linkage_templates(linkage).unwrap();
        let descriptors = crate::plan_elf_procedure_linkage_section_descriptors(templates).unwrap();
        let tags = crate::plan_elf_dynamic_tags(descriptors).unwrap();
        let dynamic = crate::serialize_elf_dynamic_table(tags).unwrap();
        let descriptor = crate::plan_elf_dynamic_table_section_descriptor(dynamic).unwrap();
        let names = crate::plan_elf_section_name_table(descriptor).unwrap();
        let roster = crate::plan_elf_dynamic_section_roster(names).unwrap();
        let headers = crate::serialize_elf_section_header_table(roster).unwrap();
        crate::plan_elf_indexed_section_payloads(headers).unwrap()
    }

    pub(crate) fn import_call_load_layout(
        image: image::FinalImage,
        symbol_handle: image::FinalImageSymbolHandle,
        target: TargetProfile,
    ) -> crate::ValidatedElfDynamicLoadLayout {
        let payloads = import_call_indexed_payloads(image, symbol_handle, target);
        let relative = crate::plan_elf_relative_section_payload_layout(payloads).unwrap();
        crate::plan_elf_dynamic_load_layout(relative).unwrap()
    }

    pub(crate) fn insert_text_call_relocations(
        image: &mut image::FinalImage,
        target: TargetProfile,
        sites: impl IntoIterator<Item = (usize, image::FinalImageSymbolHandle)>,
    ) {
        use image::{FinalImageRelocation, FinalImageSection};
        use object_file::RelocationKind;
        for (instruction_offset, symbol_handle) in sites {
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
    }

    #[derive(Clone, Copy)]
    pub(crate) struct ImportFixture {
        pub(crate) object: &'static [u8],
        pub(crate) symbol: &'static [u8],
        pub(crate) version: &'static [u8],
        pub(crate) instruction_offsets: &'static [usize],
    }

    pub(crate) const IMPORTS: [ImportFixture; 3] = [
        ImportFixture {
            object: b"liba\xff.so",
            symbol: b"alpha\xfe",
            version: b"V1\xfd",
            instruction_offsets: &[0, 44],
        },
        ImportFixture {
            object: b"liba\xff.so",
            symbol: b"beta",
            version: b"V2",
            instruction_offsets: &[16],
        },
        ImportFixture {
            object: b"libb.so",
            symbol: b"gamma",
            version: b"V1\xfd",
            instruction_offsets: &[28],
        },
    ];

    pub(crate) const PAIR_IMPORTS: [ImportFixture; 2] = [
        ImportFixture {
            object: b"liba\xff.so",
            symbol: b"alpha\xfe",
            version: b"V1\xfd",
            instruction_offsets: &[0, 32],
        },
        ImportFixture {
            object: b"libb.so",
            symbol: b"beta",
            version: b"V2",
            instruction_offsets: &[16],
        },
    ];

    pub(crate) fn versioned_import(
        image: &mut FinalImage,
        name: &str,
        object: &[u8],
        symbol: &[u8],
        version: &[u8],
        target: TargetProfile,
        locator_expect: &str,
    ) -> image::FinalImageSymbolHandle {
        use image::{FinalImageImport, FinalImageImportPlan, FinalImageSection, FinalImageSymbol};
        use object_file::SymbolKind;
        use target::{ForeignLocatorCandidate, normalize_foreign_locator};
        let imported = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: name.into(),
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
                        object: object.to_vec(),
                        symbol: symbol.to_vec(),
                        version: version.to_vec(),
                    },
                    target,
                )
                .expect(locator_expect)
                .into(),
            ),
        });
        imported
    }

    pub(crate) fn insert_import_fixture(
        image: &mut image::FinalImage,
        symbol_prefix: &str,
        index: usize,
        object: &[u8],
        symbol: &[u8],
        version: &[u8],
        target: TargetProfile,
        locator_expect: &str,
    ) -> image::FinalImageSymbolHandle {
        versioned_import(
            image,
            &format!("{symbol_prefix}{index}"),
            object,
            symbol,
            version,
            target,
            locator_expect,
        )
    }

    pub(crate) fn import_fixture_interpreter_path(target: TargetProfile) -> &'static [u8] {
        match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2",
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1",
            _ => unreachable!("import fixture uses a Linux target"),
        }
    }
}
