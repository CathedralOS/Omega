//! In-place relocation application: one driver over the final image's
//! relocation table that resolves each symbol once, checks the record against
//! the image's instruction set, and patches the field its relocation kind
//! declares.
//!
//! This driver used to exist once per instruction set, and each copy carried
//! its own symbol resolution, addend overflow arm, absolute-slot arm and
//! foreign-kind rejection; only the field encodings are genuinely
//! per-instruction-set, so those stay in the named patch functions below.
//! The relocation kind itself names the instruction set whose encoding it
//! spells, so an image whose target disagrees is refused here rather than by
//! which entrypoint a caller happened to pick.
//!
//! [`relocation_field_geometry`] is the single declaration of how wide each
//! kind's field is and which instruction bits it owns. The patchers mask with
//! it, and `relocation_envelope.rs` excuses exactly those bits after the fact;
//! the envelope previously carried the masks a second time as little-endian
//! byte arrays, kept in step with this arithmetic by hand.
//!
//! The constants the instruction sets force on us:
//!
//! - AArch64 ADRP: pages are 4096 bytes (`address & !0xfff`), the page delta
//!   must lie in `[-2^20, 2^20)`, and the 21-bit immediate splits into
//!   `immlo` at instruction bits 29-30 and `immhi` at bits 5-23.
//! - AArch64 ADD page offset: `symbol_address & 0xfff`, shifted into bits
//!   10-21.
//! - x86-64 rel32: `relocation_address = section_address + offset + 4`. That
//!   `+ 4` is an assumption never stated at the instruction: it says the
//!   four-byte displacement is the last field of the instruction, and it is
//!   why the delta is computed in `i128` before being narrowed to `i32`.

use crate::{
    FinalImage, FinalImageLayout, FinalImageSection, final_image_imports_symbol,
    final_image_symbol_address, final_image_symbol_name, patch_bytes,
};
use diagnostics::Diagnostic;
use object_file::RelocationKind;
use target::Architecture;

/// Apply every checked relocation in the image's plan to its section bytes
/// against the layout the emitter chose.
pub fn apply_relocations(
    image: &mut FinalImage,
    layout: &FinalImageLayout,
    output_name: &str,
) -> Result<(), Diagnostic> {
    let architecture = image.target.architecture;
    let isa = architecture_name(architecture);
    for (_, relocation) in image.relocation_table.relocations.iter() {
        let Some(symbol_address) =
            final_image_symbol_address(image, relocation.symbol_handle, layout)
        else {
            let symbol_name = final_image_symbol_name(image, relocation.symbol_handle);
            if final_image_imports_symbol(image, relocation.symbol_handle) {
                return Err(Diagnostic::error(format!(
                    "{output_name} cannot import `{symbol_name}` yet; use syscalls or add dynamic binding"
                )));
            }

            return Err(Diagnostic::error(format!(
                "{output_name} relocation references unknown symbol `{symbol_name}`"
            )));
        };
        let relocation_target = symbol_address
            .checked_add_signed(relocation.addend)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "{output_name} {isa} relocation target overflows after addend {}",
                    relocation.addend
                ))
            })?;
        if let Some(kind_architecture) = kind_architecture(relocation.kind) {
            if kind_architecture != architecture {
                return Err(Diagnostic::error(format!(
                    "{output_name} {isa} image received {} relocation",
                    architecture_name(kind_architecture)
                )));
            }
        }

        match relocation.kind {
            RelocationKind::Absolute64 => {
                let section = relocation.section;
                let section_bytes = image.memory.initialized_section_mut(section).ok_or_else(|| {
                    Diagnostic::error(format!(
                        "{output_name} {isa} absolute relocation targets non-materialized section {section:?}"
                    ))
                })?;
                patch_bytes::write_u64(section_bytes, relocation.offset, relocation_target, isa)?;
            }
            RelocationKind::X86_64Relative32 => {
                let section = relocation.section;
                let section_address = layout.section_address(section).ok_or_else(|| {
                    Diagnostic::error(format!(
                        "{output_name} {isa} relative relocation has no section"
                    ))
                })?;
                let relocation_address = section_address + relocation.offset as u64 + 4;
                let delta = i128::from(relocation_target) - i128::from(relocation_address);
                let value = i32::try_from(delta).map_err(|_| {
                    Diagnostic::error(format!(
                        "{output_name} {isa} relative relocation is out of range: {delta} byte(s)"
                    ))
                })?;
                let section_bytes = image.memory.initialized_section_mut(section).ok_or_else(|| {
                    Diagnostic::error(format!(
                        "{output_name} {isa} relative relocation targets non-materialized section {section:?}"
                    ))
                })?;
                patch_bytes::write_i32(section_bytes, relocation.offset, value, isa)?;
            }
            RelocationKind::Aarch64Page21 => {
                require_text_relocation(output_name, isa, relocation.section)?;
                patch_aarch64_adrp(
                    &mut image.memory.text,
                    relocation.offset,
                    layout.text_address + relocation.offset as u64,
                    relocation_target,
                    isa,
                )?;
            }
            RelocationKind::Aarch64PageOffset12 => {
                require_text_relocation(output_name, isa, relocation.section)?;
                patch_aarch64_add_page_offset(
                    &mut image.memory.text,
                    relocation.offset,
                    relocation_target,
                    isa,
                )?;
            }
            RelocationKind::Aarch64Branch26 => {
                require_text_relocation(output_name, isa, relocation.section)?;
                patch_aarch64_branch26(
                    &mut image.memory.text,
                    relocation.offset,
                    layout.text_address + relocation.offset as u64,
                    relocation_target,
                    isa,
                )?;
            }
        }
    }

    Ok(())
}

