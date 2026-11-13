//! A program's frozen typed trees as the lowered-form checker is shown
//! them, without running the program's Build
//! (`CompilationSession::typed_frontier_without_build`, the path behind
//! `examples/route_probe.rs`).
//!
//! The tests of the lowered-form checker read a kept fixture or a staged
//! source through here. Nothing on this path depends on a Build activation,
//! so a test that reads only what the checker concludes does not wait for
//! the product path. What the program's Build would decide is stated by the
//! caller instead: the entry it binds and the dependency edges it registers
//! (`unbuilt_dependencies.rs`).

// Shared across test targets through `#[path]`; each target uses the subset
// its programs need.
#![allow(dead_code)]

#[path = "unbuilt_dependencies.rs"]
mod unbuilt_dependencies;

use std::path::{Path, PathBuf};

/// The repository path `relative` names.
pub fn repository(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .join(relative)
}

/// The typed trees of `main` for `linux_x86_64`, or the messages that
/// stopped the program before Psi 04 finished.
///
/// `entry` is the machine the program's Build binds as the program entry;
/// `None` binds `main` when the source declares one. Each of `dependencies`
/// is a Product edge the Build registers, `(alias, repository directory)`.
pub fn frontier(
    main: &Path,
    entry: Option<&str>,
    dependencies: &[(&str, &str)],
) -> Result<Box<compiler::TypedFrontier>, Vec<String>> {
    let mut request = compiler::CheckedCompileRequest::new(main, Some("linux_x86_64"));
    if !dependencies.is_empty() {
        let program = main.parent().expect("a source file has a directory");
        let dependencies = dependencies
            .iter()
            .map(|(alias, directory)| ((*alias).to_owned(), repository(directory)))
            .collect::<Vec<_>>();
        request.package_inputs = Some(
            unbuilt_dependencies::graph(program, &dependencies)
                .unwrap_or_else(|reason| panic!("{}: {reason}", main.display())),
        );
    }
    compiler::CompilationSession::default()
        .typed_frontier_without_build(request, entry)
        .map_err(|failure| {
            failure
                .into_diagnostics()
                .into_iter()
                .map(|diagnostic| diagnostic.message)
                .collect()
        })
}

/// The typed trees of the kept program in the fixture directory `fixture`,
/// with `main` bound when it declares one and no dependency.
pub fn fixture_frontier(fixture: &str) -> Box<compiler::TypedFrontier> {
    bound_fixture_frontier(fixture, None, &[])
}

/// The typed trees of the kept program in `fixture` with what its `build.omg`
/// binds and registers stated: the entry machine and the dependency edges.
pub fn bound_fixture_frontier(
    fixture: &str,
    entry: Option<&str>,
    dependencies: &[(&str, &str)],
) -> Box<compiler::TypedFrontier> {
    frontier(&repository(fixture).join("main.omg"), entry, dependencies)
        .unwrap_or_else(|stopped| panic!("{fixture}: Psi 00 through 04 must complete: {stopped:?}"))
}

/// The typed trees of `source` staged as a standalone `main.omg` under a
/// directory named for `name`, removed afterwards; or the messages that
/// stopped it.
pub fn staged(name: &str, source: &str) -> Result<Box<compiler::TypedFrontier>, Vec<String>> {
    let root = std::env::temp_dir().join(format!("omega-unbuilt-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&root).expect("staging directory");
    let main = root.join("main.omg");
    std::fs::write(&main, source).expect("staged source");
    let staged = frontier(&main, None, &[]);
    let _ = std::fs::remove_dir_all(&root);
    staged
}

/// `staged`, for a source that must reach the checker.
pub fn staged_frontier(name: &str, source: &str) -> Box<compiler::TypedFrontier> {
    staged(name, source)
        .unwrap_or_else(|stopped| panic!("{name}: Psi 00 through 04 must complete: {stopped:?}"))
}

/// The machine named `name` that the program's own source declares. The
/// typed trees hold every bundled library beside the program, and a library
/// may declare a machine of the same name.
pub fn program_machine(
    typed: &typed_trees::TypedTrees,
    name: &str,
) -> Option<symbols::SymbolHandle> {
    typed
        .machines()
        .iter()
        .find(|declared| {
            declared.name.as_str() == name
                && typed
                    .symbols
                    .symbol_source_span(declared.symbol)
                    .and_then(|span| typed.symbols.source_file(span))
                    .is_some_and(|file| file.origin == source::SourceOrigin::User)
        })
        .map(|declared| declared.symbol)
}
