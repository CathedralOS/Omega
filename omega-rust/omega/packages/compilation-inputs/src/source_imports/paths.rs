//! Physical source candidates and containment checks for import resolution.

use diagnostics::Diagnostic;
use std::path::{Path, PathBuf};
use syntax_trees::identifier::Identifier;
use syntax_trees::identifier::JoinedSyntaxPath;

pub(super) fn resolve_source_path(
    syntax_trees: &syntax_trees::SyntaxTrees,
    root_dir: &Path,
    source_path: &[Identifier],
    filesystem: &source::SourceFileSystem,
) -> Result<PathBuf, Vec<Diagnostic>> {
    let mut segments = source_path.iter();
    let mut path = if is_bundled_omega_path(syntax_trees, source_path) {
        segments.next();
        segments.next();
        bundled_omega_root()?
    } else {
        root_dir.to_path_buf()
    };

    let relative_path = segments.fold(PathBuf::new(), |mut relative_path, segment| {
        relative_path.push(segment.as_str(syntax_trees));
        relative_path
    });
    for relative_candidate in source_import_candidates(&relative_path) {
        let candidate = path.join(relative_candidate);
        if filesystem.is_file(&candidate) {
            return Ok(candidate);
        }
    }
    path.push(relative_path);
    Ok(source_path_candidates(&path)
        .into_iter()
        .next()
        .unwrap_or(path))
}

pub(super) fn resolve_reconciled_import(
    syntax_trees: &syntax_trees::SyntaxTrees,
    expected_root: PathBuf,
    source_path: &[Identifier],
    source_kind: &str,
    filesystem: &source::SourceFileSystem,
) -> Result<PathBuf, Vec<Diagnostic>> {
    let mut relative_path = PathBuf::new();
    for segment in source_path {
        relative_path.push(segment.as_str(syntax_trees));
    }

    resolve_reconciled_relative_import(expected_root, &relative_path, source_kind, filesystem)
}

pub(super) fn resolve_reconciled_relative_import(
    expected_root: PathBuf,
    relative_path: &Path,
    source_kind: &str,
    filesystem: &source::SourceFileSystem,
) -> Result<PathBuf, Vec<Diagnostic>> {
    let path = expected_root.join(relative_path);

    let candidate = source_import_candidates(relative_path)
        .into_iter()
        .map(|candidate| expected_root.join(candidate))
        .find(|candidate| filesystem.is_file(candidate))
        .unwrap_or_else(|| {
            source_path_candidates(&path)
                .into_iter()
                .next()
                .unwrap_or(path)
        });
    let canonical = normalize_path(&candidate, filesystem)?;
    if !canonical.starts_with(&expected_root) {
        return Err(vec![Diagnostic::error(format!(
            "resolved {source_kind} import {} escapes expected source root {}",
            canonical.display(),
            expected_root.display()
        ))]);
    }
    Ok(canonical)
}

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
    crate::toolchain_library::root().map_err(|error| vec![Diagnostic::error(error)])
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

pub(super) fn source_path_candidates(base_path: &Path) -> Vec<PathBuf> {
    // An alias-only import selects the package's Product root. An empty
    // relative path must not nominate the source directory itself as a file.
    if base_path.as_os_str().is_empty() {
        return vec![PathBuf::from("main.omg"), PathBuf::from("mod.omg")];
    }
    let mut file_omg = base_path.to_path_buf();
    file_omg.set_extension("omg");

    let mut file_omega = base_path.to_path_buf();
    file_omega.set_extension("omega");

    vec![
        file_omg,
        base_path.join("mod.omg"),
        file_omega,
        base_path.join("mod.omega"),
    ]
}

/// Route a declaration import through its longest existing source prefix.
/// The parsed module and declaration must still establish its nominal identity.
pub(super) fn source_import_candidates(relative_path: &Path) -> Vec<PathBuf> {
    let mut candidates = source_path_candidates(relative_path);
    let mut prefix = relative_path.parent();
    while let Some(path) = prefix.filter(|path| !path.as_os_str().is_empty()) {
        candidates.extend(source_path_candidates(path));
        prefix = path.parent();
    }
    candidates
}
