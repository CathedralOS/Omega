use language_core::bundled_files::BundledFile;
use language_core::sealed_intrinsics::{SealedIntrinsic, SealedIntrinsicCatalog as Catalog};
use std::path::Path;

/// A declaration already known to come from a toolchain source file.
#[derive(Debug, Clone, Copy)]
pub struct SealedDeclarationSite<'a> {
    /// The file path relative to its package root.
    pub file: &'a Path,
    /// The declaration path inside the package, segments joined by `::`.
    pub declaration: &'a str,
    /// The complete source bytes of the file as loaded.
    pub source: &'a [u8],
}

pub struct SealedIntrinsicCatalog;

impl SealedIntrinsicCatalog {
    pub fn classify(site: SealedDeclarationSite<'_>) -> Option<SealedIntrinsic> {
        Catalog::match_declaration(site.file, site.declaration, |file| {
            crate::bytes(file) == site.source
        })
    }
}
/// Exact source bytes whose canonical declarations own float operation contracts.
/// A matching name outside these bytes grants no numeric primitive custody.
pub fn float_operations_source() -> (&'static str, &'static [u8]) {
    (
        "float_operations.omg",
        crate::bytes(BundledFile::CoreFloatOperations),
    )
}

/// Exact source coordinates for the hosted primitive and its public contract.
/// Consumers still authenticate source custody and the complete signature.
pub fn hosted_process_exit_sources() -> [(&'static str, &'static str, &'static [u8]); 3] {
    [
        (
            "process_exit_native.omg",
            "ProcessExitPrimitive",
            crate::bytes(BundledFile::CoreProcessExitNative),
        ),
        (
            "process_exit.omg",
            "ProcessExit",
            crate::bytes(BundledFile::CoreProcessExit),
        ),
        ("never.omg", "Never", crate::bytes(BundledFile::CoreNever)),
    ]
}

#[cfg(test)]
mod tests {
    use super::SealedIntrinsicCatalog;
    use super::*;
    use language_core::byte_predicates::ByteSequencePredicate;
    use language_core::sealed_intrinsics::*;

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
    fn hosted_exit_owners_name_the_sealed_primitive_and_requirement() {
        let [primitive, requirement, _] = hosted_process_exit_sources();
        for ((file, _, bytes), member, intrinsic) in [
            (
                primitive,
                primitive.1,
                SealedIntrinsic::HostedPrimitive(HostedOperation::ProcessExit),
            ),
            (
                requirement,
                "ProcessExit::exit_process",
                SealedIntrinsic::HostedRequirement(HostedOperation::ProcessExit),
            ),
        ] {
            assert_eq!(
                SealedIntrinsicCatalog::classify(site(file, member, bytes)),
                Some(intrinsic)
            );
        }
    }

    #[test]
    fn byte_observations_match_only_exact_bundled_declaration_sites() {
        // This lower-level catalog assumes toolchain provenance. The symbol
        // gate separately proves that the source was loaded from the toolchain.
        for (declaration, predicate) in [
            ("valid_utf8", ByteSequencePredicate::ValidUtf8),
            ("no_nul", ByteSequencePredicate::NoNul),
            ("ascii_only", ByteSequencePredicate::AsciiOnly),
            ("non_empty", ByteSequencePredicate::NonEmpty),
        ] {
            assert_eq!(
                SealedIntrinsicCatalog::classify(site(
                    "byte_observation/byte_observation.omg",
                    declaration,
                    crate::bytes(BundledFile::CoreByteObservation),
                )),
                Some(SealedIntrinsic::CompilerPrimitive(
                    CompilerPrimitive::ByteObservation(predicate),
                )),
            );
            for file in ["copied/byte_observation.omg", "other.omg"] {
                assert_eq!(
                    SealedIntrinsicCatalog::classify(site(
                        file,
                        declaration,
                        crate::bytes(BundledFile::CoreByteObservation),
                    )),
                    None,
                );
            }
            let mut edited = crate::bytes(BundledFile::CoreByteObservation).to_vec();
            edited.push(b' ');
            assert_eq!(
                SealedIntrinsicCatalog::classify(site(
                    "byte_observation/byte_observation.omg",
                    declaration,
                    &edited,
                )),
                None,
            );
            let foreign_member = format!("Lookalike::{declaration}");
            assert_eq!(
                SealedIntrinsicCatalog::classify(site(
                    "byte_observation/byte_observation.omg",
                    &foreign_member,
                    crate::bytes(BundledFile::CoreByteObservation),
                )),
                None,
            );
        }
        assert_eq!(
            SealedIntrinsicCatalog::classify(site(
                "byte_observation/byte_observation.omg",
                "unknown_predicate",
                crate::bytes(BundledFile::CoreByteObservation),
            )),
            None,
        );
    }

    #[test]
    fn private_console_adapters_have_no_hosted_catalog_identity() {
        for member in ["read_byte", "write_byte", "exit_process"] {
            let requirement = format!("ConsoleHost::{member}");
            assert_eq!(
                SealedIntrinsicCatalog::classify(site(
                    "console.omg",
                    &requirement,
                    crate::bytes(BundledFile::StdConsole)
                )),
                None,
            );
            let mut edited = crate::bytes(BundledFile::StdConsole).to_vec();
            edited.push(b' ');
            assert_eq!(
                SealedIntrinsicCatalog::classify(site("console.omg", &requirement, &edited)),
                None,
            );
            assert_eq!(
                SealedIntrinsicCatalog::classify(site(
                    "copied/console.omg",
                    &requirement,
                    crate::bytes(BundledFile::StdConsole)
                )),
                None,
            );
        }
    }

    #[test]
    fn rows_match_only_their_exact_file_declaration_and_source() {
        for placement in [
            "layout_plan_place_private",
            "layout_plan_place_private_field",
        ] {
            assert_eq!(
                SealedIntrinsicCatalog::classify(site(
                    "layout.omg",
                    placement,
                    crate::bytes(BundledFile::CoreLayout)
                )),
                Some(SealedIntrinsic::PrivateLayoutPlacement)
            );
        }
        assert_eq!(
            SealedIntrinsicCatalog::classify(site(
                "layout.omg",
                "layout_plan_place_private",
                b"authored lookalike"
            )),
            None
        );
        assert_eq!(
            SealedIntrinsicCatalog::classify(site(
                "task.omg",
                "TaskRuntime::start",
                crate::bytes(BundledFile::CoreTask)
            )),
            Some(SealedIntrinsic::TaskRuntimeStart)
        );
        assert_eq!(
            SealedIntrinsicCatalog::classify(site(
                "float_provider.omg",
                "NativeFloats::float_add_f32_f32",
                crate::bytes(BundledFile::CoreFloatProvider)
            )),
            Some(SealedIntrinsic::NativeFloatOperation)
        );
        // A lookalike file, an edited source, a nested member, or a bare
        // owner name supplies no custody.
        // Canonical primitive bytes cannot revive a retired target path.
        for file in [
            "targets/linux_x86_64/process_exit_impl.omg",
            "targets/linux_arm64/process_exit_impl.omg",
        ] {
            assert_eq!(
                SealedIntrinsicCatalog::classify(site(
                    file,
                    "ProcessExitPrimitive",
                    crate::bytes(BundledFile::CoreProcessExitNative)
                )),
                None
            );
        }
        assert_eq!(
            SealedIntrinsicCatalog::classify(site(
                "tasks.omg",
                "TaskRuntime::start",
                crate::bytes(BundledFile::CoreTask)
            )),
            None
        );
        assert_eq!(
            SealedIntrinsicCatalog::classify(site("task.omg", "TaskRuntime::start", b"edited")),
            None
        );
        assert_eq!(
            SealedIntrinsicCatalog::classify(site(
                "float_provider.omg",
                "NativeFloats::a::b",
                crate::bytes(BundledFile::CoreFloatProvider)
            )),
            None
        );
        // The map itself is its own row; a bare member owner is not.
        assert_eq!(
            SealedIntrinsicCatalog::classify(site(
                "float_provider.omg",
                "NativeFloats",
                crate::bytes(BundledFile::CoreFloatProvider)
            )),
            Some(SealedIntrinsic::NativeFloatMap)
        );
        assert_eq!(
            SealedIntrinsicCatalog::classify(site(
                "float_operations.omg",
                "Float",
                crate::bytes(BundledFile::CoreFloatOperations)
            )),
            None
        );
    }
}
