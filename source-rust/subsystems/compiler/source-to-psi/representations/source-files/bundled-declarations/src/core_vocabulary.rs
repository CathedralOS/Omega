use crate::sealed_intrinsics::SealedDeclarationSite;
use language_core::core_vocabulary::{
    CoreSource, CoreVocabulary, CoreVocabularyCatalog as Catalog,
};
use std::path::Path;

pub struct CoreVocabularyCatalog;

impl CoreVocabularyCatalog {
    pub fn classify(site: SealedDeclarationSite<'_>) -> Option<CoreVocabulary> {
        Catalog::match_declaration(site.file, site.declaration, |file| {
            crate::bytes(file) == site.source
        })
    }

    pub fn source(file: &Path, source: &[u8]) -> Option<CoreSource> {
        Catalog::match_source(file, |file| crate::bytes(file) == source)
    }
}

#[cfg(test)]
mod tests {
    use super::CoreVocabularyCatalog;
    use super::*;
    use language_core::bundled_files::BundledFile;
    use language_core::core_vocabulary::*;

    #[test]
    fn filesystem_services_require_the_exact_bundled_declaration() {
        for service in FilesystemService::ALL {
            let vocabulary = CoreVocabulary::FilesystemService(service);
            let coordinates = Catalog::declaration(vocabulary).unwrap();
            let declared = SealedDeclarationSite {
                file: coordinates.file,
                declaration: coordinates.declaration,
                source: crate::bytes(coordinates.source),
            };
            assert_eq!(CoreVocabularyCatalog::classify(declared), Some(vocabulary));
            for changed in [
                site("other.omg", declared.declaration, declared.source),
                site("filesystem_authority.omg", "Lookalike", declared.source),
                site("filesystem_authority.omg", declared.declaration, b"edited"),
            ] {
                assert_eq!(CoreVocabularyCatalog::classify(changed), None);
            }
        }
    }

    fn site<'a>(
        file: &'a str,
        declaration: &'a str,
        source: &'a [u8],
    ) -> SealedDeclarationSite<'a> {
        SealedDeclarationSite {
            file: Path::new(file),
            declaration,
            source,
        }
    }

    #[test]
    fn never_identifies_only_the_exact_bundled_ordinary_declaration() {
        assert_eq!(
            CoreVocabularyCatalog::classify(site(
                "never.omg",
                "Never",
                crate::bytes(BundledFile::CoreNever)
            )),
            Some(CoreVocabulary::Never)
        );
        for candidate in [
            site("other.omg", "Never", crate::bytes(BundledFile::CoreNever)),
            site("never.omg", "Other", crate::bytes(BundledFile::CoreNever)),
            site("never.omg", "Never", b"public data Never {}"),
        ] {
            assert_eq!(CoreVocabularyCatalog::classify(candidate), None);
        }
    }

    #[test]
    fn rows_match_only_their_exact_file_declaration_and_source() {
        assert_eq!(
            CoreVocabularyCatalog::classify(site(
                "assembly.omg",
                "MachineControl",
                crate::bytes(BundledFile::CoreAssembly)
            )),
            Some(CoreVocabulary::MachineControlService)
        );
        assert_eq!(
            CoreVocabularyCatalog::classify(site(
                "data_segment.omg",
                "read_gs_selector",
                crate::bytes(BundledFile::CoreDataSegment)
            )),
            Some(CoreVocabulary::SegmentSelector(
                SegmentSelectorOperation::ReadGs
            ))
        );
        // A selector name in the other segment file is not that primitive.
        assert_eq!(
            CoreVocabularyCatalog::classify(site(
                "code_segment.omg",
                "read_gs_selector",
                crate::bytes(BundledFile::CoreCodeSegment)
            )),
            None
        );
        assert_eq!(
            CoreVocabularyCatalog::classify(site("assembly.omg", "MachineControl", b"edited")),
            None
        );
        assert_eq!(
            CoreVocabularyCatalog::source(
                Path::new("atomic.omg"),
                crate::bytes(BundledFile::CoreAtomic)
            ),
            Some(CoreSource::Atomic)
        );
        assert_eq!(
            CoreVocabularyCatalog::source(Path::new("atomic.omg"), b"edited"),
            None
        );
        assert_eq!(
            CoreSource::from_file(CoreSource::Layout.file()),
            Some(CoreSource::Layout)
        );
        assert_eq!(
            CoreVocabularyCatalog::classify(site(
                "extent.omg",
                "Resident",
                crate::bytes(BundledFile::CoreExtent)
            )),
            Some(CoreVocabulary::ExtentDomain(ExtentDomain::Resident))
        );
        // `reflect.omg` declares `module reflect`: rows use the full
        // package path, and the bare name matches nothing.
        assert_eq!(
            CoreVocabularyCatalog::classify(site(
                "reflect.omg",
                "reflect::inspect_record_fields",
                crate::bytes(BundledFile::CoreReflect)
            )),
            Some(CoreVocabulary::Reflection(
                ReflectionDeclaration::InspectRecordFields
            ))
        );
        assert_eq!(
            CoreVocabularyCatalog::classify(site(
                "reflect.omg",
                "inspect_record_fields",
                crate::bytes(BundledFile::CoreReflect)
            )),
            None
        );
        // The integer carrier query is one exact declaration of `integer.omg`;
        // the family queries built on it are ordinary machines with no row.
        assert_eq!(
            CoreVocabularyCatalog::classify(site(
                "integer.omg",
                "integer::carrier",
                crate::bytes(BundledFile::CoreInteger)
            )),
            Some(CoreVocabulary::IntegerCarrier(
                IntegerCarrierDeclaration::Query
            ))
        );
        for unrecognized in ["carrier", "integer::is_integer", "integer::is_signed"] {
            assert_eq!(
                CoreVocabularyCatalog::classify(site(
                    "integer.omg",
                    unrecognized,
                    crate::bytes(BundledFile::CoreInteger)
                )),
                None,
                "{unrecognized}"
            );
        }
        assert_eq!(
            CoreVocabularyCatalog::classify(site("integer.omg", "integer::carrier", b"edited")),
            None
        );
        assert_eq!(
            CoreVocabularyCatalog::classify(site(
                "reflect.omg",
                "core::reflect::inspect_record_fields",
                crate::bytes(BundledFile::CoreReflect)
            )),
            None
        );
    }
    #[test]
    fn never_recognition_requires_its_unchanged_core_declaration() {
        let source = crate::bytes(BundledFile::CoreNever);
        assert_eq!(
            CoreVocabularyCatalog::classify(site("never.omg", "Never", source)),
            Some(CoreVocabulary::Never),
        );
        assert_eq!(
            CoreVocabularyCatalog::classify(site("never.omg", "Other", source)),
            None
        );
        assert_eq!(
            CoreVocabularyCatalog::classify(site("never.omg", "Never", b"public data Never {}")),
            None
        );
    }
}
