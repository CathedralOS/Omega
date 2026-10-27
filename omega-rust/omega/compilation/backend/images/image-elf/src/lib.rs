//! ELF process-image and shared-component container emission.
//!
//! `shared_component.rs` owns genuine ET_DYN callable exports, PIC relocation
//! and dynamic import tables. It reuses the import-table producer below while
//! keeping its own original image/export custody; it grants no installation.
//! The static process emitter and staged dynamic-process pipeline follow.
//!
//! `emit_elf_executable` is the whole static lane, and it is deliberately
//! small, refusing any image whose target is not a hosted ELF diagonal. The
//! output has a 64-byte ELF header, exactly two program headers, and NO
//! section headers at all:
//!
//! ```text
//!   0x000000  ELF header (64) + 2 program headers (56 each) = 176 bytes
//!   0x001000  .text   at the first PAGE_SIZE boundary, vaddr 0x401000
//!   0x00N000  .data   at the next boundary, .bss aligned after it
//!   IMAGE_BASE = 0x400000, PAGE_SIZE = 0x1000, PROGRAM_HEADER_COUNT = 2
//! ```
//!
//! The container is architecture-agnostic apart from `e_machine` (62 for
//! x86-64, 183 for AArch64), which is read off the image's own target the same
//! way `apply_relocations` reads its instruction set.
//!
//! That lane FAILS CLOSED the moment it sees a referenced import: any surviving
//! entry from `canonical_referenced_imports` is an immediate error before the
//! image is mutated at all. A statically emitted ELF here is one that needs no
//! loader.
//!
//! Everything else in the crate is the dynamic lane: `emit_elf_dynamic_executable`
//! in `dynamic_executable.rs` drives a linear chain of 22 stages:
//!
//! ```text
//!   plan_elf_dynamic_link_inputs -> plan_elf_dynamic_sections
//!   -> serialize_elf_dynamic_sections -> plan_elf_dynamic_section_descriptors
//!   -> plan_elf_procedure_linkage_relocations
//!   -> plan_elf_procedure_linkage_templates
//!   -> plan_elf_procedure_linkage_section_descriptors
//!   -> plan_elf_dynamic_tags -> serialize_elf_dynamic_table
//!   -> plan_elf_dynamic_table_section_descriptor -> plan_elf_section_name_table
//!   -> plan_elf_dynamic_section_roster -> serialize_elf_section_header_table
//!   -> plan_elf_indexed_section_payloads
//!   -> plan_elf_relative_section_payload_layout -> plan_elf_dynamic_load_layout
//!   -> apply_elf_section_header_placements -> apply_elf_dynamic_address_fixups
//!   -> serialize_elf_dynamic_file_envelope -> apply_elf_procedure_linkage_fixups
//!   -> assemble_elf_dynamic_file -> admit_elf_dynamic_executable
//! ```
//!
//! Each stage takes the previous stage's `ValidatedElf*` carrier as its only
//! input. Not by convention - there is no way to call stage N without holding a
//! value that only stage N-1 can construct.
//!
//! The three page-size constants look redundant and are not. The static lane
//! writes `p_align` of `PAGE_SIZE` (0x1000). The dynamic lane aligns segments to
//! `DYNAMIC_MAX_PAGE_SIZE` (0x1_0000), because AArch64 permits translation
//! granules up to 64 KiB and a segment aligned only to 4 KiB is not portable
//! across them. `AARCH64_RELOCATION_PAGE_SIZE` stays 0x1000 in the same file
//! because ADRP's page is 4 KiB regardless of what the loader maps with. Two of
//! the three are numerically equal and mean different things.

//! One carrier type and one error enum per stage - 22 of each - instead of one
//! `ElfDynamicImage` struct that every stage fills in a little more. The cost is
//! roughly forty types that exist only to be passed once. What it buys is that
//! running the stages out of order, or running one on half-planned input, is not
//! something a caller can express: there is no constructor for stage N's input
//! except stage N-1's success. A mutable shared struct would move all 22
//! ordering constraints into review comments.
//!
//! The static emitter refuses an import rather than emitting an image that might
//! load. A best-effort static link - resolve what we can, leave the rest - would
//! produce a file that runs until it reaches the unbound call, which is the
//! failure mode hardest to attribute back to the compiler.

