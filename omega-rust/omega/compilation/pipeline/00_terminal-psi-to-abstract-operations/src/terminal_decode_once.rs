//! Source-scanning guard pinning the realization path's canonical module
//! decode to stage-00 admission.
//!
//! `terminal_codec::decode_module` is the most expensive admission step the
//! compiler runs: scalar qualification, canonical ordering, structural
//! foundation and module-representation validation, then a canonical
//! re-encode that rejects non-canonical bytes. `CanonicalTerminalArtifact`
//! retains the module its own construction decode produced, so every
//! consumer holding the artifact reads `semantic_module()` and pays nothing.
//! The only caller that may decode is the byte entrance — a function whose
//! input is loose section bytes rather than an artifact — and in Omega that is
//! stage-00's `lower_artifact`. Provider installation reads the admitted
//! artifact's retained module through `admit_checked_provider_installation`.
//!
//! A decode added anywhere else re-runs the whole admission against a module
//! the artifact already decoded; a decode removed from an entrance leaves an
//! admit path with no module at all. Either change must update
//! [`PERMITTED_MODULE_DECODES`] — the ratchet fails in both directions, so a
//! deleted site cannot rot the allowlist. `CanonicalTerminalArtifact::
//! from_bytes` is the same admission in envelope form: reconstructing the
//! artifact re-decodes its semantic section internally, so its sites are
//! pinned by [`PERMITTED_ARTIFACT_REPLAYS`], and the byte round trip this
//! module removed — `from_bytes` fed by `to_bytes()` on one expression — is
//! pinned at zero occurrences.
//!
//! The scan covers `omega-rust/omega/` Rust sources minus `tests/`,
//! `tests.rs` and `*_tests.rs` files, inline `#[cfg(test)] mod … { … }`
//! blocks, `examples/`, `target/`, and dot-directories; Psi-side
//! decodes at external byte ingress live outside that root. Internal provider
//! installation passes a retained view, not bytes. Needles use `concat!` so this file's own text
//! never satisfies a pattern it enforces.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn provider_installation_uses_retained_artifact_custody() {
    let source = include_str!("provider_installation/admission.rs");
    assert!(source.contains("TerminalArtifactView<'_>"));
    assert!(!source.contains("admit_provider_installation_from_artifact("));
    assert!(!source.contains("semantic_bytes"));
}

/// The `decode_module` call needle spelled without the contiguous literal;
/// see the module header.
const DECODE_MODULE_CALL: &str = concat!("decode_module", "(");
/// The `CanonicalTerminalArtifact::from_bytes` call needle, same trick.
const ARTIFACT_REPLAY_CALL: &str = concat!("CanonicalTerminalArtifact", "::", "from_bytes", "(");
/// `from_bytes` and `to_bytes` spelled as concat fragments; a scanned line
/// carrying both call shapes re-admits bytes a value just encoded.
const FROM_BYTES_CALL: &str = concat!("from_bytes", "(");
const TO_BYTES_OUTPUT: &str = concat!("to_bytes", "()");

/// The complete permitted set of `decode_module` call sites in Omega non-test
/// source: each entry's repository-relative path, its exact occurrence count,
/// and why the site is a byte entrance rather than an artifact-holding
/// consumer.
const PERMITTED_MODULE_DECODES: &[(&str, usize, &str)] = &[(
    "omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/artifact_admission.rs",
    1,
    "stage-00 admission: `lower_artifact` decodes loose canonical sections",
)];

/// The complete permitted set of `CanonicalTerminalArtifact::from_bytes`
/// envelope replays in Omega non-test source: admitting a description's
/// embedded artifact on the describe path, and inspecting envelope bytes a
/// user supplied. The realization path holds the artifact itself and must
/// never rebuild one.
const PERMITTED_ARTIFACT_REPLAYS: &[(&str, usize, &str)] = &[
    (
        "omega-rust/omega/application/cli/src/inspection/mod.rs",
        1,
        "inspect path: `inspect_terminal_artifact` admits externally supplied envelope bytes",
    ),
    (
        "omega-rust/omega/compilation/backend/artifacts/component-description/src/component_description.rs",
        2,
        "describe path: `decode_component_description` and `description_subject` admit the description's embedded artifact",
    ),
];

/// Every `.rs` file under `root` that is not test, example, or build-output
/// source. The guard counts realizations-path text, so `tests/` module trees,
/// `tests.rs` and `*_tests.rs` files, `examples/`, `target/`, and
/// dot-directories stay out.
fn collect_rust_sources(root: &Path, sources: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(root)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", root.display()));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("cannot list {}: {error}", root.display()))
            .path();
        if path.is_dir() {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if name.starts_with('.') || matches!(name, "tests" | "target" | "examples") {
                continue;
            }
            collect_rust_sources(&path, sources);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs")
            && !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "tests.rs" || name.ends_with("_tests.rs"))
        {
            sources.push(path);
        }
    }
}

/// Repository root and the `omega-rust/omega/` tree the guard scans, derived
/// from this crate's manifest directory so the check covers the checkout it
/// compiles in — a worktree scans its own files.
fn scanned_tree() -> (PathBuf, PathBuf) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository = manifest
        .ancestors()
        .nth(5)
        .unwrap_or_else(|| panic!("{manifest:?} has no repository-root ancestor"))
        .to_path_buf();
    let omega = manifest
        .ancestors()
        .nth(3)
        .unwrap_or_else(|| panic!("{manifest:?} has no omega/ ancestor"))
        .to_path_buf();
    (repository, omega)
}

