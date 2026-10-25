use crate::capture::PackageReviewInput;
use crate::capture::semantics::declarations::nominal_identity;
use crate::record::{PackageReviewDangerousAuthorityClass, PackageReviewNominalOwner};
use diagnostics::Diagnostic;

const CORE_CRITICAL_SERVICES: &[(&str, &str, PackageReviewDangerousAuthorityClass)] = &[
    (
        "filesystem_authority.omg",
        "FilesystemContentRead",
        PackageReviewDangerousAuthorityClass::Filesystem,
    ),
    (
        "filesystem_authority.omg",
        "FilesystemContentWrite",
        PackageReviewDangerousAuthorityClass::Filesystem,
    ),
    (
        "filesystem_authority.omg",
        "FilesystemMetadataQuery",
        PackageReviewDangerousAuthorityClass::Filesystem,
    ),
    (
        "filesystem_authority.omg",
        "FilesystemMetadataMutation",
        PackageReviewDangerousAuthorityClass::Filesystem,
    ),
    (
        "filesystem_authority.omg",
        "DirectoryEnumeration",
        PackageReviewDangerousAuthorityClass::Filesystem,
    ),
    (
        "filesystem_authority.omg",
        "FilesystemNamespaceMutation",
        PackageReviewDangerousAuthorityClass::Filesystem,
    ),
    (
        "process_authority.omg",
        "ProcessInput",
        PackageReviewDangerousAuthorityClass::Process,
    ),
    (
        "process_authority.omg",
        "ProcessOutput",
        PackageReviewDangerousAuthorityClass::Process,
    ),
    (
        "process_exit.omg",
        "ProcessExit",
        PackageReviewDangerousAuthorityClass::Process,
    ),
    (
        "assembly.omg",
        "MachineControl",
        PackageReviewDangerousAuthorityClass::MachineControl,
    ),
    (
        "assembly.omg",
        "PortIo",
        PackageReviewDangerousAuthorityClass::PortIo,
    ),
    (
        "assembly.omg",
        "InterruptControl",
        PackageReviewDangerousAuthorityClass::InterruptControl,
    ),
    (
        "interrupt.omg",
        "InterruptEntry",
        PackageReviewDangerousAuthorityClass::InterruptEntry,
    ),
    (
        "assembly.omg",
        "RootMemoryAccess",
        PackageReviewDangerousAuthorityClass::RootMemory,
    ),
];

/// Critical semantic services and exact compiler-bound aggregate services.
///
/// Core classifications require exact checked toolchain ownership and module
/// identity. Compiler-resolved aggregate bindings retain their conservative
/// summaries while those services remain in use. Neither classification is
/// evidence of exercised physical authority; realization records that separately.
pub(crate) fn dangerous_authority_class(
    compilation: &PackageReviewInput<'_>,
    definition: &language_semantics::ServiceReachDefinition,
) -> Result<Option<PackageReviewDangerousAuthorityClass>, Vec<Diagnostic>> {
    let identity = nominal_identity(compilation, definition.symbol)?;
    if matches!(
        identity.owner(),
        PackageReviewNominalOwner::ToolchainSource(_)
    ) {
        let source = compilation
            .typed
            .symbols
            .symbol_provenance_source_span(definition.symbol)
            .and_then(|span| compilation.typed.symbols.source_file(span))
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "critical service has no retained toolchain source",
                )]
            })?;
        if source
            .package_root
            .file_name()
            .is_some_and(|namespace| namespace == "core")
            // Virtual toolchain declarations (including Build operations) have
            // digest custody but are not filesystem modules in omega::core.
            && !crate::capture::semantics::declarations::is_canonical_virtual_toolchain_path(&source.path)
        {
            let relative = source
                .path
                .strip_prefix(&source.package_root)
                .map_err(|_| {
                    vec![Diagnostic::error(
                        "critical service source is outside its canonical core root",
                    )]
                })?;
            if let Some((_, _, class)) =
                CORE_CRITICAL_SERVICES
                    .iter()
                    .find(|(path, declaration, _)| {
                        relative == std::path::Path::new(path) && identity.path() == *declaration
                    })
            {
                return Ok(Some(*class));
            }
        }
    }
    if compilation
        .custody
        .execution()
        .resolved_semantic_binding(
            package_compilation::AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        )
        .is_some_and(|binding| binding.declaration_symbol() == definition.symbol)
    {
        return Ok(Some(PackageReviewDangerousAuthorityClass::Process));
    }
    if compilation
        .custody
        .execution()
        .resolved_semantic_binding(
            package_compilation::AcceptedSemanticBindingRole::FilesystemHostService,
        )
        .is_some_and(|binding| binding.declaration_symbol() == definition.symbol)
    {
        return Ok(Some(PackageReviewDangerousAuthorityClass::Filesystem));
    }

    Ok(None)
}
