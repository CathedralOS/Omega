// Reserved toolchain mount recognition and exact captured-path normalization.
// Authored imports select logical names; no filename-prefix lookup lives here.

use diagnostics::Diagnostic;
use std::path::{Path, PathBuf};
use syntax_trees::identifier::Identifier;
use syntax_trees::identifier::JoinedSyntaxPath;

pub(super) fn is_bundled_omega_path(
    syntax_trees: &syntax_trees::SyntaxTrees,
    path: &[Identifier],
) -> bool {
    path.first()
        .is_some_and(|segment| segment.as_str(syntax_trees) == "omega")
        && path
            .get(1)
            .is_some_and(|segment| segment.as_str(syntax_trees) == "language")
}

pub(super) fn is_bundled_core_path(
    syntax_trees: &syntax_trees::SyntaxTrees,
    path: &[Identifier],
) -> bool {
    is_bundled_omega_path(syntax_trees, path)
        && path
            .get(2)
            .is_some_and(|segment| segment.as_str(syntax_trees) == "core")
}

pub(super) fn identifier_path_text(
    syntax_trees: &syntax_trees::SyntaxTrees,
    path: &[Identifier],
) -> String {
    path.joined_path_in(syntax_trees)
}

pub(crate) fn bundled_omega_root() -> Result<PathBuf, Vec<Diagnostic>> {
    crate::source::toolchain_library::root().map_err(|error| vec![Diagnostic::error(error)])
}

pub fn bundled_core_root() -> Result<PathBuf, Vec<Diagnostic>> {
    Ok(bundled_omega_root()?.join("core"))
}

pub(super) fn normalize_path(
    path: &Path,
    filesystem: &source::SourceFileSystem,
) -> Result<PathBuf, Vec<Diagnostic>> {
    filesystem.canonicalize(path).map_err(|error| {
        vec![Diagnostic::error(format!(
            "failed to resolve {}: {error}",
            path.display()
        ))]
    })
}
