//! Repository-wide guard for exported compact fingerprint fields.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("architecture crate lives under tests/architecture")
        .to_path_buf()
}

fn collect_rust_sources(directory: &Path, sources: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
        .map(|entry| entry.expect("read repository entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_rust_sources(&path, sources);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            sources.push(path);
        }
    }
}

fn exported_compact_fingerprint_field(line: &str) -> Option<&str> {
    let line = line.trim();
    let declaration = if let Some(declaration) = line.strip_prefix("pub ") {
        declaration
    } else if let Some(rest) = line.strip_prefix("pub(") {
        rest.split_once(')')?.1.trim_start()
    } else {
        return None;
    };
    let (name, ty) = declaration.split_once(':')?;
    let name = name.trim();
    let ty = ty.trim_start();
    if name.ends_with("fingerprint")
        && ty.starts_with("u64")
        && ty[3..]
            .chars()
            .next()
            .is_none_or(|next| next == ',' || next.is_whitespace())
    {
        Some(name)
    } else {
        None
    }
}

fn explicitly_non_authoritative(name: &str) -> bool {
    [
        "report",
        "compatibility",
        "cache",
        "discriminator",
        "index",
        "informational",
        "non_authoritative",
    ]
    .iter()
    .any(|classification| name.contains(classification))
}

#[test]
fn new_exported_u64_fingerprints_require_explicit_classification() {
    let root = workspace_root();
    let source_root = root.join("source/omega-rust");
    let mut sources = Vec::new();
    collect_rust_sources(&source_root, &mut sources);

    // This is a shrinking migration ceiling, not an approval list. Each row is
    // already tracked by CLASSIFY-AND-HARDEN-AUTHORITATIVE-IDENTITIES. A rename
    // to explicit report/cache vocabulary removes it; no path may add another
    // occurrence or introduce a new unclassified exported field.
    let legacy_maximums = BTreeMap::<&str, usize>::from([
        (
            "source/omega-rust/omega/build/omega-provider-planning/src/calling_policy_plans.rs:fingerprint",
            1,
        ),
        (
            "source/omega-rust/omega/representations/omega-installation-evidence/src/native_fuel/evidence.rs:fingerprint",
            1,
        ),
        (
            "source/omega-rust/omega/representations/omega-task-plans/src/lib.rs:specialization_fingerprint",
            2,
        ),
        (
            "source/omega-rust/omega/tooling/omega-artifacts/src/lib.rs:instance_contract_fingerprint",
            1,
        ),
        (
            "source/omega-rust/omega/tooling/omega-artifacts/src/lib.rs:instance_fingerprint",
            1,
        ),
        (
            "source/omega-rust/omega/tooling/omega-artifacts/src/lib.rs:provider_plan_fingerprint",
            2,
        ),
        (
            "source/omega-rust/omega/tooling/omega-artifacts/src/lib.rs:selected_provider_closure_fingerprint",
            1,
        ),
        (
            "source/omega-rust/omega/tooling/omega-artifacts/src/lib.rs:template_fingerprint",
            1,
        ),
        (
            "source/omega-rust/psi/representations/psi-checked-trees/src/facts/contract_plans.rs:contract_fingerprint",
            1,
        ),
        (
            "source/omega-rust/psi/representations/psi-checked-trees/src/facts/contract_plans.rs:fingerprint",
            1,
        ),
        (
            "source/omega-rust/psi/representations/psi-checked-trees/src/facts/nominal_machine_uses.rs:boundary_calling_plan_fingerprint",
            1,
        ),
        (
            "source/omega-rust/psi/representations/psi-checked-trees/src/facts/nominal_machine_uses.rs:contract_fingerprint",
            1,
        ),
        (
            "source/omega-rust/psi/representations/psi-checked-trees/src/facts/nominal_machine_uses.rs:published_requirement_fingerprint",
            1,
        ),
        (
            "source/omega-rust/psi/representations/psi-checked-trees/src/facts/nominal_machine_uses.rs:selected_actual_fingerprint",
            1,
        ),
        (
            "source/omega-rust/psi/representations/psi-checked-trees/src/flow/terminal.rs:cleanup_contract_fingerprint",
            1,
        ),
        (
            "source/omega-rust/psi/representations/psi-checked-trees/src/flow/terminal.rs:contract_fingerprint",
            2,
        ),
        (
            "source/omega-rust/psi/representations/psi-checked-trees/src/flow/terminal.rs:target_contract_fingerprint",
            1,
        ),
        (
            "source/omega-rust/psi/representations/psi-typed-trees/src/typed_trees.rs:fingerprint",
            2,
        ),
        (
            "source/omega-rust/psi/representations/psi-typed-trees/src/typed_trees.rs:template_contract_fingerprint",
            1,
        ),
    ]);
    let mut observed = BTreeMap::<String, usize>::new();
    for path in sources {
        let relative = path.strip_prefix(&root).expect("source is below workspace");
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        for line in source.lines() {
            let Some(field) = exported_compact_fingerprint_field(line) else {
                continue;
            };
            if explicitly_non_authoritative(field) {
                continue;
            }
            let key = format!("{}:{field}", relative.display());
            *observed.entry(key).or_default() += 1;
        }
    }

    let unexpected = observed
        .iter()
        .filter(|(key, count)| {
            legacy_maximums
                .get(key.as_str())
                .is_none_or(|max| *count > max)
        })
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "new exported compact fingerprints must be named as report/cache/compatibility data or gain exact/strong authority replay; unexpected fields: {unexpected:#?}",
    );
    let stale_or_overstated = legacy_maximums
        .iter()
        .filter(|(key, maximum)| observed.get(**key) != Some(maximum))
        .collect::<Vec<_>>();
    assert!(
        stale_or_overstated.is_empty(),
        "the legacy compact-fingerprint ceiling must shrink in the same change that classifies a field; stale or overstated rows: {stale_or_overstated:#?}",
    );
}
