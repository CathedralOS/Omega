//! Highlight the critical services of the selected Product, including capability-only traits.
use super::{declarations::unavailable, input::PolicySource, semantic_bindings::checked_service};
use diagnostics::Diagnostic;
use provider_acceptance::AcceptedSemanticBindingRole as Role;
use review_evidence::record::{
    PackageReviewDangerousAuthorityClass as Class, PackageReviewNominalIdentity,
};
use symbols::SymbolHandle;

const CORE_SERVICES: &[(&str, &str, Class)] = &[
    (
        "filesystem_authority.omg",
        "FilesystemContentRead",
        Class::Filesystem,
    ),
    (
        "filesystem_authority.omg",
        "FilesystemContentWrite",
        Class::Filesystem,
    ),
    (
        "filesystem_authority.omg",
        "FilesystemMetadataQuery",
        Class::Filesystem,
    ),
    (
        "filesystem_authority.omg",
        "FilesystemMetadataMutation",
        Class::Filesystem,
    ),
    (
        "filesystem_authority.omg",
        "DirectoryEnumeration",
        Class::Filesystem,
    ),
    (
        "filesystem_authority.omg",
        "FilesystemNamespaceMutation",
        Class::Filesystem,
    ),
    ("process_authority.omg", "ProcessInput", Class::Process),
    ("process_authority.omg", "ProcessOutput", Class::Process),
    ("process_exit.omg", "ProcessExit", Class::Process),
    ("assembly.omg", "MachineControl", Class::MachineControl),
    ("assembly.omg", "PortIo", Class::PortIo),
    ("assembly.omg", "InterruptControl", Class::InterruptControl),
    ("interrupt.omg", "InterruptEntry", Class::InterruptEntry),
    ("assembly.omg", "RootMemoryAccess", Class::RootMemory),
];

/// The checked symbol set already authenticates which source bytes own the declaration.
/// Restrict the core role further to its exact file and unqualified declaration path.
pub(super) fn core_service(
    input: &PolicySource<'_>,
    symbol: SymbolHandle,
) -> Result<Option<Class>, Vec<Diagnostic>> {
    let symbols = &input.typed().symbols;
    if symbols.symbol_package_identity(symbol).is_some() {
        return Ok(None);
    }
    let Some(source) = symbols
        .symbol_provenance_source_span(symbol)
        .and_then(|span| symbols.source_file(span))
        .filter(|source| source.origin.is_toolchain())
    else {
        return Ok(None);
    };
    let path = symbols.display_path(symbol, "::");
    let Some((file, _, class)) = CORE_SERVICES.iter().find(|(_, name, _)| *name == path) else {
        return Ok(None);
    };
    let unit = source_admission::source_identity::consumed_source_unit(source, false)
        .map_err(unavailable)?;
    if unit.toolchain_namespace() == Some("core")
        && matches!(unit.relative_path(), [name] if name.as_str() == *file)
    {
        input.nominal(symbol)?;
        return Ok(Some(*class));
    }
    Ok(None)
}

pub(super) fn critical_services(
    input: &PolicySource<'_>,
    selected: &compiler::SelectedProviderResult,
) -> Result<Vec<(PackageReviewNominalIdentity, Class)>, Vec<Diagnostic>> {
    let mut classes = Vec::new();
    for declaration in input.typed().traits().iter().filter(|d| d.is_boundary) {
        if let Some(class) = core_service(input, declaration.symbol)? {
            classes.push((input.nominal(declaration.symbol)?, class));
        }
    }
    for binding in input.sources().accepted_semantic_bindings() {
        let class = match binding.role() {
            Role::ConsoleIo | Role::ProcessExitExitProcessI32 => Class::Process,
            Role::FilesystemHostService => Class::Filesystem,
            _ => continue,
        };
        let (symbol, _) = checked_service(input, selected, binding)?;
        classes.push((input.nominal(symbol)?, class));
    }
    classes.sort();
    classes.dedup();
    Ok(classes)
}