/// The instruction set whose field encoding a relocation kind spells, or
/// `None` for an address-sized slot that belongs to every instruction set.
const fn kind_architecture(kind: RelocationKind) -> Option<Architecture> {
    match kind {
        RelocationKind::Aarch64Page21
        | RelocationKind::Aarch64PageOffset12
        | RelocationKind::Aarch64Branch26 => Some(Architecture::Aarch64),
        RelocationKind::X86_64Relative32 => Some(Architecture::X86_64),
        RelocationKind::Absolute64 => None,
    }
}

/// The diagnostic and patch-label spelling of an architecture. Every
/// instruction-set name in this module's diagnostics comes from here.
const fn architecture_name(architecture: Architecture) -> &'static str {
    match architecture {
        Architecture::Aarch64 => "AArch64",
        Architecture::X86_64 => "x86_64",
    }
}

/// The field one relocation kind owns: the byte width a checked plan must
/// declare for it, and the instruction bits it may rewrite. Bits are numbered
/// in the little-endian field, so `mask.to_le_bytes()[..byte_width]` is the
/// byte-array form of the same field.
pub(crate) struct RelocationFieldGeometry {
    pub byte_width: usize,
    pub mask: u64,
}

/// The one declaration of each kind's field geometry, read by the patchers
/// below and by `relocation_envelope`'s after-the-fact proof. The envelope test
/// `each_relocation_kind_masks_exactly_its_declared_field_bits` pins this
/// declaration against independently written byte masks.
pub(crate) const fn relocation_field_geometry(kind: RelocationKind) -> RelocationFieldGeometry {
    match kind {
        RelocationKind::Aarch64Page21 => RelocationFieldGeometry {
            byte_width: 4,
            mask: 0x60ff_ffe0,
        },
        RelocationKind::Aarch64PageOffset12 => RelocationFieldGeometry {
            byte_width: 4,
            mask: 0x003f_fc00,
        },
        RelocationKind::Aarch64Branch26 => RelocationFieldGeometry {
            byte_width: 4,
            mask: 0x03ff_ffff,
        },
        RelocationKind::Absolute64 => RelocationFieldGeometry {
            byte_width: 8,
            mask: u64::MAX,
        },
        RelocationKind::X86_64Relative32 => RelocationFieldGeometry {
            byte_width: 4,
            mask: 0xffff_ffff,
        },
    }
}

fn require_text_relocation(
    output_name: &str,
    isa: &str,
    section: FinalImageSection,
) -> Result<(), Diagnostic> {
    if section == FinalImageSection::Text {
        Ok(())
    } else {
        Err(Diagnostic::error(format!(
            "{output_name} {isa} instruction relocation targets non-text section {section:?}"
        )))
    }
}

