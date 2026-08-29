//! Ownership-preserving preflight for exact ELF dynamic-link inputs.
//!
//! This module joins a final image to its target/deployment-selected
//! interpreter and canonical referenced versioned imports. It deliberately
//! stops before section planning, relocation application, or byte mutation.

use crate::imports::{ElfImportLocator, ElfImportRequest, canonical_referenced_imports};
use omega_image::FinalImage;
use omega_target::{NormalizedElfInterpreterPlan, TargetProfile};
use psi_diagnostics::Diagnostic;

/// One exact final image joined to all currently available dynamic-link inputs.
///
/// Canonical import rows remain private to the ELF owner. This carrier grants
/// no loader, publication, admission, or runnable-image authority.
#[derive(Debug)]
#[must_use = "planned ELF dynamic-link inputs retain the exact final image and interpreter"]
pub struct PlannedElfDynamicLinkInputs {
    image: FinalImage,
    interpreter: NormalizedElfInterpreterPlan,
    imports: Vec<ElfImportRequest>,
}

impl PlannedElfDynamicLinkInputs {
    pub const fn image(&self) -> &FinalImage {
        &self.image
    }

    pub const fn interpreter(&self) -> &NormalizedElfInterpreterPlan {
        &self.interpreter
    }

    pub fn referenced_import_count(&self) -> usize {
        self.imports.len()
    }

    pub(crate) fn imports(&self) -> &[ElfImportRequest] {
        &self.imports
    }

    #[allow(dead_code)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        FinalImage,
        NormalizedElfInterpreterPlan,
        Vec<ElfImportRequest>,
    ) {
        (self.image, self.interpreter, self.imports)
    }
}

/// Rejected dynamic-link input preflight with both original owned inputs.
#[derive(Debug)]
#[must_use = "ELF dynamic-link rejection retains the final image and interpreter input"]
pub struct ElfDynamicLinkInputPlanningError {
    image: FinalImage,
    interpreter: NormalizedElfInterpreterPlan,
    diagnostic: Diagnostic,
}

impl ElfDynamicLinkInputPlanningError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (FinalImage, NormalizedElfInterpreterPlan, Diagnostic) {
        (self.image, self.interpreter, self.diagnostic)
    }
}

impl std::fmt::Display for ElfDynamicLinkInputPlanningError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfDynamicLinkInputPlanningError {}

/// Join one exact final image to a matching Linux interpreter and only its
/// referenced normalized versioned-ELF import rows.
///
/// Every rejection occurs before image mutation and returns both original
/// inputs. Success remains a preflight carrier; the complete dynamic sections,
/// relocation realization, layout, and final-byte replay remain later work.
pub fn plan_elf_dynamic_link_inputs(
    image: FinalImage,
    interpreter: NormalizedElfInterpreterPlan,
) -> Result<PlannedElfDynamicLinkInputs, Box<ElfDynamicLinkInputPlanningError>> {
    let reject = |image, interpreter, diagnostic| {
        Err(Box::new(ElfDynamicLinkInputPlanningError {
            image,
            interpreter,
            diagnostic,
        }))
    };

    if interpreter.target().native_target() != image.target {
        return reject(
            image,
            interpreter,
            Diagnostic::error(
                "ELF interpreter profile does not match the exact final-image target",
            ),
        );
    }

    let imports = match canonical_referenced_imports(&image) {
        Ok(imports) => imports,
        Err(diagnostic) => return reject(image, interpreter, diagnostic),
    };
    if imports.is_empty() {
        return reject(
            image,
            interpreter,
            Diagnostic::error(
                "ELF interpreter input has no referenced normalized versioned import to join",
            ),
        );
    }

    for import in &imports {
        let ElfImportLocator::Versioned { target_profile, .. } = &import.locator else {
            return reject(
                image,
                interpreter,
                Diagnostic::error(
                    "string-backed ELF bootstrap imports cannot enter the normalized dynamic-link input plan",
                ),
            );
        };
        if *target_profile != interpreter.target()
            || !matches!(
                target_profile,
                TargetProfile::LinuxArm64 | TargetProfile::LinuxX64
            )
        {
            return reject(
                image,
                interpreter,
                Diagnostic::error(
                    "versioned ELF import profile does not match the exact interpreter profile",
                ),
            );
        }
    }

    Ok(PlannedElfDynamicLinkInputs {
        image,
        interpreter,
        imports,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_image::{
        FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
        FinalImageSection, FinalImageSymbol,
    };
    use omega_object_file::{RelocationKind, SymbolKind};
    use omega_target::{
        ForeignLocatorCandidate, NativeTarget, normalize_elf_interpreter_plan,
        normalize_foreign_locator,
    };
    use psi_arena::Handle;

    fn interpreter(target: TargetProfile, path: &[u8]) -> NormalizedElfInterpreterPlan {
        normalize_elf_interpreter_plan(path.to_vec(), target).expect("valid interpreter input")
    }

    fn image_with_import(import: FinalImageImportPlan) -> FinalImage {
        let target = match &import {
            FinalImageImportPlan::Normalized(locator) => locator.target().native_target(),
            FinalImageImportPlan::StringBackedBootstrap { .. } | FinalImageImportPlan::None => {
                NativeTarget::linux_x64()
            }
        };
        let mut image = FinalImage::with_capacity(
            target,
            FinalImageMemory {
                text: vec![0; 20],
                ..FinalImageMemory::default()
            },
            Handle::invalid(),
            1,
            1,
            2,
        );
        let imported = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "__omega_dynamic_import".into(),
            section: FinalImageSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: imported,
            import,
        });
        for offset in [3, 15] {
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

    fn versioned_import(
        target: TargetProfile,
        object: &[u8],
        symbol: &[u8],
        version: &[u8],
    ) -> FinalImageImportPlan {
        FinalImageImportPlan::Normalized(
            normalize_foreign_locator(
                ForeignLocatorCandidate::ElfVersioned {
                    object: object.to_vec(),
                    symbol: symbol.to_vec(),
                    version: version.to_vec(),
                },
                target,
            )
            .expect("valid versioned ELF locator"),
        )
    }

    #[test]
    fn exact_interpreter_and_canonical_versioned_import_rows_join_without_mutation() {
        let object = b"libraw\xff.so.6";
        let symbol = b"entry\xfe";
        let version = b"ABI_4\xfd";
        let image = image_with_import(versioned_import(
            TargetProfile::LinuxX64,
            object,
            symbol,
            version,
        ));
        let expected_image = image.clone();
        let interpreter = interpreter(TargetProfile::LinuxX64, b"/lib64/ld-linux-\xfc-x86-64.so.2");
        let expected_interpreter = interpreter.clone();

        let planned = plan_elf_dynamic_link_inputs(image, interpreter)
            .expect("matching exact dynamic-link inputs");

        assert_eq!(planned.image(), &expected_image);
        assert_eq!(planned.interpreter(), &expected_interpreter);
        assert_eq!(planned.referenced_import_count(), 1);
        assert_eq!(
            planned.imports[0].locator,
            ElfImportLocator::Versioned {
                target_profile: TargetProfile::LinuxX64,
                normalized_identity: match &expected_image
                    .symbol_table
                    .imports
                    .iter()
                    .next()
                    .expect("one import")
                    .1
                    .import
                {
                    FinalImageImportPlan::Normalized(locator) => {
                        locator.non_authoritative_compatibility_fingerprint()
                    }
                    _ => unreachable!("fixture uses a normalized import"),
                },
                object: object.to_vec(),
                symbol: symbol.to_vec(),
                version: version.to_vec(),
            },
        );
        assert_eq!(
            planned.imports[0]
                .relocations
                .iter()
                .map(|relocation| relocation.offset)
                .collect::<Vec<_>>(),
            [3, 15],
        );
    }

    #[test]
    fn target_drift_rejects_and_returns_both_original_inputs() {
        let image = image_with_import(versioned_import(
            TargetProfile::LinuxX64,
            b"libexact.so.1",
            b"entry",
            b"ABI_1",
        ));
        let expected_image = image.clone();
        let interpreter = interpreter(TargetProfile::LinuxArm64, b"/lib/ld-linux-aarch64.so.1");
        let expected_interpreter = interpreter.clone();

        let error = plan_elf_dynamic_link_inputs(image, interpreter)
            .expect_err("different architecture/profile must reject");
        let (returned_image, returned_interpreter, diagnostic) = error.into_parts();

        assert_eq!(returned_image, expected_image);
        assert_eq!(returned_interpreter, expected_interpreter);
        assert!(diagnostic.message.contains("final-image target"));
    }

    #[test]
    fn unused_interpreter_and_string_bootstrap_import_reject_recoverably() {
        let empty = FinalImage::with_capacity(
            NativeTarget::linux_x64(),
            FinalImageMemory::default(),
            Handle::invalid(),
            0,
            0,
            0,
        );
        let empty_expected = empty.clone();
        let empty_interpreter = interpreter(TargetProfile::LinuxX64, b"/loader");
        let empty_interpreter_expected = empty_interpreter.clone();
        let error = plan_elf_dynamic_link_inputs(empty, empty_interpreter)
            .expect_err("an unused interpreter input must reject");
        let (returned_image, returned_interpreter, diagnostic) = error.into_parts();
        assert_eq!(returned_image, empty_expected);
        assert_eq!(returned_interpreter, empty_interpreter_expected);
        assert!(diagnostic.message.contains("no referenced"));

        let bootstrap = image_with_import(FinalImageImportPlan::StringBackedBootstrap {
            library: "libbootstrap.so".into(),
        });
        let bootstrap_expected = bootstrap.clone();
        let bootstrap_interpreter = interpreter(TargetProfile::LinuxX64, b"/loader");
        let bootstrap_interpreter_expected = bootstrap_interpreter.clone();
        let error = plan_elf_dynamic_link_inputs(bootstrap, bootstrap_interpreter)
            .expect_err("string-backed bootstrap rows must not enter the normalized plan");
        let (returned_image, returned_interpreter, diagnostic) = error.into_parts();
        assert_eq!(returned_image, bootstrap_expected);
        assert_eq!(returned_interpreter, bootstrap_interpreter_expected);
        assert!(diagnostic.message.contains("string-backed"));
    }

    #[test]
    fn canonical_import_rejection_returns_unmodified_inputs() {
        let locator = normalize_foreign_locator(
            ForeignLocatorCandidate::ElfVersioned {
                object: b"libexact.so.1".to_vec(),
                symbol: b"entry".to_vec(),
                version: b"ABI_1".to_vec(),
            },
            TargetProfile::LinuxX64,
        )
        .expect("valid locator");
        let mut image = image_with_import(FinalImageImportPlan::Normalized(locator.clone()));
        let duplicated = image
            .symbol_table
            .imports
            .iter()
            .next()
            .expect("one import")
            .1
            .clone();
        image.symbol_table.imports.insert(duplicated);
        let expected_image = image.clone();
        let interpreter = interpreter(TargetProfile::LinuxX64, b"/loader");
        let expected_interpreter = interpreter.clone();

        let error = plan_elf_dynamic_link_inputs(image, interpreter)
            .expect_err("duplicate canonical locator rows must reject");
        let (returned_image, returned_interpreter, diagnostic) = error.into_parts();

        assert_eq!(returned_image, expected_image);
        assert_eq!(returned_interpreter, expected_interpreter);
        assert!(diagnostic.message.contains("duplicate import rows"));
    }
}
