//! Sole rule catalogs, exact rule folders, and legalization inventory.

use std::fs;

use crate::Audit;

use super::inventory::{RULE_STAGES, collect_rust_files, repository_relative_path};

struct RequiredRuleCatalog {
    path: &'static str,
    order_marker: &'static str,
}

struct RequiredExactRuleFolder {
    directory: &'static str,
    rule_marker: &'static str,
}

/// Catalog rows that must descend into a same-named folder with one contract
/// and proposal join (`mod.rs`) above a closed semantic partition (`laws.rs`).
/// This prevents a short stage entrance from hiding many rules in a mixed
/// `rule.rs` catch-all one rung below the catalog.
const REQUIRED_EXACT_RULE_FOLDERS: &[RequiredExactRuleFolder] = &[
    RequiredExactRuleFolder {
        directory: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/wrapping_neutral",
        rule_marker: "pub struct WrappingNeutralArithmeticIdentityRule",
    },
    RequiredExactRuleFolder {
        directory: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/wrapping_shift_zero_count",
        rule_marker: "pub struct WrappingShiftZeroCountIdentityRule",
    },
    RequiredExactRuleFolder {
        directory: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/wrapping_multiply_zero",
        rule_marker: "pub struct WrappingMultiplyZeroAnnihilationRule",
    },
    RequiredExactRuleFolder {
        directory: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/saturating_neutral",
        rule_marker: "pub struct SaturatingNeutralArithmeticIdentityRule",
    },
    RequiredExactRuleFolder {
        directory: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/saturating_multiply_zero",
        rule_marker: "pub struct SaturatingMultiplyZeroAnnihilationRule",
    },
    RequiredExactRuleFolder {
        directory: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/bitwise_neutral",
        rule_marker: "pub struct BitwiseNeutralLiteralIdentityRule",
    },
    RequiredExactRuleFolder {
        directory: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/bitwise_absorbing",
        rule_marker: "pub struct BitwiseAbsorbingLiteralIdentityRule",
    },
];

/// Additional construction catalogs that are not source-visible optimization
/// stages but still own one closed ordered family inventory.
const REQUIRED_RULE_CATALOGS: &[RequiredRuleCatalog] = &[RequiredRuleCatalog {
    path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/construction/scalar/catalog.rs",
    order_marker: "SCALAR_FAMILIES",
}];

pub(crate) fn check(audit: &mut Audit) {
    let repository = &audit.repository;
    let violations = &mut audit.violations;

    for stage in RULE_STAGES {
        match fs::read_to_string(repository.join(stage.catalog)) {
            Ok(contents) if contents.contains(stage.catalog_marker) => {}
            Ok(_) => {
                violations.insert(format!(
                    "rule-stage catalog lacks ordered marker `{}`: {}",
                    stage.catalog_marker, stage.catalog
                ));
            }
            Err(error) => {
                violations.insert(format!(
                    "missing rule-stage catalog {}: {error}",
                    stage.catalog
                ));
            }
        }
    }

    for catalog in REQUIRED_RULE_CATALOGS {
        match fs::read_to_string(repository.join(catalog.path)) {
            Ok(contents) if contents.contains(catalog.order_marker) => {}
            Ok(_) => {
                violations.insert(format!(
                    "rule catalog lacks ordered marker `{}`: {}",
                    catalog.order_marker, catalog.path
                ));
            }
            Err(error) => {
                violations.insert(format!(
                    "missing required rule catalog {}: {error}",
                    catalog.path
                ));
            }
        }
    }

    for rule in REQUIRED_EXACT_RULE_FOLDERS {
        let entrance = repository.join(rule.directory).join("mod.rs");
        let laws = repository.join(rule.directory).join("laws.rs");
        match fs::read_to_string(&entrance) {
            Ok(contents)
                if contents.contains(rule.rule_marker)
                    && contents.contains("propose_total_scalar_identities") => {}
            Ok(_) => {
                violations.insert(format!(
                    "exact rule entrance lacks `{}` or its proposal join: {}/mod.rs",
                    rule.rule_marker, rule.directory
                ));
            }
            Err(error) => {
                violations.insert(format!(
                    "missing exact rule entrance {}/mod.rs: {error}",
                    rule.directory
                ));
            }
        }
        match fs::read_to_string(&laws) {
            Ok(contents) if contents.contains("fn classify(") => {}
            Ok(_) => {
                violations.insert(format!(
                    "exact rule laws lack a closed classifier: {}/laws.rs",
                    rule.directory
                ));
            }
            Err(error) => {
                violations.insert(format!(
                    "missing exact rule laws {}/laws.rs: {error}",
                    rule.directory
                ));
            }
        }
    }

    let legalization_root = repository.join(
        "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization",
    );
    let mut legalization_files = Vec::new();
    match collect_rust_files(&legalization_root, &mut legalization_files) {
        Ok(()) => {
            let mut catalog_declarations = Vec::new();
            for file in legalization_files {
                let Ok(relative) = repository_relative_path(&repository, &file) else {
                    continue;
                };
                let contents = match fs::read_to_string(&file) {
                    Ok(contents) => contents,
                    Err(error) => {
                        violations.insert(format!("cannot read {relative}: {error}"));
                        continue;
                    }
                };
                for _ in contents.match_indices("const LEGALIZATION_FORMS") {
                    catalog_declarations.push(relative.clone());
                }
                for superseded in [
                    "const SCALAR_LEGALIZATION_FORMS",
                    "const UNIT_LEGALIZATION_FORMS",
                    "const STRUCTURAL_UNIT_LEGALIZATION_FORMS",
                ] {
                    if contents.contains(superseded) {
                        violations.insert(format!(
                            "legalization retains superseded alternate catalog `{superseded}` in {relative}"
                        ));
                    }
                }
            }
            let expected = ["source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/catalog.rs".to_string()];
            if catalog_declarations != expected {
                violations.insert(format!(
                    "legalization must declare exactly one `LEGALIZATION_FORMS` catalog in catalog.rs; found {catalog_declarations:?}"
                ));
            }
        }
        Err(error) => {
            violations.insert(format!(
                "failed to inventory legalization catalogs: {error}"
            ));
        }
    }
}
