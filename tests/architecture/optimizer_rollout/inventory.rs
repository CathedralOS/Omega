//! Canonical vocabulary to versioned release-table reconciliation, and the
//! reverse ownership direction: every rule a phase's rewrite catalog can
//! enable must itself be canonical vocabulary declared with that phase.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;

use crate::Audit;

const SELECTION_SOURCE: &str =
    "omega-rust/omega/representations/optimization-core/src/optimization_core.rs";
const SELECTED_LOWERING_CATALOG: &str = "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/catalog.rs";
const ALLOCATION_RECOVERY_CATALOG: &str = "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/catalog.rs";
const FUNCTION_RELATIVE_LAYOUT_CATALOG: &str = "omega-rust/omega/pipeline/resolved-layout-to-resolved-layout/src/x86_branch_relaxation/catalog.rs";
const INVENTORY_START: &str = "<!-- exact-rule-inventory:start -->";
const INVENTORY_END: &str = "<!-- exact-rule-inventory:end -->";

/// The cataloged phases and their single enable/order catalogs. A rule in a
/// cataloged phase resolves its applicability from the catalog row; Psi and
/// checked-tree rules own no per-rule stage catalog.
const PHASE_CATALOGS: &[(&str, &str)] = &[
    ("SelectedLowering", SELECTED_LOWERING_CATALOG),
    ("AllocationRecovery", ALLOCATION_RECOVERY_CATALOG),
    ("FunctionRelativeLayout", FUNCTION_RELATIVE_LAYOUT_CATALOG),
];

