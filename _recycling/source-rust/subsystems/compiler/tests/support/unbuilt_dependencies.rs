//! The dependency edges a program's Build would register, stated by the
//! caller of `CompilationSession::typed_frontier_without_build`.
//!
//! A program reaches another package only through an edge its Build
//! registers (`Build::dependency(builder, dependency::from_source(...))`),
//! and that probe runs no Build. Without the edge, `use
//! omega_language_std::Console;` is read as a path inside the program's own
//! package and selects nothing. `graph` is the package graph such a Build
//! settles, for local sources only: the program's directory as the root
//! application and one Product edge per `(alias, directory)`, each package
//! captured as acquisition captures a local source.
//!
//! The identities are probe policy, numbered in the order given. They stand
//! for no review or acceptance, so nothing a receiving policy would bind
//! (entry suppliers, dangerous services, assumed contracts) is present.

// Shared by the route probe example and its test through `#[path]`.
#![allow(dead_code)]

use package_compilation::{
    BuildDeclarationKind, PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::path::Path;

fn captured(
    marker: u8,
    name: &str,
    root: &Path,
) -> Result<(PackageKeyIdentity, PackageSourceBinding), String> {
    let identity = PackageKeyIdentity::from_digest([marker; 32])
        .ok_or_else(|| format!("package `{name}` has no probe identity"))?;
    let capture = package_compilation::capture_anonymous_compilation_source(root)
        .map_err(|diagnostics| format!("cannot capture `{name}`: {diagnostics:?}"))?;
    PackageSourceBinding::new(identity, name, capture.canonical_live_root().to_path_buf())
        .with_captured_source(std::sync::Arc::clone(capture.tree()))
        .map(|binding| (identity, binding))
        .map_err(|reason| format!("cannot bind `{name}`: {reason}"))
}

/// The graph of the program in `program_root` with one Product dependency
/// per `(alias, directory)`. At most 254 dependencies: each package takes one
/// nonzero identity byte.
pub fn graph(
    program_root: &Path,
    dependencies: &[(String, std::path::PathBuf)],
) -> Result<PackageCompilationInputs, String> {
    let (root_identity, root) = captured(1, "probed_program", program_root)?;
    let mut packages = vec![root];
    let mut edges = Vec::with_capacity(dependencies.len());
    for (position, (alias, directory)) in dependencies.iter().enumerate() {
        let marker = u8::try_from(position + 2)
            .map_err(|_| "a probe graph holds at most 254 dependencies".to_owned())?;
        let (identity, dependency) = captured(marker, alias, directory)?;
        edges.push(PackageDependencyBinding::new(
            root_identity,
            alias,
            identity,
        ));
        packages.push(dependency);
    }
    PackageCompilationInputs::new(
        root_identity,
        BuildDeclarationKind::Application,
        packages,
        edges,
    )
    .map_err(|errors| format!("the stated dependencies form no package graph: {errors:?}"))
}
