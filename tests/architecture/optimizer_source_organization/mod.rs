//! Repository guard for the optimizer source-navigation contract.
//!
//! See the adjacent `README.md` for the checked organization contract.

use std::collections::BTreeSet;
use std::path::PathBuf;

mod catalogs;
mod entrances;
mod inventory;
mod retired_paths;

struct Audit {
    repository: PathBuf,
    source_files: BTreeSet<String>,
    violations: BTreeSet<String>,
}

#[test]
fn optimizer_source_organization_preserves_semantic_owners() {
    let mut audit = inventory::collect();
    entrances::check(&mut audit);
    catalogs::check(&mut audit);
    retired_paths::check(&mut audit);

    assert!(
        audit.violations.is_empty(),
        "optimizer source organization violations:\n{}",
        audit.violations.into_iter().collect::<Vec<_>>().join("\n")
    );
}
