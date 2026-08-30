//! Repository guard for the optimizer source-navigation contract.
//!
//! The governing design brief is
//! `wiki/design_briefs/optimizer/source_organization.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

mod bounds;
mod catalogs;
mod entrances;
mod inventory;
mod retired_paths;

struct Audit {
    repository: PathBuf,
    source_lines: BTreeMap<String, usize>,
    violations: BTreeSet<String>,
}

#[test]
fn optimizer_source_organization_is_bounded_and_navigable() {
    let mut audit = inventory::collect();
    bounds::check(&mut audit);
    entrances::check(&mut audit);
    catalogs::check(&mut audit);
    retired_paths::check(&mut audit);

    assert!(
        audit.violations.is_empty(),
        "optimizer source organization violations:\n{}",
        audit.violations.into_iter().collect::<Vec<_>>().join("\n")
    );
}