//! `image-emission/src/dynamic_elf.rs` is the only caller of the dynamic
//! lane's entry; `image` supplies `FinalImage`, `place_executable_regions`
//! and the relocation appliers both lanes use.
//!
//! @Note: do not decide what is dead in this crate by grepping for type names.
//! The chain binds every stage result with an inferred `let` and never spells a
//! carrier type, so 21 of the 22 carriers have zero occurrences of their names
//! anywhere outside this crate while being entirely load-bearing -
//! `ValidatedElfDynamicLoadLayout` among them, which is the return type of a
//! stage the chain calls in production. A scouting pass over this crate
//! called 18 such types dead on exactly that evidence.
//!
//! @Incomplete: the static lane's own error message is out of date and says so
//! in the most misleading possible place - the text a user sees. It claims
//! "no target-owned ELF loader plan carries the exact PT_INTERP bytes". One
//! does: `target::NormalizedElfInterpreterPlan::interpreter_path` carries
//! exactly those bytes, `plan_elf_dynamic_link_inputs` takes that plan as its
//! second argument, and `dynamic_file_envelope.rs` maps
//! `ElfLoadProgramHeaderKind::Interpreter` to `PT_INTERP` (3). The claim in the
//! other direction is stale too: `target/src/elf_loader.rs` says dynamic
//! ELF emission "remains unavailable until a later owner joins this input to the
//! complete dynamic-link structures", and that owner is this crate. Each side
//! documents the other's absence while the other is present. Fix them together
//! or not at all.
//!
//! On disk the crate splits the same way: `static_executable.rs` is the
//! static lane, `dynamic_executable.rs` owns the 22-stage dynamic lane's
//! entry and groups its stages by phase, and `bytes.rs`, `constants.rs`,
//! `entry_symbol.rs` and `imports.rs` hold what both lanes share.

mod shared_component;
pub use shared_component::{
    ElfComponentExport, ElfSharedComponentEmissionError, ElfSharedComponentImage,
    emit_elf_shared_component,
};