pub(super) fn check(
    audit: &mut Audit,
) -> (
    BTreeMap<String, ReleaseRow>,
    BTreeMap<String, CanonicalRule>,
) {
    let canonical = canonical_rules(audit);
    let published = published_rules(audit);
    if canonical.keys().collect::<Vec<_>>() != published.keys().collect::<Vec<_>>() {
        audit.violations.insert(format!(
            "optimizer release inventory names drifted from the canonical vocabulary: expected {:?}, published {:?}",
            canonical.keys().collect::<Vec<_>>(),
            published.keys().collect::<Vec<_>>()
        ));
    }

    for (name, expected) in &canonical {
        let Some(row) = published.get(name) else {
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
    for &(phase, catalog) in PHASE_CATALOGS {
        check_catalog_rule_ownership(audit, &canonical, phase, catalog);
    }
    (published, canonical)
}

/// Every `Optimization` member a phase catalog enables must be owned by the
/// canonical vocabulary under that same phase. `catalog_applicability`
/// covers the other direction — a canonical rule missing from its catalog —
/// but a catalog row naming a retired or foreign-phase member is an unowned
/// rewrite no release-inventory row can reach.
fn check_catalog_rule_ownership(
    audit: &mut Audit,
    canonical: &BTreeMap<String, CanonicalRule>,
    phase: &str,
    catalog: &str,
) {
    let Some(block) = catalog_rule_block(audit, catalog) else {
        return;
    };
    let mut seen = BTreeSet::new();
    for member in catalog_members(&block) {
        if !seen.insert(member.clone()) {
            audit.violations.insert(format!(
                "rewrite rule catalog {catalog} repeats exact rule `{member}`"
            ));
        }
        match canonical.get(&member) {
            None => {
                audit.violations.insert(format!(
                    "rewrite rule catalog {catalog} enables `{member}`, which is not canonical optimization vocabulary"
                ));
            }
            Some(rule) if rule.phase != phase => {
                audit.violations.insert(format!(
                    "rewrite rule catalog {catalog} enables `{member}`, whose canonical phase is `{}`",
                    rule.phase
                ));
            }
            _ => {}
        }
    }
}

/// The text of a catalog file's single `const *_CATALOG` table: the block
/// whose `OptimizationCatalogDescriptor` entry heads enumerate the rewrite
/// rules the phase can enable. Reading only this block keeps the audit
/// immune to `Optimization::` mentions in the catalog's own tests.
fn catalog_rule_block(audit: &mut Audit, catalog: &str) -> Option<String> {
    let Ok(contents) = fs::read_to_string(audit.repository.join(catalog)) else {
        audit
            .violations
            .insert(format!("cannot read rewrite rule catalog {catalog}"));
        return None;
    };
    let start = contents.match_indices("pub const ").find_map(|(at, _)| {
        let line = contents[at..].lines().next().unwrap_or("");
        line.contains("CATALOG").then_some(at)
    });
    let Some(start) = start else {
        audit.violations.insert(format!(
            "rewrite rule catalog {catalog} declares no `pub const *_CATALOG` table"
        ));
        return None;
    };
    let Some(end) = contents[start..].find("];") else {
        audit.violations.insert(format!(
            "rewrite rule catalog {catalog} leaves its `*_CATALOG` const unterminated"
        ));
        return None;
    };
    Some(contents[start..start + end].to_owned())
}

/// The `Optimization::Name` members spelled inside a catalog const block —
/// each entry head is one rule the catalog enables.
fn catalog_members(block: &str) -> Vec<String> {
    block
        .split("Optimization::")
        .skip(1)
        .map(|rest| {
            rest.chars()
                .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                .collect()
        })
        .collect()
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
        // Retired physical-rewrite spellings remain decode/rejection vocabulary,
        // not supported executable rules eligible for rollout.
        if phase == "PostAllocationMachine" {
            continue;
        }
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
    // Psi and checked-tree rules own no per-rule stage catalog; both phases are
    // target-independent transforms over fully checked source semantics.
    if phase == "Psi" || phase == "CheckedTrees" {
        return "Target-independent".to_owned();
    }
    let Some(&(_, catalog)) = PHASE_CATALOGS
        .iter()
        .find(|(catalog_phase, _)| *catalog_phase == phase)
    else {
        audit.violations.insert(format!(
            "canonical optimization `{name}` has unknown release phase `{phase}`"
        ));
        return "Unknown".to_owned();
    };
    let Some(block) = catalog_rule_block(audit, catalog) else {
        return "Unknown".to_owned();
    };
    let marker = format!("Optimization::{name}");
    let Some(start) = block.find(&marker) else {
        audit.violations.insert(format!(
            "optimization applicability catalog {catalog} lacks exact rule `{name}`"
        ));
        return "Unknown".to_owned();
    };
    let remaining = &block[start + marker.len()..];
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
    let Ok(contents) = fs::read_to_string(audit.repository.join(super::RULE_INVENTORY)) else {
        audit.violations.insert(format!(
            "cannot read optimizer rule inventory {}",
            super::RULE_INVENTORY
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
pub(super) struct CanonicalRule {
    pub(super) phase: String,
    pub(super) applicability: String,
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
    use std::collections::BTreeMap;
    use std::collections::BTreeSet;
    use std::fs;

    use super::{
        CanonicalRule, catalog_members, catalog_rule_block, check_catalog_rule_ownership,
        is_broad_optimization_alias,
    };
    use crate::Audit;

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

    fn rule(name: &str, phase: &str) -> (String, CanonicalRule) {
        (
            name.to_owned(),
            CanonicalRule {
                phase: phase.to_owned(),
                applicability: "Target-independent".to_owned(),
            },
        )
    }

    fn fixture_catalog(contents: &str) -> (std::path::PathBuf, Audit) {
        let root = std::env::temp_dir().join(format!(
            "omega_catalog_ownership_gate_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(root.join("phase")).expect("fixture root");
        fs::write(root.join("phase/catalog.rs"), contents).expect("fixture catalog");
        let audit = Audit {
            repository: root.clone(),
            violations: BTreeSet::new(),
        };
        (root, audit)
    }

    const CATALOG_SHAPE: &str = r#"
        pub const SOME_PHASE_RULE_CATALOG: [Entry; 4] = [
            Entry::new(Optimization::RightPhase, ()),
            Entry::new(Optimization::RightPhase, ()),
            Entry::new(Optimization::ForeignPhase, ()),
            Entry::new(Optimization::NotCanonical, ()),
        ];
        #[cfg(test)]
        mod tests {
            fn selects() { let _ = Optimization::TestOnlyMention; }
        }
    "#;

    /// The reverse ownership check rejects catalog rows the canonical
    /// vocabulary does not own — retired members, foreign-phase members, and
    /// repeats — while a `Optimization::` mention in the catalog's own test
    /// module is never mistaken for an enabled rule.
    #[test]
    fn catalog_rows_must_be_canonical_rules_of_the_catalog_phase() {
        let (root, mut audit) = fixture_catalog(CATALOG_SHAPE);
        let canonical: BTreeMap<String, CanonicalRule> = [
            rule("RightPhase", "SelectedLowering"),
            rule("ForeignPhase", "AllocationRecovery"),
        ]
        .into_iter()
        .collect();

        check_catalog_rule_ownership(
            &mut audit,
            &canonical,
            "SelectedLowering",
            "phase/catalog.rs",
        );

        let violations: Vec<&String> = audit.violations.iter().collect();
        assert_eq!(violations.len(), 3, "{violations:?}");
        assert!(
            violations.iter().any(|violation| violation
                .contains("enables `ForeignPhase`, whose canonical phase is `AllocationRecovery`"))
        );
        assert!(
            violations.iter().any(|violation| violation.contains(
                "enables `NotCanonical`, which is not canonical optimization vocabulary"
            ))
        );
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("repeats exact rule `RightPhase`")),
            "{violations:?}"
        );
        assert_eq!(
            violations
                .iter()
                .filter(|v| v.contains("TestOnlyMention"))
                .count(),
            0
        );
        drop(fs::remove_dir_all(root));
    }

    /// A malformed or absent catalog table reports its own violation rather
    /// than silently passing the ownership loop.
    #[test]
    fn catalog_rule_block_flags_missing_and_unterminated_tables() {
        let (root, mut audit) = fixture_catalog("pub const UNRELATED: u32 = 0;\n");
        assert!(catalog_rule_block(&mut audit, "phase/catalog.rs").is_none());
        assert!(
            audit
                .violations
                .iter()
                .any(|violation| { violation.contains("declares no `pub const *_CATALOG` table") })
        );

        let (second_root, mut audit) =
            fixture_catalog("pub const SOME_PHASE_RULE_CATALOG: [Entry; 0] = [");
        assert!(catalog_rule_block(&mut audit, "phase/catalog.rs").is_none());
        assert!(
            audit.violations.iter().any(|violation| {
                violation.contains("leaves its `*_CATALOG` const unterminated")
            })
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(second_root);
    }

    #[test]
    fn catalog_members_extracts_only_entry_head_idents() {
        let members =
            catalog_members("Entry::new(Optimization::Alpha, ()); let _ = Optimization::Beta_X9;");
        assert_eq!(members, ["Alpha".to_owned(), "Beta_X9".to_owned()]);
    }
}
