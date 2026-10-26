//! Separate ELF shared-component containers with exact symbolic exports.
//!
//! This is an ET_DYN/PIC owner, distinct from the process ET_EXEC writer. It
//! reuses the canonical versioned-import table producer, retains the original
//! FinalImage and export roster, and performs no source compilation or mapping.
//! A container is not an InstalledCode claim, runtime era or callable address.
//!
//! ELF wire contracts: https://gabi.xinuos.com/elf/05-symtab.html and
//! https://gabi.xinuos.com/elf/08-dynamic.html .

mod contents;

use diagnostics::Diagnostic;
use image::{FinalImage, FinalImageSymbolHandle};

/// One selected export's linker spelling and original final-image definition.
/// The caller obtains this projection from a complete checked slot application;
/// the ELF owner cannot infer that semantic admission from a spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElfComponentExport {
    pub symbol: FinalImageSymbolHandle,
    pub name: Vec<u8>,
    pub slot_commitment: [u8; 32],
}

#[derive(Debug)]
pub struct ElfSharedComponentImage {
    image: FinalImage,
    exports: Vec<ElfComponentExport>,
    contents: Vec<u8>,
}

impl ElfSharedComponentImage {
    pub const fn image(&self) -> &FinalImage {
        &self.image
    }
    pub fn exports(&self) -> &[ElfComponentExport] {
        &self.exports
    }
    pub fn contents(&self) -> &[u8] {
        &self.contents
    }
}

#[derive(Debug)]
pub struct ElfSharedComponentEmissionError {
    image: FinalImage,
    exports: Vec<ElfComponentExport>,
    diagnostic: Diagnostic,
}

image::rejection_carrier!(
    ElfSharedComponentEmissionError;
    image: FinalImage,
    exports: Vec<ElfComponentExport>,
);

/// Emit a separately loadable component without borrowing process-entry
/// authority or relabeling a dynamic executable. Failure retains both complete
/// original inputs. Absolute local pointers need loader-relative relocations
/// and reject until that distinct lane is implemented.
pub fn emit_elf_shared_component(
    image: FinalImage,
    exports: Vec<ElfComponentExport>,
) -> Result<ElfSharedComponentImage, Box<ElfSharedComponentEmissionError>> {
    match contents::emit(&image, &exports) {
        Ok(contents) => Ok(ElfSharedComponentImage {
            image,
            exports,
            contents,
        }),
        Err(diagnostic) => Err(Box::new(ElfSharedComponentEmissionError {
            image,
            exports,
            diagnostic,
        })),
    }
}
