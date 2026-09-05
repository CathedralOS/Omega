//! Canonical vocabulary to versioned release-table reconciliation.

use std::collections::BTreeMap;
use std::fs;

use crate::Audit;

const SELECTION_SOURCE: &str =
    "omega-rust/omega/representations/omega-optimization-core/src/selection.rs";
const SELECTED_LOWERING_CATALOG: &str =
    "omega-rust/omega/pipeline/omega-regalloc/src/rules/selected_lowering/catalog.rs";
const ALLOCATION_RECOVERY_CATALOG: &str =
    "omega-rust/omega/pipeline/omega-regalloc/src/rules/allocation_recovery/catalog.rs";
const POST_ALLOCATION_CATALOG: &str =
    "omega-rust/omega/pipeline/omega-machine-optimizer/src/rules/catalog.rs";
const FUNCTION_RELATIVE_LAYOUT_CATALOG: &str = "omega-rust/omega/pipeline/omega-selected-form-encoding-to-resolved-layout/src/x86_branch_relaxation/catalog.rs";
const INVENTORY_START: &str = "<!-- exact-rule-inventory:start -->";
const INVENTORY_END: &str = "<!-- exact-rule-inventory:end -->";

pub(super) fn check(audit: &mut Audit) -> BTreeMap<String, ReleaseRow> {
    let canonical = canonical_rules(audit);
    let published = published_rules(audit);
    if canonical.keys().collect::<Vec<_>>() != published.keys().collect::<Vec<_>>() {
        audit.violations.insert(format!(
            "optimizer release inventory names drifted from the canonical vocabulary: expected {:?}, published {:?}",
            canonical.keys().collect::<Vec<_>>(),
            published.keys().collect::<Vec<_>>()
        ));
    }

    for (name, expected) in canonical {
        let Some(row) = published.get(&name) else {
            continue;
        };
        if row.phase != expected.phase {
            audit.violations.insert(format!(
                "optimizer release row `{name}` has phase `{}`, expected `{}`",
                row.phase, expected.phase
            ));
        }
        if row.applicability != expected.applicability {
            audit.violations.insert(format!(
                "optimizer release row `{name}` has applicability `{}`, expected `{}`",
                row.applicability, expected.applicability
            ));
        }
        if row.rollback != format!("--disable-optimization {name}") {
            audit.violations.insert(format!(
                "optimizer release row `{name}` has a non-exact rollback command `{}`",
                row.rollback
            ));
        }
        if row.owner_review != "Required" {
            audit.violations.insert(format!(
                "optimizer release row `{name}` does not require owner review"
            ));
        }
        if !matches!(
            row.status.as_str(),
            "Experimental" | "Recommended" | "Default"
        ) {
            audit.violations.insert(format!(
                "optimizer release row `{name}` has unknown status `{}`",
                row.status
            ));
        }
    }
    published
}

fn canonical_rules(audit: &mut Audit) -> BTreeMap<String, CanonicalRule> {
    let Ok(contents) = fs::read_to_string(audit.repository.join(SELECTION_SOURCE)) else {
        audit.violations.insert(format!(
            "cannot read canonical optimization vocabulary {SELECTION_SOURCE}"
        ));
        return BTreeMap::new();
    };
    let mut pending_name = None;
    let mut rules = BTreeMap::new();
    for line in contents.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("case: \"") {
            pending_name = rest.split('"').next().map(|name| {
                if is_broad_optimization_alias(name) {
                    audit.violations.insert(format!(
                        "canonical optimization vocabulary exposes forbidden broad alias `{name}`"
                    ));
                }
                name.to_owned()
            });
            continue;
        }
        let Some(name) = pending_name.take_if(|_| line.starts_with("phase: ")) else {
            continue;
        };
        let phase = line
            .trim_start_matches("phase: ")
            .trim_end_matches(',')
            .to_owned();
        let applicability = catalog_applicability(audit, &name, &phase);
        if rules
            .insert(
                name.clone(),
                CanonicalRule {
                    phase,
                    applicability,
                },
            )
            .is_some()
        {
            audit.violations.insert(format!(
                "canonical optimization vocabulary repeats release name `{name}`"
            ));
        }
    }
    rules
}

fn is_broad_optimization_alias(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "o0" | "o1"
            | "o2"
            | "o3"
            | "os"
            | "oz"
            | "debug"
            | "release"
            | "optimizationlevel"
            | "optimizationprofile"
    )
}