fn patch_aarch64_adrp(
    text: &mut [u8],
    offset: usize,
    instruction_address: u64,
    symbol_address: u64,
    isa: &str,
) -> Result<(), Diagnostic> {
    let instruction_page = instruction_address & !0xfff;
    let symbol_page = symbol_address & !0xfff;
    let page_delta = (symbol_page as i64 - instruction_page as i64) / 4096;

    if !(-(1 << 20)..(1 << 20)).contains(&page_delta) {
        return Err(Diagnostic::error(format!(
            "AArch64 ADRP relocation is out of range: {page_delta} page(s)"
        )));
    }

    let immediate = (page_delta as u32) & 0x1f_ffff;
    let immediate_low = immediate & 0b11;
    let immediate_high = (immediate >> 2) & 0x7ffff;
    let mask = relocation_field_geometry(RelocationKind::Aarch64Page21).mask as u32;
    let mut instruction = patch_bytes::read_u32(text, offset, isa)?;
    instruction &= !mask;
    instruction |= (immediate_low << 29) | (immediate_high << 5);
    patch_bytes::write_u32(text, offset, instruction, isa)
}

fn patch_aarch64_add_page_offset(
    text: &mut [u8],
    offset: usize,
    symbol_address: u64,
    isa: &str,
) -> Result<(), Diagnostic> {
    let page_offset = (symbol_address & 0xfff) as u32;
    let mask = relocation_field_geometry(RelocationKind::Aarch64PageOffset12).mask as u32;
    let mut instruction = patch_bytes::read_u32(text, offset, isa)?;
    instruction &= !mask;
    instruction |= page_offset << 10;
    patch_bytes::write_u32(text, offset, instruction, isa)
}

fn patch_aarch64_branch26(
    text: &mut [u8],
    offset: usize,
    instruction_address: u64,
    symbol_address: u64,
    isa: &str,
) -> Result<(), Diagnostic> {
    let byte_delta = symbol_address as i64 - instruction_address as i64;
    if byte_delta % 4 != 0 {
        return Err(Diagnostic::error(
            "AArch64 branch relocation target is not instruction-aligned",
        ));
    }
    let immediate = byte_delta / 4;
    if !(-(1 << 25)..(1 << 25)).contains(&immediate) {
        return Err(Diagnostic::error(format!(
            "AArch64 branch relocation is out of range: {immediate} instruction(s)"
        )));
    }

    let mask = relocation_field_geometry(RelocationKind::Aarch64Branch26).mask as u32;
    let mut instruction = patch_bytes::read_u32(text, offset, isa)?;
    instruction &= !mask;
    instruction |= (immediate as u32) & 0x03ff_ffff;
    patch_bytes::write_u32(text, offset, instruction, isa)
}

#[cfg(test)]
mod tests {
    use super::apply_relocations;
    use crate::{
        FinalImage, FinalImageLayout, FinalImageMemory, FinalImageRelocation, FinalImageSection,
        FinalImageSymbol,
    };
    use arena::Handle;
    use object_file::{RelocationKind, SymbolKind};
    use target::NativeTarget;

    /// An address-sized slot belongs to every instruction set, so the same
    /// signed addend lands on each native target; the two former per-ISA test
    /// copies carry only the numbers this table holds.
    #[test]
    fn absolute_relocation_applies_signed_addend_per_target() {
        for (native_target, symbol_offset, addend, expected) in [
            (NativeTarget::linux_arm64(), 8, -4, 0x1004u64),
            (NativeTarget::linux_x64(), 4, 7, 0x100b),
        ] {
            let mut image = FinalImage::with_capacity(
                native_target,
                FinalImageMemory {
                    text: vec![0; 8],
                    data: vec![0; 8],
                    ..Default::default()
                },
                Handle::invalid(),
                1,
                0,
                1,
            );
            let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: "entry".into(),
                section: FinalImageSection::Text,
                offset: symbol_offset,
                size: 1,
                kind: SymbolKind::Function,
            });
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    section: FinalImageSection::Data,
                    offset: 0,
                    byte_width: 8,
                    symbol_handle: symbol,
                    addend,
                    kind: RelocationKind::Absolute64,
                });

            apply_relocations(
                &mut image,
                &FinalImageLayout {
                    text_address: 0x1000,
                    data_address: 0x2000,
                    bss_address: 0x3000,
                },
                "test image",
            )
            .expect("data relocation should apply");

            assert_eq!(
                u64::from_le_bytes(image.memory.data.try_into().unwrap()),
                expected
            );
        }
    }
}