mod bytes;
mod constants;
mod dynamic_executable;
mod entry_symbol;
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

    /// The platform dynamic-linker interpreter the ELF fixtures link
    /// against; the tests only exercise the Linux profiles.
    pub(crate) fn interpreter(target: TargetProfile) -> NormalizedElfInterpreterPlan {
        let interpreter_path = match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-x86-64.so.2".as_slice(),
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-aarch64.so.1".as_slice(),
            _ => unreachable!(),
        };
        normalize_elf_interpreter_plan(interpreter_path.to_vec(), target).unwrap()
    }

    /// The shared dynamic-link staging chain every dynamic-executable test
    /// drives through section planning to the procedure-linkage relocation
    /// plan.
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
    /// One imported symbol plus its text-relative relocation sites — the
    /// shape every import-driven fixture stages before planning.
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

    /// One dynamic-executable import entry shared by the dynamic-table
    /// fixtures — the same shape as `ImportFixture`.
    pub(crate) type DynamicImportFixture = ImportFixture;

    /// The imported-symbol image and Linux interpreter the dynamic-table
    /// fixtures stage before their own descriptor or tag assertions.
    pub(crate) fn dynamic_import_inputs(
        target: TargetProfile,
        imports: &[DynamicImportFixture],
        symbol_prefix: &'static str,
        locator_expect: &'static str,
        interpreter_expect: &'static str,
    ) -> crate::PlannedElfDynamicLinkInputs {
        use arena::Handle;
        use image::{FinalImageMemory, FinalImageRelocation, FinalImageSection};
        use object_file::RelocationKind;

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
                    _ => unreachable!("dynamic-table fixtures use a Linux target"),
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
        let interpreter_path = match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2".as_slice(),
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1".as_slice(),
            _ => unreachable!("dynamic-table fixtures use a Linux target"),
        };
        let interpreter = normalize_elf_interpreter_plan(interpreter_path.to_vec(), target)
            .expect(interpreter_expect);
        plan_elf_dynamic_link_inputs(image, interpreter).expect("valid dynamic-link inputs")
    }

    /// The section/descriptor/linkage/tag staging chain the dynamic-table
    /// fixtures replay after building their import image.
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

    /// An import entry whose only call site lands at `index * 8` in `.text`
    /// — the convention the import-section fixtures use.
    #[derive(Clone, Copy)]
    pub(crate) struct SingleSiteImportFixture {
        pub(crate) object: &'static [u8],
        pub(crate) symbol: &'static [u8],
        pub(crate) version: &'static [u8],
    }

    /// Four imports across two objects with a shared `OMEGA_1`/`OMEGA_2`
    /// version split — the multi-entry roster the import-section tests share.
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

    /// The dynamic-link preflight over `imports` where each fixture gets one
    /// relocation at `index * 8` — the shared image-build prelude for the
    /// import-section tests.
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

    /// The procedure-linkage section descriptor plan — the staging
    /// point the dynamic-tag fixtures build on.
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

    /// The section-name table at the end of the staged dynamic-section
    /// chain — the shared prefix for the section-header fixtures.
    pub(crate) fn dynamic_section_names(
        inputs: crate::PlannedElfDynamicLinkInputs,
    ) -> crate::ValidatedElfSectionNameTablePlan {
        let tags = dynamic_tag_plan(inputs);
        let dynamic = crate::serialize_elf_dynamic_table(tags).expect("valid dynamic table");
        let descriptor = crate::plan_elf_dynamic_table_section_descriptor(dynamic)
            .expect("valid dynamic descriptor");
        crate::plan_elf_section_name_table(descriptor).expect("valid section-name table")
    }

    /// The serialized section-header table over `dynamic_section_names` —
    /// the staging point for the indexed-payload and layout fixtures.
    pub(crate) fn section_header_table(
        inputs: crate::PlannedElfDynamicLinkInputs,
    ) -> crate::ValidatedElfSectionHeaderTableTemplate {
        let names = dynamic_section_names(inputs);
        let roster = crate::plan_elf_dynamic_section_roster(names).expect("valid section roster");
        crate::serialize_elf_section_header_table(roster).expect("valid section headers")
    }

    /// The serialized section-header table staged one rung deeper than
    /// `resolved_linkage` — file-assembly fixtures run the eight-stage
    /// dynamic tail from a resolved procedure linkage.
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

    /// The import-call fixture staged through indexed section payloads — the
    /// shared prefix for the load-layout and section-header assertions.
    pub(crate) fn import_call_indexed_payloads(
        mut image: image::FinalImage,
        symbol_handle: image::FinalImageSymbolHandle,
        target: TargetProfile,
    ) -> crate::ValidatedElfIndexedSectionPayloadPlan {
        use image::{FinalImageRelocation, FinalImageSection};
        use object_file::RelocationKind;
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
                symbol_handle,
                addend: 0,
                kind,
            });
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

    /// Extend the import-call fixture through relative payload layout to the
    /// validated load layout — the point where the load-placement and
    /// section-header tests diverge.
    pub(crate) fn import_call_load_layout(
        image: image::FinalImage,
        symbol_handle: image::FinalImageSymbolHandle,
        target: TargetProfile,
    ) -> crate::ValidatedElfDynamicLoadLayout {
        let payloads = import_call_indexed_payloads(image, symbol_handle, target);
        let relative = crate::plan_elf_relative_section_payload_layout(payloads).unwrap();
        crate::plan_elf_dynamic_load_layout(relative).unwrap()
    }

    /// Insert one text call-site relocation per site: the x86_64 sites patch
    /// a `call rel32` and relocate the operand, the arm64 sites patch a `bl`
    /// and relocate the instruction itself.
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

    /// One procedure-linkage import for the multi-import fixtures: the raw
    /// library coordinates plus the text offsets that relocate against it.
    /// The byte values are deliberately unprintable so the tests exercise
    /// byte-exact identity rather than display paths.
    #[derive(Clone, Copy)]
    pub(crate) struct ImportFixture {
        pub(crate) object: &'static [u8],
        pub(crate) symbol: &'static [u8],
        pub(crate) version: &'static [u8],
        pub(crate) instruction_offsets: &'static [usize],
    }

    /// The three-import procedure-linkage fixture shared by the relocation
    /// and template tests: two imports in one library plus a third in
    /// another.
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

    /// The two-import cross-library fixture the section and table tests
    /// share: one relocation site pair per library.
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

    /// One imported symbol plus its normalized ELF-versioned locator — the
    /// shape every versioned-import fixture stages.
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

    /// Insert an index-named imported symbol and its normalized ELF-versioned
    /// locator — the shared prelude of every multi-import fixture.
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

    /// The unprintable-byte interpreter path the multi-import fixtures link
    /// against.
    pub(crate) fn import_fixture_interpreter_path(target: TargetProfile) -> &'static [u8] {
        match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2",
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1",
            _ => unreachable!("import fixture uses a Linux target"),
        }
    }
}