fn catalog_applicability(audit: &mut Audit, name: &str, phase: &str) -> String {
    if phase == "Psi" {
        return "Target-independent".to_owned();
    }
    let catalog = match phase {
        "SelectedLowering" => SELECTED_LOWERING_CATALOG,
        "AllocationRecovery" => ALLOCATION_RECOVERY_CATALOG,
        "PostAllocationMachine" => POST_ALLOCATION_CATALOG,
        "FunctionRelativeLayout" => FUNCTION_RELATIVE_LAYOUT_CATALOG,
        unknown => {
            audit.violations.insert(format!(
                "canonical optimization `{name}` has unknown release phase `{unknown}`"
            ));
            return "Unknown".to_owned();
        }
    };
    let Ok(contents) = fs::read_to_string(audit.repository.join(catalog)) else {
        audit.violations.insert(format!(
            "cannot read optimization applicability catalog {catalog}"
        ));
        return "Unknown".to_owned();
    };
    let marker = format!("Optimization::{name}");
    let Some(start) = contents.find(&marker) else {
        audit.violations.insert(format!(
            "optimization applicability catalog {catalog} lacks exact rule `{name}`"
        ));
        return "Unknown".to_owned();
    };
    let remaining = &contents[start + marker.len()..];
    let row = remaining
        .find("Optimization::")
        .map_or(remaining, |end| &remaining[..end]);
    if row.contains("Architecture::Aarch64") {
        "AArch64".to_owned()
    } else if row.contains("Architecture::X86_64") {
        "x86-64".to_owned()
    } else if row.contains("RegisterAllocationRuleTargetApplicability::TargetIndependent") {
        "Target-independent".to_owned()
    } else {
        audit.violations.insert(format!(
            "optimization applicability catalog {catalog} has no recognized target payload for `{name}`"
        ));
        "Unknown".to_owned()
    }
}

fn published_rules(audit: &mut Audit) -> BTreeMap<String, ReleaseRow> {
    let Ok(contents) = fs::read_to_string(audit.repository.join(super::RELEASE_NOTES)) else {
        audit.violations.insert(format!(
            "cannot read versioned optimizer release notes {}",
            super::RELEASE_NOTES
        ));
        return BTreeMap::new();
    };
    let Some((_, inventory)) = contents.split_once(INVENTORY_START) else {
        audit.violations.insert(format!(
            "optimizer release notes lack inventory start marker {INVENTORY_START}"
        ));
        return BTreeMap::new();
    };
    let Some((inventory, _)) = inventory.split_once(INVENTORY_END) else {
        audit.violations.insert(format!(
            "optimizer release notes lack inventory end marker {INVENTORY_END}"
        ));
        return BTreeMap::new();
    };

    let mut rows = BTreeMap::new();
    for line in inventory.lines().map(str::trim) {
        if !line.starts_with('|') || line.contains("Exact rule") || line.contains("---") {
            continue;
        }
        let fields = line
            .split('|')
            .skip(1)
            .take_while(|field| !field.is_empty())
            .map(|field| field.trim().trim_matches('`').to_owned())
            .collect::<Vec<_>>();
        let [name, phase, applicability, status, rollback, owner_review] = fields.as_slice() else {
            audit.violations.insert(format!(
                "optimizer release inventory row is not the exact six-column schema: {line}"
            ));
            continue;
        };
        if rows
            .insert(
                name.clone(),
                ReleaseRow {
                    phase: phase.clone(),
                    applicability: applicability.clone(),
                    status: status.clone(),
                    rollback: rollback.clone(),
                    owner_review: owner_review.clone(),
                },
            )
            .is_some()
        {
            audit.violations.insert(format!(
                "optimizer release inventory repeats exact rule `{name}`"
            ));
        }
    }
    rows
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CanonicalRule {
    phase: String,
    applicability: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReleaseRow {
    pub(super) status: String,
    phase: String,
    applicability: String,
    rollback: String,
    owner_review: String,
}

#[cfg(test)]
mod tests {
    use super::is_broad_optimization_alias;

    #[test]
    fn broad_levels_and_build_modes_cannot_masquerade_as_exact_rules() {
        for alias in [
            "O0",
            "O1",
            "O2",
            "O3",
            "Os",
            "Oz",
            "Debug",
            "Release",
            "OptimizationLevel",
            "OptimizationProfile",
        ] {
            assert!(is_broad_optimization_alias(alias), "missed `{alias}`");
        }
        for exact in [
            "CopyPropagation",
            "ProofCheckElision",
            "X86SelectXorZeroI64MaterializationV1",
        ] {
            assert!(!is_broad_optimization_alias(exact), "rejected `{exact}`");
        }
    }
}