/// Numbered code lines of one source file: `//` lines are prose, and an
/// inline `#[cfg(test)] mod name { … }` block is test source, skipped through
/// the closing brace at its own indentation. A `#[cfg(test)] mod name;`
/// declaration names a separate file, which `collect_rust_sources` filters.
fn code_lines(text: &str) -> Vec<(usize, &str)> {
    let mut lines = text.lines().enumerate().peekable();
    let mut code = Vec::new();
    while let Some((number, line)) = lines.next() {
        if line.trim() == "#[cfg(test)]"
            && let Some((_, item)) = lines.peek()
            && item.trim_start().starts_with("mod ")
            && item.trim_end().ends_with('{')
        {
            let indentation = &item[..item.len() - item.trim_start().len()];
            let closing = format!("{indentation}}}");
            for (_, skipped) in lines.by_ref() {
                if skipped.trim_end() == closing {
                    break;
                }
            }
            continue;
        }
        if !line.trim_start().starts_with("//") {
            code.push((number, line));
        }
    }
    code
}

/// Count `needle` occurrences on code lines in every scanned file, keyed by
/// repository-relative path. A comment naming the pinned shape (like the
/// removed round trip's own history note) is not a violation, and neither is
/// an inline test module. Files with no occurrence stay out of the map.
fn count_occurrences(
    repository: &Path,
    sources: &[PathBuf],
    needle: &str,
) -> BTreeMap<String, usize> {
    let mut found = BTreeMap::new();
    for source in sources {
        let text = fs::read_to_string(source)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", source.display()));
        let count = code_lines(&text)
            .into_iter()
            .map(|(_, line)| line.matches(needle).count())
            .sum::<usize>();
        if count > 0 {
            let relative = source
                .strip_prefix(repository)
                .unwrap_or_else(|_| panic!("{} escapes {}", source.display(), repository.display()))
                .to_string_lossy()
                .replace('\\', "/");
            found.insert(relative, count);
        }
    }
    found
}

fn expected(sites: &[(&str, usize, &str)]) -> BTreeMap<String, usize> {
    sites
        .iter()
        .map(|(path, count, _)| ((*path).to_owned(), *count))
        .collect()
}

fn describe(sites: &[(&str, usize, &str)]) -> String {
    sites
        .iter()
        .map(|(path, count, role)| format!("  {count}x {path} — {role}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `decode_module` appears in Omega non-test source only at the admitted
/// byte entrances in [`PERMITTED_MODULE_DECODES`], each with its exact
/// occurrence count. Any consumer holding a `CanonicalTerminalArtifact` must
/// read `semantic_module()` instead of decoding; a new decode site, an extra
/// decode inside a permitted file, or a removed entrance all fail here.
#[test]
fn realization_decodes_terminal_module_only_at_stage_00_admission() {
    let (repository, omega) = scanned_tree();
    let mut sources = Vec::new();
    collect_rust_sources(&omega, &mut sources);
    let found = count_occurrences(&repository, &sources, DECODE_MODULE_CALL);
    let permitted = expected(PERMITTED_MODULE_DECODES);
    assert_eq!(
        found,
        permitted,
        "canonical module decode moved off stage-00 admission; every consumer holding a \
         CanonicalTerminalArtifact reads semantic_module() instead of re-decoding, and only the \
         loose-bytes entrances may call decode_module. Permitted sites:\n{}",
        describe(PERMITTED_MODULE_DECODES),
    );
}

/// `CanonicalTerminalArtifact::from_bytes` is admission in envelope form: it
/// re-decodes the semantic section internally. Its only Omega sites admit
/// bytes from outside the compile: the describe path's embedded artifact and
/// the CLI's inspection of supplied envelope bytes. The realization path holds
/// the artifact itself, and a new replay there repeats the whole decode the
/// artifact already paid.
#[test]
fn realization_never_rebuilds_the_terminal_artifact() {
    let (repository, omega) = scanned_tree();
    let mut sources = Vec::new();
    collect_rust_sources(&omega, &mut sources);
    let found = count_occurrences(&repository, &sources, ARTIFACT_REPLAY_CALL);
    let permitted = expected(PERMITTED_ARTIFACT_REPLAYS);
    assert_eq!(
        found,
        permitted,
        "a Terminal artifact replay entered Omega non-test source outside the describe path; the \
         realization path holds the artifact and must not rebuild it from bytes. Permitted \
         sites:\n{}",
        describe(PERMITTED_ARTIFACT_REPLAYS),
    );
}

/// No scanned line feeds `to_bytes()` output into a `from_bytes` call: that
/// is the byte round trip `fragment_object_publication` used to run — four
/// module validations, a canonical re-encode, and proof, debug and manifest
/// replay, purely to obtain ownership of an identical artifact — now a
/// clone, since the artifact's invariant is a property of its private bytes.
#[test]
fn no_source_replays_encoded_bytes_through_from_bytes() {
    let (repository, omega) = scanned_tree();
    let mut sources = Vec::new();
    collect_rust_sources(&omega, &mut sources);
    let mut violations = Vec::new();
    for source in &sources {
        let text = fs::read_to_string(source)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", source.display()));
        for (line_number, line) in code_lines(&text) {
            if line.contains(FROM_BYTES_CALL) && line.contains(TO_BYTES_OUTPUT) {
                let relative = source
                    .strip_prefix(&repository)
                    .unwrap_or_else(|_| {
                        panic!("{} escapes {}", source.display(), repository.display())
                    })
                    .to_string_lossy()
                    .replace('\\', "/");
                violations.push(format!("{relative}:{}", line_number + 1));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "from_bytes fed by to_bytes() re-admits bytes a value just encoded — the byte round trip \
         this stage removed; clone or share the value instead. Sites: {violations:?}"
    );
}
