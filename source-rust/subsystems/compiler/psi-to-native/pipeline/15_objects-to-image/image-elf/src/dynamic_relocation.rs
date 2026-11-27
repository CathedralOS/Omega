/*
Step 2 of the dynamic ELF pipeline: classify import relocations into PLT call
sites and `.rela.dyn` data slots, encode `.plt`, `.got.plt`, `.rela.plt`,
`.rela.dyn`, and `.dynamic`, apply image-local relocations on `.text` and
`.data`, and resolve all procedure-linkage fixups against Step 1 addresses.
*/

use crate::bytes::write_u64;
use crate::constants::{ELF64_GOT_WORD_SIZE, ELF64_RELA_SIZE, GOT_PLT_HEADER_WORDS};
use crate::dynamic_layout::{
    DynamicLayoutPlan, DynamicSectionIndex, checked_product, checked_sum, checked_sum_u64,
    checked_u32, checked_u64, require,
};
use crate::dynamic_tables::{DynamicTableAddresses, ElfDynamicImportBinding, encode_dynamic_table};
use crate::imports::{ElfImportLocator, ElfImportRequest};
use diagnostics::Diagnostic;
use image::{
    FinalDataRegion, FinalDataRegionOrigin, FinalImage, FinalImageRelocation,
    FinalImageRelocationTable, FinalImageSection, PlacedDataRegionInventory, apply_relocations,
    final_image_imports_symbol, place_data_extent,
};
use machine_code::import_code::{EncodedImportBody, ImportOperandKind, ImportOperandTarget};
use object_file::RelocationKind;
use target::TargetProfile;

