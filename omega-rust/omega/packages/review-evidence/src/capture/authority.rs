use crate::capture::PackageReviewInput;
use crate::record::PackageReviewDangerousAuthorityClass;

/// Transitional compiler-owned risk classification for exact semantic bindings.
///
/// Physical terminal authority is classified independently from exact exercised
/// mechanisms during realization. Package review retains only the broad legacy
/// summaries backed by compiler-resolved Console and FilesystemHost bindings;
/// service names, source paths, and toolchain provenance cannot manufacture a
/// physical authority class.
pub(crate) fn dangerous_authority_class(
    compilation: &PackageReviewInput<'_>,
    definition: &language_semantics::ServiceReachDefinition,
) -> Option<PackageReviewDangerousAuthorityClass> {
    if compilation
        .custody
        .execution()
        .resolved_semantic_binding(
            package_compilation::AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        )
        .is_some_and(|binding| binding.declaration_symbol() == definition.symbol)
    {
        return Some(PackageReviewDangerousAuthorityClass::Process);
    }
    if compilation
        .custody
        .execution()
        .resolved_semantic_binding(
            package_compilation::AcceptedSemanticBindingRole::FilesystemHostService,
        )
        .is_some_and(|binding| binding.declaration_symbol() == definition.symbol)
    {
        return Some(PackageReviewDangerousAuthorityClass::Filesystem);
    }

    None
}
