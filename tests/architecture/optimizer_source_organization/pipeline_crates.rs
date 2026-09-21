//! Pipeline-crate coverage sweep.
//!
//! Every crate directory under `omega-rust/omega/pipeline/` must either carry
//! at least one governed root (the whole crate or a subtree inside it) or be
//! pinned here as carrying no optimizer surface. A crate added or renamed
//! without either silently escapes the audit's jurisdiction.

use std::collections::BTreeSet;
use std::fs;

use crate::Audit;
use crate::inventory::{GOVERNED_ROOTS, repository_relative_path};

/// The optimizer pipeline directory the sweep walks.
const PIPELINE_DIR: &str = "omega-rust/omega/pipeline";

/// Pipeline crates carrying no optimizer-governed surface. Each entry is a
/// jurisdictional decision, not an exemption: a crate listed here that gains
/// an optimizer surface must move into `GOVERNED_ROOTS`, and a pinned crate
/// that disappears leaves a stale pin the sweep reports.
const CRATES_WITHOUT_OPTIMIZER_SURFACES: &[&str] =
    &["omega-rust/omega/pipeline/checked-compilation-to-terminal-artifact"];

pub(super) fn check(audit: &mut Audit) {
    let entries = match fs::read_dir(audit.repository.join(PIPELINE_DIR)) {
        Ok(entries) => entries,
        Err(error) => {
            audit
                .violations
                .insert(format!("failed to sweep {PIPELINE_DIR}: {error}"));
            return;
        }
    };

    let mut crates = BTreeSet::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                audit
                    .violations
                    .insert(format!("failed to sweep {PIPELINE_DIR}: {error}"));
                continue;
            }
        };
        let path = entry.path();
        if !path.is_dir() || !path.join("Cargo.toml").is_file() {
            continue;
        }
        match repository_relative_path(&audit.repository, &path) {
            Ok(relative) => {
                crates.insert(relative);
            }
            Err(error) => {
                audit.violations.insert(error);
            }
        }
    }

    let pinned: BTreeSet<&str> = CRATES_WITHOUT_OPTIMIZER_SURFACES.iter().copied().collect();
    for pinned_crate in &pinned {
        if !crates.contains(*pinned_crate) {
            audit
                .violations
                .insert(format!("pinned pipeline crate is missing: {pinned_crate}"));
        }
    }

    for crate_root in &crates {
        let governed = GOVERNED_ROOTS.iter().any(|root| {
            root == crate_root
                || root.starts_with(&format!("{crate_root}/"))
                || crate_root.starts_with(&format!("{root}/"))
        });
        let pinned_ungoverned = pinned.contains(crate_root.as_str());
        if governed && pinned_ungoverned {
            audit.violations.insert(format!(
                "pipeline crate carries governed roots while pinned ungoverned: {crate_root}"
            ));
        } else if !governed && !pinned_ungoverned {
            audit.violations.insert(format!(
                "pipeline crate outside audit jurisdiction: {crate_root} \
                 (govern a root in it or pin it in CRATES_WITHOUT_OPTIMIZER_SURFACES)"
            ));
        }
    }
}