const R_X86_64_JUMP_SLOT: u32 = 7;
const R_AARCH64_JUMP_SLOT: u32 = 1026;
const R_X86_64_64: u32 = 1;
const R_AARCH64_ABS64: u32 = 257;
const X86_PLT_LAZY_TAIL_OFFSET: u64 = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DirectImportCallSite {
    pub(crate) instruction_offset: usize,
    pub(crate) relocation_offset: usize,
    pub(crate) byte_width: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogicalPltSlot {
    pub(crate) logical_ordinal: u32,
    pub(crate) request_index: usize,
    pub(crate) dynamic_symbol_index: u32,
    pub(crate) call_sites: Vec<DirectImportCallSite>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GeneralImportRelocation {
    pub(crate) dynamic_symbol_index: u32,
    pub(crate) relocation_type: u32,
    pub(crate) source_section: FinalImageSection,
    pub(crate) source_offset: usize,
    pub(crate) addend: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LinkageClassification {
    pub(crate) slots: Vec<LogicalPltSlot>,
    pub(crate) jump_slot_type: u32,
    pub(crate) general_relocations: Vec<GeneralImportRelocation>,
}

pub(crate) struct ResolvedDynamicRelocations {
    pub(crate) text_bytes: Vec<u8>,
    pub(crate) data_bytes: Vec<u8>,
    pub(crate) plt_bytes: Vec<u8>,
    pub(crate) got_plt_bytes: Vec<u8>,
    pub(crate) rela_plt_bytes: Vec<u8>,
    pub(crate) rela_dyn_bytes: Vec<u8>,
    pub(crate) dynamic_bytes: Vec<u8>,
    pub(crate) import_data_regions: PlacedDataRegionInventory,
}

#[derive(Clone, Copy)]
enum FixupStorage {
    SourceText,
    Plt,
    GotPlt,
    RelaPlt,
    RelaDyn,
}

#[derive(Clone, Copy)]
enum FixupKind {
    X86PcRelative32,
    Aarch64Page21,
    Aarch64Load64Low12,
    Aarch64AddLow12,
    Aarch64Branch26,
    Absolute64,
}

#[derive(Clone, Copy)]
enum FixupTarget {
    DynamicSection,
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
    ImageSection {
        section: FinalImageSection,
        byte_offset: usize,
    },
}

/// Classify all import relocations into direct PLT call sites or general `.rela.dyn` slots.
pub(crate) fn classify_import_relocations(
    target: TargetProfile,
    image: &FinalImage,
    imports: &[ElfImportRequest],
    bindings: &[ElfDynamicImportBinding],
) -> Result<LinkageClassification, Diagnostic> {
    require(
        bindings.len() == imports.len(),
        "ELF dynamic bindings do not cover every canonical import request",
    )?;
    let (expected_call_kind, jump_slot_type, general_type) = match target {
        TargetProfile::LinuxX64 => (
            RelocationKind::X86_64Relative32,
            R_X86_64_JUMP_SLOT,
            R_X86_64_64,
        ),
        TargetProfile::LinuxArm64 => (
            RelocationKind::Aarch64Branch26,
            R_AARCH64_JUMP_SLOT,
            R_AARCH64_ABS64,
        ),
        _ => {
            return Err(Diagnostic::error(
                "ELF procedure linkage requires an exact Linux x86-64 or AArch64 profile",
            ));
        }
    };

    let mut slots = Vec::with_capacity(bindings.len());
    let mut general_relocations = Vec::new();
    let mut all_call_sites = Vec::new();

    for (index, binding) in bindings.iter().enumerate() {
        let logical_ordinal = checked_u32(index, "logical procedure-linkage slot ordinal")?;
        let request = imports.get(binding.request_index).ok_or_else(|| {
            Diagnostic::error("ELF dynamic binding request index exceeds canonical imports")
        })?;
        require(
            !request.relocations.is_empty(),
            "ELF procedure-linkage import has no retained relocations",
        )?;

        let mut call_sites = Vec::new();
        for relocation in &request.relocations {
            if relocation.section == FinalImageSection::Text
                && relocation.kind == expected_call_kind
                && relocation.byte_width == 4
                && relocation.addend == 0
            {
                let site = validate_call_site(target, &image.memory.text, relocation)?;
                all_call_sites.push(site.clone());
                call_sites.push(site);
            } else {
                let general = validate_general_slot(
                    image.memory.data.len(),
                    general_type,
                    binding.dynamic_symbol_index,
                    relocation,
                )?;
                general_relocations.push(general);
            }
        }
        call_sites.sort_unstable_by_key(|site| (site.instruction_offset, site.relocation_offset));
        slots.push(LogicalPltSlot {
            logical_ordinal,
            request_index: binding.request_index,
            dynamic_symbol_index: binding.dynamic_symbol_index,
            call_sites,
        });
    }

    all_call_sites.sort_unstable_by_key(|site| (site.instruction_offset, site.relocation_offset));
    for pair in all_call_sites.windows(2) {
        let left_end = checked_sum(
            pair[0].relocation_offset,
            pair[0].byte_width,
            "call site end",
        )?;
        require(
            left_end <= pair[1].instruction_offset,
            "ELF direct import call sites overlap",
        )?;
    }

    general_relocations.sort_unstable_by_key(|reloc| reloc.source_offset);
    for pair in general_relocations.windows(2) {
        let left_end = checked_sum(pair[0].source_offset, 8, "general relocation slot end")?;
        require(
            left_end <= pair[1].source_offset,
            "ELF general dynamic relocations overlap",
        )?;
    }

    Ok(LinkageClassification {
        slots,
        jump_slot_type,
        general_relocations,
    })
}

fn validate_call_site(
    target: TargetProfile,
    text: &[u8],
    relocation: &FinalImageRelocation,
) -> Result<DirectImportCallSite, Diagnostic> {
    let end = checked_sum(
        relocation.offset,
        relocation.byte_width,
        "ELF dynamic import call relocation",
    )?;
    if end > text.len() {
        return Err(Diagnostic::error(
            "ELF dynamic import call relocation exceeds .text",
        ));
    }
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
        _ => unreachable!(),
    };
    Ok(DirectImportCallSite {
        instruction_offset,
        relocation_offset: relocation.offset,
        byte_width: relocation.byte_width,
    })
}

fn validate_general_slot(
    data_len: usize,
    general_type: u32,
    dynamic_symbol_index: u32,
    relocation: &FinalImageRelocation,
) -> Result<GeneralImportRelocation, Diagnostic> {
    require(
        relocation.section == FinalImageSection::Data
            && relocation.kind == RelocationKind::Absolute64
            && relocation.byte_width == 8
            && relocation.offset.is_multiple_of(8),
        "ELF dynamic import requires a non-procedure or malformed source relocation",
    )?;
    let end = checked_sum(relocation.offset, 8, "general import relocation end")?;
    if end > data_len {
        return Err(Diagnostic::error(
            "ELF dynamic import data slot exceeds .data",
        ));
    }
    Ok(GeneralImportRelocation {
        dynamic_symbol_index,
        relocation_type: general_type,
        source_section: relocation.section,
        source_offset: relocation.offset,
        addend: relocation.addend,
    })
}

/// Step 2: Encode GOT/PLT/RELA/dynamic tables, apply image-local relocations, and patch fixups.
pub(crate) fn resolve_dynamic_relocations(
    image: &FinalImage,
    plan: &DynamicLayoutPlan,
) -> Result<ResolvedDynamicRelocations, Diagnostic> {
    let resolver = image
        .import_program
        .resolver
        .as_ref()
        .ok_or_else(|| Diagnostic::error("ELF imports have no previously encoded resolver"))?;

    let mut plt_bytes = resolver.bytes.clone();
    let mut plt_entry_offsets = Vec::with_capacity(plan.linkage.slots.len());
    let mut fixup_list = Vec::new();
    collect_plt_operands(&mut fixup_list, resolver, 0, None)?;

    for slot in &plan.linkage.slots {
        let entry_offset = plt_bytes.len();
        plt_entry_offsets.push(checked_u64(entry_offset, "PLT entry offset")?);
        let request = &plan.imports[slot.request_index];
        let body = image.encoded_import_body(request.symbol_handle)?;
        let mut bytes = body.bytes.clone();
        patch_binding_ordinal(plan.target, &mut bytes, body, slot.logical_ordinal)?;
        collect_plt_operands(
            &mut fixup_list,
            body,
            entry_offset,
            Some(slot.logical_ordinal),
        )?;
        plt_bytes.extend_from_slice(&bytes);
    }

    let got_plt_len = checked_product(
        checked_sum(
            GOT_PLT_HEADER_WORDS,
            plan.linkage.slots.len(),
            "GOT word count",
        )?,
        ELF64_GOT_WORD_SIZE,
        "GOT size",
    )?;
    let mut got_plt_bytes = vec![0u8; got_plt_len];

    let mut rela_plt_bytes = Vec::with_capacity(checked_product(
        plan.linkage.slots.len(),
        ELF64_RELA_SIZE,
        "rela.plt size",
    )?);
    for slot in &plan.linkage.slots {
        write_u64(&mut rela_plt_bytes, 0);
        let info =
            (u64::from(slot.dynamic_symbol_index) << 32) | u64::from(plan.linkage.jump_slot_type);
        write_u64(&mut rela_plt_bytes, info);
        write_u64(&mut rela_plt_bytes, 0);
    }

    let mut rela_dyn_bytes = Vec::with_capacity(checked_product(
        plan.linkage.general_relocations.len(),
        ELF64_RELA_SIZE,
        "rela.dyn size",
    )?);
    for reloc in &plan.linkage.general_relocations {
        write_u64(&mut rela_dyn_bytes, 0);
        let info = (u64::from(reloc.dynamic_symbol_index) << 32) | u64::from(reloc.relocation_type);
        write_u64(&mut rela_dyn_bytes, info);
        write_u64(&mut rela_dyn_bytes, reloc.addend as u64);
    }

    let dynamic_bytes = encode_dynamic_table(
        &plan.tables,
        rela_plt_bytes.len(),
        rela_dyn_bytes.len(),
        &DynamicTableAddresses {
            got_plt: plan
                .section(DynamicSectionIndex::ProcedureGot)
                .virtual_address,
            sysv_hash: plan
                .section(DynamicSectionIndex::SystemVHash)
                .virtual_address,
            gnu_hash: plan.section(DynamicSectionIndex::GnuHash).virtual_address,
            dynstr: plan
                .section(DynamicSectionIndex::DynamicString)
                .virtual_address,
            dynsym: plan
                .section(DynamicSectionIndex::DynamicSymbol)
                .virtual_address,
            rela_plt: plan
                .section(DynamicSectionIndex::ProcedureRelocation)
                .virtual_address,
            rela_dyn: plan
                .section(DynamicSectionIndex::GeneralRelocation)
                .virtual_address,
            versym: plan
                .section(DynamicSectionIndex::GnuSymbolVersion)
                .virtual_address,
            verneed: plan
                .section(DynamicSectionIndex::GnuVersionRequirement)
                .virtual_address,
        },
    )?;

    // Apply image-local relocations on cloned memory so the caller's FinalImage stays unmodified on error.
    let (mut text_bytes, data_bytes) = apply_image_local_relocations(image, plan)?;

    // Append GOT, RELA, and source-text call-site fixups.
    match plan.target {
        TargetProfile::LinuxX64 => {
            fixup_list.push((
                FixupStorage::GotPlt,
                0,
                FixupKind::Absolute64,
                FixupTarget::DynamicSection,
            ));
            for slot in &plan.linkage.slots {
                fixup_list.push((
                    FixupStorage::GotPlt,
                    (GOT_PLT_HEADER_WORDS + slot.logical_ordinal as usize) * ELF64_GOT_WORD_SIZE,
                    FixupKind::Absolute64,
                    FixupTarget::PltLazyTail {
                        logical_ordinal: slot.logical_ordinal,
                    },
                ));
            }
        }
        TargetProfile::LinuxArm64 => {
            for slot in &plan.linkage.slots {
                fixup_list.push((
                    FixupStorage::GotPlt,
                    (GOT_PLT_HEADER_WORDS + slot.logical_ordinal as usize) * ELF64_GOT_WORD_SIZE,
                    FixupKind::Absolute64,
                    FixupTarget::PltHeader,
                ));
            }
        }
        _ => unreachable!(),
    }

    for (index, slot) in plan.linkage.slots.iter().enumerate() {
        fixup_list.push((
            FixupStorage::RelaPlt,
            index * ELF64_RELA_SIZE,
            FixupKind::Absolute64,
            FixupTarget::GotPltSlot {
                logical_ordinal: slot.logical_ordinal,
            },
        ));
    }
    for (index, reloc) in plan.linkage.general_relocations.iter().enumerate() {
        fixup_list.push((
            FixupStorage::RelaDyn,
            index * ELF64_RELA_SIZE,
            FixupKind::Absolute64,
            FixupTarget::ImageSection {
                section: reloc.source_section,
                byte_offset: reloc.source_offset,
            },
        ));
    }
    let call_kind = match plan.target {
        TargetProfile::LinuxX64 => FixupKind::X86PcRelative32,
        TargetProfile::LinuxArm64 => FixupKind::Aarch64Branch26,
        _ => unreachable!(),
    };
    for slot in &plan.linkage.slots {
        for site in &slot.call_sites {
            fixup_list.push((
                FixupStorage::SourceText,
                site.relocation_offset,
                call_kind,
                FixupTarget::PltEntry {
                    logical_ordinal: slot.logical_ordinal,
                },
            ));
        }
    }

    let plt_vaddr = plan
        .section(DynamicSectionIndex::ProcedureLinkage)
        .virtual_address;
    let got_vaddr = plan
        .section(DynamicSectionIndex::ProcedureGot)
        .virtual_address;
    let rela_plt_vaddr = plan
        .section(DynamicSectionIndex::ProcedureRelocation)
        .virtual_address;
    let rela_dyn_vaddr = plan
        .section(DynamicSectionIndex::GeneralRelocation)
        .virtual_address;
    let dyn_vaddr = plan
        .section(DynamicSectionIndex::DynamicTable)
        .virtual_address;

    for (storage, byte_offset, kind, target) in fixup_list {
        let storage_base = match storage {
            FixupStorage::SourceText => plan.image_layout.text_address,
            FixupStorage::Plt => plt_vaddr,
            FixupStorage::GotPlt => got_vaddr,
            FixupStorage::RelaPlt => rela_plt_vaddr,
            FixupStorage::RelaDyn => rela_dyn_vaddr,
        };
        let source_address = checked_sum_u64(
            storage_base,
            checked_u64(byte_offset, "fixup byte offset")?,
            "fixup source address",
        )?;
        let target_address = match target {
            FixupTarget::DynamicSection => dyn_vaddr,
            FixupTarget::PltHeader => plt_vaddr,
            FixupTarget::PltEntry { logical_ordinal } => checked_sum_u64(
                plt_vaddr,
                plt_entry_offsets[logical_ordinal as usize],
                "PLT entry address",
            )?,
            FixupTarget::PltLazyTail { logical_ordinal } => checked_sum_u64(
                plt_vaddr,
                checked_sum_u64(
                    plt_entry_offsets[logical_ordinal as usize],
                    X86_PLT_LAZY_TAIL_OFFSET,
                    "PLT lazy tail offset",
                )?,
                "PLT lazy tail address",
            )?,
            FixupTarget::GotPltHeaderWord { word_index } => checked_sum_u64(
                got_vaddr,
                u64::from(word_index) * ELF64_GOT_WORD_SIZE as u64,
                "GOT header word address",
            )?,
            FixupTarget::GotPltSlot { logical_ordinal } => checked_sum_u64(
                got_vaddr,
                (GOT_PLT_HEADER_WORDS as u64 + u64::from(logical_ordinal))
                    * ELF64_GOT_WORD_SIZE as u64,
                "GOT slot address",
            )?,
            FixupTarget::ImageSection {
                section,
                byte_offset,
            } => {
                let base = match section {
                    FinalImageSection::Text => plan.image_layout.text_address,
                    FinalImageSection::Data => plan.image_layout.data_address,
                    FinalImageSection::Bss => plan.image_layout.bss_address,
                    FinalImageSection::None => {
                        return Err(Diagnostic::error(
                            "general relocation targets no materialized image section",
                        ));
                    }
                };
                checked_sum_u64(
                    base,
                    checked_u64(byte_offset, "general slot offset")?,
                    "general relocation target address",
                )?
            }
        };

        let buf: &mut [u8] = match storage {
            FixupStorage::SourceText => &mut text_bytes,
            FixupStorage::Plt => &mut plt_bytes,
            FixupStorage::GotPlt => &mut got_plt_bytes,
            FixupStorage::RelaPlt => &mut rela_plt_bytes,
            FixupStorage::RelaDyn => &mut rela_dyn_bytes,
        };
        apply_fixup(buf, byte_offset, kind, source_address, target_address)?;
    }

    let mut import_regions = Vec::with_capacity(plan.linkage.slots.len());
    for slot in &plan.linkage.slots {
        let request = &plan.imports[slot.request_index];
        let (ElfImportLocator::Versioned { symbol, .. }
        | ElfImportLocator::Component { symbol, .. }) = &request.locator;
        let symbol = String::from_utf8(symbol.clone())
            .map_err(|_| Diagnostic::error("dynamic ELF import symbol spelling is not UTF-8"))?;
        import_regions.push(FinalDataRegion {
            origin: FinalDataRegionOrigin::ImportBindingSlot,
            section_offset: (GOT_PLT_HEADER_WORDS + slot.logical_ordinal as usize)
                * ELF64_GOT_WORD_SIZE,
            byte_count: ELF64_GOT_WORD_SIZE,
            symbol,
        });
    }
    let import_data_regions = place_data_extent(
        &got_plt_bytes,
        import_regions,
        got_vaddr,
        "dynamic ELF .got.plt",
    )?;

    Ok(ResolvedDynamicRelocations {
        text_bytes,
        data_bytes,
        plt_bytes,
        got_plt_bytes,
        rela_plt_bytes,
        rela_dyn_bytes,
        dynamic_bytes,
        import_data_regions,
    })
}

fn patch_binding_ordinal(
    target: TargetProfile,
    bytes: &mut [u8],
    body: &EncodedImportBody,
    logical_ordinal: u32,
) -> Result<(), Diagnostic> {
    let mut ordinals = body
        .operands
        .iter()
        .filter(|operand| operand.target == ImportOperandTarget::BindingOrdinal);
    if target == TargetProfile::LinuxX64 {
        let operand = ordinals.next().ok_or_else(|| {
            Diagnostic::error("ELF import lacks an encoded binding ordinal operand")
        })?;
        if operand.kind != ImportOperandKind::X86SignedOrdinal32 || ordinals.next().is_some() {
            return Err(Diagnostic::error(
                "ELF binding ordinal operand has a different encoding",
            ));
        }
        let ordinal = i32::try_from(logical_ordinal).map_err(|_| {
            Diagnostic::error("x86-64 lazy-binding relocation index exceeds signed imm32")
        })?;
        let end = checked_sum(operand.offset, 4, "ELF binding ordinal operand")?;
        let field = bytes
            .get_mut(operand.offset..end)
            .ok_or_else(|| Diagnostic::error("ELF binding ordinal operand is out of bounds"))?;
        require(
            field == [0; 4],
            "ELF binding ordinal is not an encoded zero placeholder",
        )?;
        field.copy_from_slice(&ordinal.to_le_bytes());
    } else if ordinals.next().is_some() {
        return Err(Diagnostic::error(
            "AArch64 import carries an unexpected binding ordinal operand",
        ));
    }
    Ok(())
}

fn collect_plt_operands(
    fixups: &mut Vec<(FixupStorage, usize, FixupKind, FixupTarget)>,
    body: &EncodedImportBody,
    base_offset: usize,
    ordinal: Option<u32>,
) -> Result<(), Diagnostic> {
    for operand in &body.operands {
        if operand.target == ImportOperandTarget::BindingOrdinal {
            if ordinal.is_none() || operand.kind != ImportOperandKind::X86SignedOrdinal32 {
                return Err(Diagnostic::error(
                    "ELF binding ordinal is outside an encoded import entry",
                ));
            }
            continue;
        }
        let kind = match operand.kind {
            ImportOperandKind::X86Relative32 => FixupKind::X86PcRelative32,
            ImportOperandKind::Aarch64Page21 => FixupKind::Aarch64Page21,
            ImportOperandKind::Aarch64PageOffset12 { scale: 3 } => FixupKind::Aarch64Load64Low12,
            ImportOperandKind::Aarch64PageOffset12 { scale: 0 } => FixupKind::Aarch64AddLow12,
            _ => return Err(Diagnostic::error("unsupported encoded ELF import operand")),
        };
        let target = match operand.target {
            ImportOperandTarget::BindingSlot => FixupTarget::GotPltSlot {
                logical_ordinal: ordinal.ok_or_else(|| {
                    Diagnostic::error("ELF resolver refers to an import-local binding slot")
                })?,
            },
            ImportOperandTarget::LoaderContext if ordinal.is_none() => {
                FixupTarget::GotPltHeaderWord { word_index: 1 }
            }
            ImportOperandTarget::LoaderResolver if ordinal.is_none() => {
                FixupTarget::GotPltHeaderWord { word_index: 2 }
            }
            ImportOperandTarget::ResolverEntry if ordinal.is_some() => FixupTarget::PltHeader,
            _ => {
                return Err(Diagnostic::error(
                    "encoded ELF import operand has an invalid symbolic target",
                ));
            }
        };
        fixups.push((
            FixupStorage::Plt,
            checked_sum(base_offset, operand.offset, "placed encoded import operand")?,
            kind,
            target,
        ));
    }
    Ok(())
}

fn apply_image_local_relocations(
    image: &FinalImage,
    plan: &DynamicLayoutPlan,
) -> Result<(Vec<u8>, Vec<u8>), Diagnostic> {
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
        import_program: image.import_program.clone(),
        symbol_table: image.symbol_table.clone(),
        relocation_table,
        executable_regions: Vec::new(),
        data_regions: Vec::new(),
        function_unwinds: Vec::new(),
    };
    apply_relocations(&mut local_image, &plan.image_layout, "dynamic ELF image")?;
    Ok((local_image.memory.text, local_image.memory.data))
}

fn apply_fixup(
    bytes: &mut [u8],
    offset: usize,
    kind: FixupKind,
    source_address: u64,
    target_address: u64,
) -> Result<(), Diagnostic> {
    let (width, mutable_mask): (usize, u64) = match kind {
        FixupKind::X86PcRelative32 => (4, 0xffff_ffff),
        FixupKind::Aarch64Page21 => (4, 0x60ff_ffe0),
        FixupKind::Aarch64Load64Low12 | FixupKind::Aarch64AddLow12 => (4, 0x003f_fc00),
        FixupKind::Aarch64Branch26 => (4, 0x03ff_ffff),
        FixupKind::Absolute64 => (8, u64::MAX),
    };
    let end = checked_sum(offset, width, "fixup field end")?;
    let slice = bytes
        .get_mut(offset..end)
        .ok_or_else(|| Diagnostic::error("procedure-linkage fixup exceeds section storage"))?;
    let original = if width == 4 {
        u64::from(u32::from_le_bytes(slice.try_into().expect("4-byte field")))
    } else {
        u64::from_le_bytes(slice.try_into().expect("8-byte field"))
    };
    require(
        original & mutable_mask == 0,
        "procedure-linkage fixup source is not an exact zero placeholder",
    )?;

    let mutable_bits = match kind {
        FixupKind::X86PcRelative32 => {
            let next_instruction = checked_sum_u64(source_address, 4, "x86-64 relocation PC")?;
            let delta = i128::from(target_address) - i128::from(next_instruction);
            let disp = i32::try_from(delta).map_err(|_| {
                Diagnostic::error(format!(
                    "x86-64 procedure relocation is out of signed-32 range: {delta} byte(s)"
                ))
            })?;
            u64::from(disp as u32)
        }
        FixupKind::Aarch64Page21 => {
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
        FixupKind::Aarch64Load64Low12 => {
            let page_offset = target_address & 0xfff;
            require(
                page_offset.is_multiple_of(8),
                "AArch64 procedure LDR target is not eight-byte aligned",
            )?;
            (page_offset / 8) << 10
        }
        FixupKind::Aarch64AddLow12 => (target_address & 0xfff) << 10,
        FixupKind::Aarch64Branch26 => {
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
        FixupKind::Absolute64 => target_address,
    };
    require(
        mutable_bits & !mutable_mask == 0,
        "procedure relocation encoding exceeds its typed mutable mask",
    )?;
    let encoded = (original & !mutable_mask) | mutable_bits;
    if width == 4 {
        slice.copy_from_slice(&(encoded as u32).to_le_bytes());
    } else {
        slice.copy_from_slice(&encoded.to_le_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{classify_import_relocations, resolve_dynamic_relocations};
    use crate::dynamic_layout::{DynamicSectionIndex, plan_dynamic_layout};
    use crate::imports::canonical_referenced_imports;
    use crate::test_support::{
        IMPORTS, UTF8_IMPORTS, dynamic_import_inputs, text_entry, versioned_import,
    };
    use image::{FinalImageRelocation, FinalImageSection};
    use object_file::RelocationKind;
    use target::TargetProfile;

    #[test]
    fn data_slot_import_relocations_emit_exact_general_rela_rows() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let mut image = crate::test_support::import_image(target, 8, 2);
            image.memory.data = vec![0; 16];
            text_entry(&mut image, 0);
            let sym = versioned_import(
                &mut image,
                "__omega_data_sym",
                b"libdata.so",
                b"data_entry",
                b"V1",
                target,
                "valid",
            );
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    section: FinalImageSection::Data,
                    offset: 8,
                    byte_width: 8,
                    symbol_handle: sym,
                    addend: 12,
                    kind: RelocationKind::Absolute64,
                });
            let interp = crate::test_support::interpreter(target);
            let plan = plan_dynamic_layout(&image, &interp).unwrap();
            let resolved = resolve_dynamic_relocations(&image, &plan).unwrap();
            assert_eq!(plan.linkage.general_relocations.len(), 1);
            assert_eq!(resolved.rela_dyn_bytes.len(), 24);
            let r_offset = u64::from_le_bytes(resolved.rela_dyn_bytes[0..8].try_into().unwrap());
            let r_info = u64::from_le_bytes(resolved.rela_dyn_bytes[8..16].try_into().unwrap());
            let r_addend = i64::from_le_bytes(resolved.rela_dyn_bytes[16..24].try_into().unwrap());
            assert_eq!(r_offset, plan.image_layout.data_address + 8);
            assert_eq!(r_info >> 32, 1);
            assert_eq!(
                r_info & 0xffff_ffff,
                match target {
                    TargetProfile::LinuxX64 => 1,
                    TargetProfile::LinuxArm64 => 257,
                    _ => unreachable!(),
                }
            );
            assert_eq!(r_addend, 12);
        }
    }

    #[test]
    fn malformed_call_placeholders_and_overlapping_relocations_reject() {
        let target = TargetProfile::LinuxX64;
        let (mut bad_opcode, _, imports) =
            dynamic_import_inputs(target, &IMPORTS, "__omega_bad_", "valid", "valid");
        let bindings = crate::dynamic_tables::build_dynamic_import_tables(
            &imports,
            b"/lib64/ld-linux-x86-64.so.2",
        )
        .unwrap()
        .contents
        .bindings;
        bad_opcode.memory.text[0] = 0x90;
        assert!(classify_import_relocations(target, &bad_opcode, &imports, &bindings).is_err());

        let mut unaligned_data = crate::test_support::import_image(target, 8, 1);
        unaligned_data.memory.data = vec![0; 16];
        let sym = versioned_import(
            &mut unaligned_data,
            "__omega_unaligned",
            b"liba.so",
            b"sym",
            b"V1",
            target,
            "valid",
        );
        unaligned_data
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                section: FinalImageSection::Data,
                offset: 4,
                byte_width: 8,
                symbol_handle: sym,
                addend: 0,
                kind: RelocationKind::Absolute64,
            });
        let imports = canonical_referenced_imports(&unaligned_data).unwrap();
        let bindings = crate::dynamic_tables::build_dynamic_import_tables(
            &imports,
            b"/lib64/ld-linux-x86-64.so.2",
        )
        .unwrap()
        .contents
        .bindings;
        assert!(classify_import_relocations(target, &unaligned_data, &imports, &bindings).is_err());
    }

    #[test]
    fn got_plt_header_words_and_import_custody_match_target_abi() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let (mut image, interp, _) =
                dynamic_import_inputs(target, &UTF8_IMPORTS, "__omega_got_", "valid", "valid");
            text_entry(&mut image, 0);
            let plan = plan_dynamic_layout(&image, &interp).unwrap();
            let resolved = resolve_dynamic_relocations(&image, &plan).unwrap();
            let got0 = u64::from_le_bytes(resolved.got_plt_bytes[0..8].try_into().unwrap());
            let got1 = u64::from_le_bytes(resolved.got_plt_bytes[8..16].try_into().unwrap());
            let got2 = u64::from_le_bytes(resolved.got_plt_bytes[16..24].try_into().unwrap());
            assert_eq!(got1, 0);
            assert_eq!(got2, 0);
            match target {
                TargetProfile::LinuxX64 => assert_eq!(
                    got0,
                    plan.section(DynamicSectionIndex::DynamicTable)
                        .virtual_address
                ),
                TargetProfile::LinuxArm64 => assert_eq!(got0, 0),
                _ => unreachable!(),
            }
            assert_eq!(resolved.import_data_regions.regions.len(), IMPORTS.len());
        }
    }
}
