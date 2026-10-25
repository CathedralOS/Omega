use super::super::semantics::declarations::{
    is_canonical_virtual_toolchain_path, toolchain_source_identity,
};
use crate::capture::PackageReviewInput;
use crate::record::{
    PackageReviewSourceLocation, PackageReviewSourceLocationOwner, PackageReviewSourceLocationRole,
};
use diagnostics::Diagnostic;

pub(crate) fn canonical_source_span_location(
    compilation: &PackageReviewInput<'_>,
    span: source::SourceSpan,
    role: PackageReviewSourceLocationRole,
) -> Result<PackageReviewSourceLocation, Vec<Diagnostic>> {
    if compilation
        .typed
        .symbols
        .source_map()
        .is_some_and(|sources| !sources.source_is_selected(span.source_id))
    {
        return Err(vec![Diagnostic::error(
            "reviewed declaration location does not belong to the current source activation",
        )]);
    }
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
        source::SourceOrigin::User => PackageReviewSourceLocationOwner::Package(
            source_file.package_identity.ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "reviewed package source `{}` has no reconciled package identity",
                    source_file.path.display()
                ))]
            })?,
        ),
        source::SourceOrigin::Toolchain => {
            PackageReviewSourceLocationOwner::Toolchain(toolchain_source_identity(
                source_file,
                compilation.custody.sources().exact_toolchain_sources(),
            )?)
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
    source_file: &source::SourceFile,
) -> Result<String, Vec<Diagnostic>> {
    let relative = match source_file.path.strip_prefix(&source_file.package_root) {
        Ok(relative) => relative,
        Err(_)
            if source_file.origin == source::SourceOrigin::Toolchain
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
