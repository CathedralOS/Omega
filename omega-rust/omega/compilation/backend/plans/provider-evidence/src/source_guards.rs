//! Source guards on provider-plan identity storage. Provider plans and
//! schemas store program-owned identities once in `ProviderIdentityTable`;
//! interned rows carry `ProviderIdentityId` handles and render text through
//! `identities.text(..)` / `identity_text(..)` only at report and codec
//! boundaries. `ProviderPlanRow::requirement_identity` is exempt by design: it
//! is the row's canonical rendered text, not an interned identity. The
//! forbidden needles are assembled at runtime so this file never contains
//! them verbatim.

use std::fs;
use std::path::{Path, PathBuf};

/// Every `(file, line number, line)` under `root` (a file or directory
/// relative to `src/`) in this crate's non-test sources, outside line
/// comments.
fn source_lines(root: &str) -> Vec<(String, usize, String)> {
    let mut directories = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(root)];
    let mut lines = Vec::new();
    while let Some(directory) = directories.pop() {
        let entries: Vec<PathBuf> = match fs::read_dir(&directory) {
            Ok(entries) => entries
                .map(|entry| {
                    entry
                        .unwrap_or_else(|error| {
                            panic!("cannot list {}: {error}", directory.display())
                        })
                        .path()
                })
                .collect(),
            Err(_) if directory.is_file() => vec![directory.clone()],
            Err(error) => panic!("cannot read {}: {error}", directory.display()),
        };
        for path in entries {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if path.is_dir() {
                if !name.starts_with('.') && !matches!(name, "tests" | "examples" | "target") {
                    directories.push(path);
                }
                continue;
            }
            if path.extension().and_then(|extension| extension.to_str()) != Some("rs")
                || name == "tests.rs"
                || name.ends_with("_tests.rs")
                || name == "source_guards.rs"
            {
                continue;
            }
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            let file = path.display().to_string();
            for (line_number, line) in source.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                lines.push((file.clone(), line_number + 1, line.to_owned()));
            }
        }
    }
    lines
}

/// Every `file:line: name` under `root`, outside line comments, that names
/// one of `forbidden`.
fn violations_in(root: &str, forbidden: &[String]) -> Vec<String> {
    let mut violations = Vec::new();
    for (file, line_number, line) in source_lines(root) {
        for name in forbidden {
            if line.contains(name.as_str()) {
                violations.push(format!("{file}:{line_number}: {name}"));
            }
        }
    }
    violations.sort();
    violations
}

/// The `field: <shape>` declaration needles `field` must never carry.
fn text_shapes(field: &str) -> Vec<String> {
    [": String", ": Option<String>", ": Vec<String>"]
        .iter()
        .map(|shape| format!("{field}{shape}"))
        .collect()
}

/// The interned fields must be declared with `ProviderIdentityId` handles;
/// the guard requires each declaration so a revert to text cannot pass
/// silently.
#[test]
fn interned_fields_hold_provider_identity_ids() {
    let mut missing = Vec::new();
    for needle in [
        format!("trait_name: {}", "ProviderIdentityId"),
        format!("requirement_identity: {}", "ProviderIdentityId"),
        format!("machine_identity: {}", "ProviderIdentityId"),
    ] {
        if !source_lines("provider_evidence/providers")
            .iter()
            .any(|(_, _, line)| line.contains(needle.as_str()))
        {
            missing.push(needle);
        }
    }
    assert!(
        missing.is_empty(),
        "provider rows lost their interned identity declarations: {missing:?}"
    );
}

/// No provider type may redeclare `trait_name` or `machine_identity` as
/// text; those names are interned identities.
#[test]
fn provider_types_carry_no_text_trait_or_machine_identities() {
    let mut forbidden = text_shapes("trait_name");
    forbidden.extend(text_shapes("machine_identity"));
    let violations = violations_in("provider_evidence/providers", &forbidden);
    assert!(
        violations.is_empty(),
        "provider types must carry interned identity ids, not text: {violations:?}"
    );
}

/// `requirement_identity` is interned on `ServiceMethod` and
/// `ServiceProgressEstablishmentRoute`; `ProviderPlanRow` deliberately keeps
/// its canonical rendered text in `provider_plan.rs`, so the text ban covers
/// the interned declarations' file only.
#[test]
fn service_schemas_carry_no_text_requirement_identities() {
    let violations = violations_in(
        "provider_evidence/providers/provider_plan/service_schemas.rs",
        &text_shapes("requirement_identity"),
    );
    assert!(
        violations.is_empty(),
        "service schemas must carry interned identity ids, not text: {violations:?}"
    );
}

/// Boundary declarations where identity text is legitimate: constructor
/// parameters interning on arrival and the authored-exclusion `Service`
/// payload, which carries the selected declaration's hermetic identity as
/// text until a Terminal module's catalog resolves it.
const ALLOWED: &[(&str, &str)] = &[(
    "behavior_exclusions/authored.rs",
    "Service { identity: String, name: String },",
)];

/// Every `file:line: name` in this crate's non-test sources, outside line
/// comments, that declares a text identity. `allowed` pairs are
/// `(file suffix, exact trimmed line)` so only the pinned declaration is
/// exempt — a `pub` field of the same name still violates.
fn violations(forbidden: &[&str], allowed: &[(&str, &str)]) -> Vec<String> {
    let mut violations = Vec::new();
    for (file, line_number, line) in source_lines("") {
        for name in forbidden {
            if line.contains(name)
                && !allowed.iter().any(|(suffix, declaration)| {
                    file.ends_with(suffix) && line.trim() == *declaration
                })
            {
                violations.push(format!("{file}:{line_number}: {name}"));
            }
        }
    }
    violations.sort();
    violations
}

/// Stored `*identity`/`identities` declarations carry interned
/// `TerminalIdentityIndex` handles; text identity survives only on the
/// boundary declarations pinned in `ALLOWED`.
#[test]
fn crate_carries_no_string_identity_fields() {
    let violations = violations(
        &[
            concat!("identity", ": String"),
            concat!("identity", ": Option<String>"),
            concat!("identity", ": Vec<String>"),
            concat!("identities", ": Vec<String>"),
            concat!("identities", ": HashSet<String>"),
            concat!("identities", ": BTreeSet<String>"),
        ],
        ALLOWED,
    );
    assert!(
        violations.is_empty(),
        "durable rows must carry interned identity indices, not text: {violations:?}"
    );
}
