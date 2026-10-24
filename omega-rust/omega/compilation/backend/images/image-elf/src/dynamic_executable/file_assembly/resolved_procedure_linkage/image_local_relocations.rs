//! Image-local source relocations of the retained `FinalImage`.
//!
//! The procedure-linkage fixups bind only relocations whose symbol is a
//! dynamic import. Every other retained relocation names a symbol the image
//! itself defines: a call or callback-address materialization that targets a
//! compiler-private function, a `.text` reference into `.data` or `.bss`, or an
//! initialized-data pointer. The dynamic image is `ET_EXEC` at a fixed base, so
//! no loader row exists for these; the writer resolves them against the
//! absolute load layout exactly as the static lane resolves them against its
//! own layout. Leaving them unapplied would publish zero displacements that
//! land on the byte after the relocated field instead of on the named symbol.
//!
//! The patch runs on copies of `.text` and `.data`; the retained image stays
//! immutable, and every import field keeps its zero placeholder for the
//! procedure-linkage fixups that follow.

use crate::dynamic_executable::load_placement::load_layout::ValidatedElfDynamicLoadLayout;
use diagnostics::Diagnostic;
use image::{FinalImage, FinalImageRelocationTable, apply_relocations, final_image_imports_symbol};
use target::TargetProfile;

/// Retained `.text` and `.data` after every image-local relocation is patched
/// at the absolute load layout. Import relocation fields are untouched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfImageLocalRelocatedMemory {
    pub(crate) text: Vec<u8>,
    pub(crate) data: Vec<u8>,
}

/// Patch copies of the retained source memory with every relocation whose
/// symbol is not a dynamic import, using the target's ordinary relocation
/// encoder and the load layout's final text/data/bss addresses.
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
    // A scratch image carrying only the image-local relocation rows lets the
    // shared encoders patch exactly those fields; symbol handles stay valid
    // because the symbol table is copied unchanged.
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
mod tests;
