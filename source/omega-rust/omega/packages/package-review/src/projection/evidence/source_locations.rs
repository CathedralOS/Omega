use super::super::exact_identity::nominal_identities::{
    is_canonical_virtual_toolchain_path, toolchain_source_identity,
};
use crate::evidence::projection::{
    PackageReviewCanonicalRowSources, ProjectedNestedSourceLocation,
};
use crate::evidence::{
    PackageReviewSourceLocation, PackageReviewSourceLocationOwner, PackageReviewSourceLocationRole,
};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) const MAX_PACKAGE_REVIEW_SOURCE_LOCATIONS: usize = 262_144;
pub(crate) const MAX_PACKAGE_REVIEW_SOURCE_LOCATION_PATH_BYTES: usize = 16 * 1024 * 1024;

pub(crate) fn validate_canonical_row_source_limits(
    sources: &PackageReviewCanonicalRowSources,
) -> Result<(), Vec<Diagnostic>> {
    let all = sources
        .public_traits
        .iter()
        .chain(&sources.public_conformances)
        .chain(&sources.public_domains)
        .chain(&sources.public_propositions)
        .chain(&sources.public_consts)
        .chain(&sources.public_operators)
        .chain(&sources.public_data)
        .chain(&sources.representation_tcb)
        .chain(&sources.semantic_dependencies)
        .chain(&sources.callables)
        .chain(&sources.external_executable_supply)
        .chain(&sources.dangerous_authorities)
        .chain(&sources.dangerous_authority_slack)
        .chain(std::iter::once(&sources.selected_provider_set));
    let mut count = 0usize;
    let mut path_bytes = 0usize;
    for source in all {
        let locations = &source.authored_locations;
        let derivations = &source.compiler_derivations;
        if locations.is_empty() && derivations.is_empty() {
            return Err(vec![Diagnostic::error(
                "package review row has neither authored source locations nor a compiler derivation",
            )]);
        }
        if locations.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(vec![Diagnostic::error(
                "authored package review source locations are not strictly canonical",
            )]);
        }
        if derivations.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(vec![Diagnostic::error(
                "package review compiler derivations are not strictly canonical",
            )]);
        }
        count = count.checked_add(locations.len()).ok_or_else(|| {
            vec![Diagnostic::error(
                "package review source-location count overflow",
            )]
        })?;
        for location in locations {
            path_bytes = path_bytes
                .checked_add(location.relative_path.len())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "package review source-location path-byte count overflow",
                    )]
                })?;
        }
    }
    if count > MAX_PACKAGE_REVIEW_SOURCE_LOCATIONS {
        return Err(vec![Diagnostic::error(
            "package review exceeds the source-location count ceiling",
        )]);
    }
    if path_bytes > MAX_PACKAGE_REVIEW_SOURCE_LOCATION_PATH_BYTES {
        return Err(vec![Diagnostic::error(
            "package review exceeds the source-location path-byte ceiling",
        )]);
    }
    Ok(())
}

pub(crate) fn canonical_source_location(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
    mut role: PackageReviewSourceLocationRole,
) -> Result<PackageReviewSourceLocation, Vec<Diagnostic>> {
    if compilation
        .typed
        .symbols
        .symbol_source_span(symbol)
        .is_none()
    {
        role = PackageReviewSourceLocationRole::DerivationOrigin;
    }
    let span = compilation
        .typed
        .symbols
        .symbol_provenance_source_span(symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed declaration `{}` has no authored source span",
                compilation.typed.symbols.display_path(symbol, "::")
            ))]
        })?;
    canonical_source_span_location(compilation, span, role)
}

pub(crate) fn project_nested_declaration_source_location(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
    authored_role: PackageReviewSourceLocationRole,
    subject: &str,
) -> Result<ProjectedNestedSourceLocation, Vec<Diagnostic>> {
    let (source_span, role) = match compilation.typed.symbols.symbol_source_span(symbol) {
        Some(source_span) => (source_span, authored_role),
        None => (
            compilation
                .typed
                .symbols
                .symbol_provenance_source_span(symbol)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "{subject} has neither an authored declaration span nor a derivation origin"
                    ))]
                })?,
            PackageReviewSourceLocationRole::DerivationOrigin,
        ),
    };
    Ok(ProjectedNestedSourceLocation { source_span, role })
}

pub(crate) fn canonical_source_span_location(
    compilation: &CheckedCompilation,
    span: psi_source::SourceSpan,
    role: PackageReviewSourceLocationRole,
) -> Result<PackageReviewSourceLocation, Vec<Diagnostic>> {
    let source_file = compilation.typed.symbols.source_file(span).ok_or_else(|| {
        vec![Diagnostic::error(
            "reviewed declaration source span has no retained source file",
        )]
    })?;
    if span.span.start >= span.span.end
        || span.span.end > source_file.source.len()
        || !source_file.source.is_char_boundary(span.span.start)
        || !source_file.source.is_char_boundary(span.span.end)
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed declaration source span is outside `{}`",
            source_file.path.display()
        ))]);
    }
    let owner = match source_file.origin {
        psi_source::SourceOrigin::User => PackageReviewSourceLocationOwner::Package(
            source_file.package_identity.ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "reviewed package source `{}` has no reconciled package identity",
                    source_file.path.display()
                ))]
            })?,
        ),
        psi_source::SourceOrigin::Toolchain => {
            PackageReviewSourceLocationOwner::Toolchain(toolchain_source_identity(source_file)?)
        }
    };
    let relative_path = canonical_review_relative_path(source_file)?;
    Ok(PackageReviewSourceLocation {
        owner,
        relative_path,
        start_byte: u64::try_from(span.span.start).expect("retained source byte offset fits u64"),
        end_byte: u64::try_from(span.span.end).expect("retained source byte offset fits u64"),
        role,
    })
}

pub(crate) fn canonical_review_relative_path(
    source_file: &psi_source::SourceFile,
) -> Result<String, Vec<Diagnostic>> {
    let relative = match source_file.path.strip_prefix(&source_file.package_root) {
        Ok(relative) => relative,
        Err(_)
            if source_file.origin == psi_source::SourceOrigin::Toolchain
                && is_canonical_virtual_toolchain_path(&source_file.path) =>
        {
            source_file.path.as_path()
        }
        Err(_) => {
            return Err(vec![Diagnostic::error(format!(
                "reviewed source `{}` is outside its retained package root `{}`",
                source_file.path.display(),
                source_file.package_root.display()
            ))]);
        }
    };
    let mut path = String::new();
    for component in relative.components() {
        let std::path::Component::Normal(component) = component else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed source `{}` has a non-canonical relative path",
                source_file.path.display()
            ))]);
        };
        let component = component.to_str().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed source `{}` has a non-UTF-8 relative path component",
                source_file.path.display()
            ))]
        })?;
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(component);
    }
    if path.is_empty() {
        return Err(vec![Diagnostic::error(
            "reviewed source location requires a non-empty relative path",
        )]);
    }
    Ok(path)
}
