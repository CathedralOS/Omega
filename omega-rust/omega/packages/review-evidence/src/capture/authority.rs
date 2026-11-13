/*
Dangerous authority classification for critical `omega::core` services and
compiler-bound aggregate host services (`dangerous_authority_class`).

Core classifications require exact checked toolchain ownership and canonical
module path identity (`filesystem_authority.omg`, `process_authority.omg`,
`process_exit.omg`, `assembly.omg`, `interrupt.omg`), excluding virtual
toolchain declarations such as `Build` operations. Compiler-resolved aggregate
bindings (`ConsoleExitProcessI32`, `FilesystemHostService`) map to their
conservative authority classes while those services remain in use.
*/

use super::published_input::PublishedPackageReviewInput;
use super::published_projection::nominal;
use crate::capture::EvidenceOption;
use crate::record::{PackageReviewDangerousAuthorityClass, PackageReviewNominalOwner};
use diagnostics::Diagnostic;
use provider_planning::service_declarations::ServiceDeclarations;

macro_rules! r {
    ($a:expr => $b:expr => $c:ident) => {
        ($a, $b, PackageReviewDangerousAuthorityClass::$c)
    };
}

const CORE_CRITICAL_SERVICES: &[(&str, &str, PackageReviewDangerousAuthorityClass)] = &[
    r!("filesystem_authority.omg" => "FilesystemContentRead" => Filesystem),
    r!("filesystem_authority.omg" => "FilesystemContentWrite" => Filesystem),
    r!("filesystem_authority.omg" => "FilesystemMetadataQuery" => Filesystem),
    r!("filesystem_authority.omg" => "FilesystemMetadataMutation" => Filesystem),
    r!("filesystem_authority.omg" => "DirectoryEnumeration" => Filesystem),
    r!("filesystem_authority.omg" => "FilesystemNamespaceMutation" => Filesystem),
    r!("process_authority.omg" => "ProcessInput" => Process),
    r!("process_authority.omg" => "ProcessOutput" => Process),
    r!("process_exit.omg" => "ProcessExit" => Process),
    r!("assembly.omg" => "MachineControl" => MachineControl),
    r!("assembly.omg" => "PortIo" => PortIo),
    r!("assembly.omg" => "InterruptControl" => InterruptControl),
    r!("interrupt.omg" => "InterruptEntry" => InterruptEntry),
    r!("assembly.omg" => "RootMemoryAccess" => RootMemory),
];

pub(crate) fn dangerous_authority_class(
    compilation: &PublishedPackageReviewInput<'_>,
    services: &ServiceDeclarations,
    definition: &language_semantics::ServiceReachDefinition,
) -> Result<Option<PackageReviewDangerousAuthorityClass>, Vec<Diagnostic>> {
    let identity = nominal(*compilation, definition.symbol)?;
    if matches!(
        identity.owner(),
        PackageReviewNominalOwner::ToolchainSource(_)
    ) {
        let source = compilation
            .candidate
            .symbols
            .symbol_provenance_source_span(definition.symbol)
            .and_then(|span| compilation.candidate.symbols.source_file(span))
            .or_evidence("critical service has no retained toolchain source")?;
        if source
            .package_root
            .file_name()
            .is_some_and(|namespace| namespace == "core")
            && !super::nominal::is_canonical_virtual_toolchain_path(&source.path)
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
    for (role, class) in [
        (
            package_compilation::AcceptedSemanticBindingRole::ConsoleExitProcessI32,
            PackageReviewDangerousAuthorityClass::Process,
        ),
        (
            package_compilation::AcceptedSemanticBindingRole::FilesystemHostService,
            PackageReviewDangerousAuthorityClass::Filesystem,
        ),
    ] {
        if let Some(binding) = compilation
            .custody
            .execution()
            .resolved_semantic_binding(role)
        {
            if services
                .identifies(definition.symbol, binding.declaration())
                .map_err(|diagnostic| vec![diagnostic])?
            {
                return Ok(Some(class));
            }
        }
    }

    Ok(None)
}
