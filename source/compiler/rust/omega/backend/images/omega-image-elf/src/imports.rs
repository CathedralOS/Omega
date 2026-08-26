//! Canonical custody for referenced ELF dynamic-import requests.
//!
//! This module deliberately stops before choosing a loader realization. It
//! binds each referenced import's exact final-image symbol handle and authored
//! library/symbol pair to every relocation that consumes it. A future ELF
//! dynamic-link implementation can consume these rows without rediscovering
//! import identity from spellings or ambient libraries.

use omega_image::{
    FinalImage, FinalImageImportPlan, FinalImageRelocation, FinalImageSection,
    FinalImageSymbolHandle,
};
use omega_object_file::SymbolKind;
use psi_diagnostics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfImportRequest {
    pub(crate) symbol_handle: FinalImageSymbolHandle,
    pub(crate) library: String,
    pub(crate) symbol: String,
    pub(crate) relocations: Vec<FinalImageRelocation>,
}

pub(crate) fn canonical_referenced_imports(
    image: &FinalImage,
) -> Result<Vec<ElfImportRequest>, Diagnostic> {
    let mut requests = Vec::new();
    let mut import_handles = Vec::new();

    for (_, import) in image.symbol_table.imports.iter() {
        if import_handles.contains(&import.symbol_handle) {
            return Err(Diagnostic::error(
                "ELF final image retains duplicate import rows for one symbol handle",
            ));
        }
        import_handles.push(import.symbol_handle);
        let symbol = image
            .symbol_table
            .symbols
            .is_valid(import.symbol_handle)
            .then(|| image.symbol_table.symbols.get(import.symbol_handle))
            .ok_or_else(|| {
                Diagnostic::error("ELF final image import names an invalid symbol handle")
            })?;
        if symbol.kind != SymbolKind::Import
            || symbol.section != FinalImageSection::None
            || symbol.offset != 0
            || symbol.size != 0
        {
            return Err(Diagnostic::error(format!(
                "ELF import `{}` is not an unresolved zero-width import symbol",
                symbol.name
            )));
        }
        let library = match &import.import {
            FinalImageImportPlan::StringBackedBootstrap { library } => library,
            FinalImageImportPlan::Normalized(locator) => {
                return Err(Diagnostic::error(format!(
                    "normalized foreign locator 0x{:016x} reached ELF emission before symbol-version semantics are implemented",
                    locator.normalized_identity(),
                )));
            }
            FinalImageImportPlan::None => {
                return Err(Diagnostic::error(format!(
                    "ELF import `{}` has no retained physical import plan",
                    symbol.name
                )));
            }
        };
        if library.is_empty()
            || library.as_bytes().contains(&0)
            || symbol.name.is_empty()
            || symbol.name.as_bytes().contains(&0)
        {
            return Err(Diagnostic::error(format!(
                "ELF import `{}` lacks a canonical library/symbol spelling",
                symbol.name
            )));
        }

        let relocations = image
            .relocation_table
            .relocations
            .iter()
            .filter_map(|(_, relocation)| {
                (relocation.symbol_handle == import.symbol_handle).then(|| relocation.clone())
            })
            .collect::<Vec<_>>();
        if !relocations.is_empty() {
            requests.push(ElfImportRequest {
                symbol_handle: import.symbol_handle,
                library: library.clone(),
                symbol: symbol.name.clone(),
                relocations,
            });
        }
    }

    for (_, relocation) in image.relocation_table.relocations.iter() {
        if !image
            .symbol_table
            .symbols
            .is_valid(relocation.symbol_handle)
        {
            continue;
        }
        let symbol = image.symbol_table.symbols.get(relocation.symbol_handle);
        if symbol.kind == SymbolKind::Import
            && !requests
                .iter()
                .any(|request| request.symbol_handle == relocation.symbol_handle)
        {
            return Err(Diagnostic::error(format!(
                "ELF relocation references import `{}` without one exact import row",
                symbol.name
            )));
        }
    }

    Ok(requests)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_image::{
        FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
        FinalImageSymbol,
    };
    use omega_object_file::RelocationKind;
    use psi_arena::Handle;

    fn imported_image() -> FinalImage {
        let mut image = FinalImage::with_capacity(
            FinalImage::default().target,
            FinalImageMemory {
                text: vec![0; 16],
                ..FinalImageMemory::default()
            },
            Handle::invalid(),
            1,
            1,
            2,
        );
        let imported = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "omega_probe".into(),
            section: FinalImageSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: imported,
            import: FinalImageImportPlan::StringBackedBootstrap {
                library: "libomega-probes.so".into(),
            },
        });
        for offset in [4, 12] {
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
        image
    }

    #[test]
    fn canonical_request_retains_exact_library_symbol_and_relocation_sites() {
        let image = imported_image();
        let requests = canonical_referenced_imports(&image).expect("canonical request");

        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].library, "libomega-probes.so");
        assert_eq!(requests[0].symbol, "omega_probe");
        assert_eq!(
            requests[0]
                .relocations
                .iter()
                .map(|relocation| relocation.offset)
                .collect::<Vec<_>>(),
            [4, 12]
        );
    }

    #[test]
    fn duplicate_or_unqualified_import_identity_rejects() {
        let mut duplicate = imported_image();
        let imported = duplicate
            .symbol_table
            .imports
            .iter()
            .next()
            .unwrap()
            .1
            .clone();
        duplicate.symbol_table.imports.insert(imported);
        assert!(canonical_referenced_imports(&duplicate).is_err());

        let mut unqualified = imported_image();
        let import_handle = unqualified.symbol_table.imports.iter().next().unwrap().0;
        unqualified
            .symbol_table
            .imports
            .get_mut(import_handle)
            .import = FinalImageImportPlan::StringBackedBootstrap {
            library: String::new(),
        };
        assert!(canonical_referenced_imports(&unqualified).is_err());
    }
}
