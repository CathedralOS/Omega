//! Exact-rule inventory and owner-reviewed promotion custody.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

mod inventory;
mod promotion;

const RULE_INVENTORY: &str = "omega-rust/omega/representations/optimization-core/rules.md";
const PROMOTION_ROOT: &str = "omega-rust/omega/representations/optimization-core/promotions";

struct Audit {
    repository: PathBuf,
    violations: BTreeSet<String>,
}

impl Audit {
    fn new() -> Self {
        Self {
            repository: workspace_root(),
            violations: BTreeSet::new(),
        }
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("architecture crate lives under tests/architecture")
        .to_path_buf()
}

#[test]
fn exact_rule_rollout_is_complete_and_promotion_gated() {
    let mut audit = Audit::new();
    let published = inventory::check(&mut audit);
    promotion::check(&mut audit, &published);

    assert!(
        audit.violations.is_empty(),
        "optimizer rollout violations:\n{}",
        audit.violations.into_iter().collect::<Vec<_>>().join("\n")
    );
}
