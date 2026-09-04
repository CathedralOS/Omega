//! Governed roots and deterministic repository inventory.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::Audit;

/// The optimizer surfaces whose source organization is architecture-governed.
/// Keep these roots explicit: silently losing a moved or renamed tree must
/// fail this test rather than shrinking its jurisdiction.
const GOVERNED_ROOTS: &[&str] = &[
    "omega-rust/psi/pipeline/psi-checked-trees-to-terminal/src/preterminal_optimization",
    "omega-rust/psi/representations/psi-optimization/src",
    "omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_entry",
    "omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_wrapper",
    "omega-rust/omega/build/omega-build-evaluation/src/optimization",
    "omega-rust/omega/compiler/omega-compiler/src/compiler/optimization",
    "omega-rust/omega/compiler/omega-compiler/src/pipeline/optimization",
    "omega-rust/omega/pipeline/omega-optimization-pipeline",
    "omega-rust/omega/tooling/omega-optimization-policy-offline",
    "omega-rust/omega/representations/omega-legalized-operations",
    "omega-rust/omega/representations/omega-assigned-target-operations",
    "omega-rust/omega/representations/omega-optimization-core",
    "omega-rust/omega/representations/omega-optimization-unit",
    "omega-rust/omega/representations/omega-selected-instructions",
    "omega-rust/omega/pipeline/omega-abstract-operations-optimizer",
    "omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations",
    "omega-rust/omega/pipeline/omega-callee-saved-requirements-to-save-storage",
    "omega-rust/omega/pipeline/omega-allocation-legality-to-active-resident-rematerialization",
    "omega-rust/omega/pipeline/omega-allocation-legality-to-literal-folds",
    "omega-rust/omega/pipeline/omega-allocation-legality-to-fixed-view-copies",
    "omega-rust/omega/pipeline/omega-allocation-legality-to-register-homes",
    "omega-rust/omega/pipeline/omega-fixed-view-copies-to-reanalyzed-legality",
    "omega-rust/omega/pipeline/omega-live-ranges-to-allocation-legality",
    "omega-rust/omega/pipeline/omega-literal-folds-to-register-homes",
    "omega-rust/omega/pipeline/omega-liveness-to-live-ranges",
    "omega-rust/omega/pipeline/omega-machine-optimizer",
    "omega-rust/omega/pipeline/omega-optimization-policy",
    "omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations",
    "omega-rust/omega/pipeline/omega-optimization-validation",
    "omega-rust/omega/pipeline/omega-psi-to-abstract-operations",
    "omega-rust/omega/pipeline/omega-post-allocation-machine-to-optimized-machine",
    "omega-rust/omega/pipeline/omega-regalloc",
    "omega-rust/omega/pipeline/omega-register-homes-to-callee-saved-requirements",
    "omega-rust/omega/pipeline/omega-register-homes-to-post-allocation-machine",
    "omega-rust/omega/pipeline/omega-selected-instructions-to-liveness",
    "omega-rust/omega/pipeline/omega-selected-instructions-to-machine-effects",
    "omega-rust/omega/pipeline/omega-spill-access-constraints-to-frame-requirements",
    "omega-rust/omega/pipeline/omega-target-operations-to-assigned-target-operations",
    "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions",
    "omega-rust/omega/pipeline/omega-target-to-register-environment",
    "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact",
];

/// One rule-owning stage's complete navigation contract. Keeping these fields
/// together prevents the entrance, catalog, marker, and next rung from
/// drifting across parallel architecture-test tables.
pub(super) struct RuleStageDescriptor {
    pub(super) entrance: &'static str,
    pub(super) catalog: &'static str,
    pub(super) coordination_marker: &'static str,
    pub(super) catalog_marker: &'static str,
    pub(super) next_rungs: &'static [&'static str],
}

pub(super) const RULE_STAGES: &[RuleStageDescriptor] = &[
    RuleStageDescriptor {
        entrance: "omega-rust/psi/pipeline/psi-checked-trees-to-terminal/src/preterminal_optimization/mod.rs",
        catalog: "omega-rust/psi/representations/psi-optimization/src/catalog.rs",
        coordination_marker: "pub fn run_psi_optimization",
        catalog_marker: "PRETERMINAL_PSI_PASS_CATALOG",
        next_rungs: &[],
    },
    RuleStageDescriptor {
        entrance: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/mod.rs",
        catalog: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/catalog.rs",
        coordination_marker: "pub fn legalize_target_operations",
        catalog_marker: "LEGALIZATION_FORMS",
        next_rungs: &[
            "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/source",
            "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/replay",
        ],
    },
    RuleStageDescriptor {
        entrance: "omega-rust/omega/pipeline/omega-abstract-operations-optimizer/src/rules/mod.rs",
        catalog: "omega-rust/omega/pipeline/omega-abstract-operations-optimizer/src/rules/catalog.rs",
        coordination_marker: "pub fn built_in_psi_registries",
        catalog_marker: "PSI_PASS_CATALOG",
        next_rungs: &[
            "omega-rust/omega/pipeline/omega-abstract-operations-optimizer/src/rules/passes",
        ],
    },
    RuleStageDescriptor {
        entrance: "omega-rust/omega/pipeline/omega-regalloc/src/rules/selected_lowering/mod.rs",
        catalog: "omega-rust/omega/pipeline/omega-regalloc/src/rules/selected_lowering/catalog.rs",
        coordination_marker: "pub fn resolve_selected_lowering_rules",
        catalog_marker: "SELECTED_LOWERING_RULE_CATALOG",
        next_rungs: &[
            "omega-rust/omega/pipeline/omega-regalloc/src/rules/selected_lowering/literal_fold",
        ],
    },
    RuleStageDescriptor {
        entrance: "omega-rust/omega/pipeline/omega-regalloc/src/rules/allocation_recovery/mod.rs",
        catalog: "omega-rust/omega/pipeline/omega-regalloc/src/rules/allocation_recovery/catalog.rs",
        coordination_marker: "pub fn selected_allocation_recovery_rule",
        catalog_marker: "ALLOCATION_RECOVERY_RULE_CATALOG",
        next_rungs: &[
            "omega-rust/omega/pipeline/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy",
            "omega-rust/omega/pipeline/omega-regalloc/src/rules/allocation_recovery/pressure_rematerialization",
        ],
    },
    RuleStageDescriptor {
        entrance: "omega-rust/omega/pipeline/omega-machine-optimizer/src/rules/mod.rs",
        catalog: "omega-rust/omega/pipeline/omega-machine-optimizer/src/rules/catalog.rs",
        coordination_marker: "pub fn selected_post_allocation_machine_rule",
        catalog_marker: "POST_ALLOCATION_MACHINE_RULE_CATALOG",
        next_rungs: &[
            "omega-rust/omega/pipeline/omega-machine-optimizer/src/rules/peephole_matching",
            "omega-rust/omega/pipeline/omega-machine-optimizer/src/rules/aarch64",
            "omega-rust/omega/pipeline/omega-machine-optimizer/src/rules/x86_64",
        ],
    },
    RuleStageDescriptor {
        entrance: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/layout/x86_branch_relaxation/mod.rs",
        catalog: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/layout/x86_branch_relaxation/catalog.rs",
        coordination_marker: "pub fn stage_optimized_x86_branch_relaxation",
        catalog_marker: "FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG",
        next_rungs: &[
            "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/layout/x86_branch_relaxation/compute.rs",
            "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/layout/x86_branch_relaxation/validation.rs",
        ],
    },
];

pub(super) fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|candidate| {
            candidate.join("Cargo.toml").is_file() && candidate.join("omega-rust").is_dir()
        })
        .expect("architecture tests must run from within the Omega repository")
        .to_path_buf()
}

pub(super) fn collect_rust_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("cannot enumerate {}: {error}", directory.display()))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|error| format!("cannot inspect {}: {error}", entry.path().display()))?;
        if file_type.is_dir() {
            collect_rust_files(&entry.path(), files)?;
        } else if file_type.is_file() && entry.path().extension().is_some_and(|ext| ext == "rs") {
            files.push(entry.path());
        }
    }
    Ok(())
}

pub(super) fn repository_relative_path(repository: &Path, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(repository).map_err(|error| {
        format!(
            "{} is outside repository {}: {error}",
            path.display(),
            repository.display()
        )
    })?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

pub(crate) fn collect() -> Audit {
    let repository = repository_root();
    let mut violations = BTreeSet::new();
    let mut source_lines = BTreeMap::<String, usize>::new();

    for governed_root in GOVERNED_ROOTS {
        let absolute_root = repository.join(governed_root);
        if !absolute_root.is_dir() {
            violations.insert(format!("missing governed root: {governed_root}"));
            continue;
        }

        let mut files = Vec::new();
        if let Err(error) = collect_rust_files(&absolute_root, &mut files) {
            violations.insert(format!("failed to inventory {governed_root}: {error}"));
            continue;
        }
        files.sort();
        if files.is_empty() {
            violations.insert(format!(
                "governed root contains no Rust files: {governed_root}"
            ));
        }

        for file in files {
            let relative = match repository_relative_path(&repository, &file) {
                Ok(relative) => relative,
                Err(error) => {
                    violations.insert(error);
                    continue;
                }
            };
            let contents = match fs::read_to_string(&file) {
                Ok(contents) => contents,
                Err(error) => {
                    violations.insert(format!("cannot read {relative}: {error}"));
                    continue;
                }
            };
            let lines = contents.lines().count();
            if source_lines.insert(relative.clone(), lines).is_some() {
                violations.insert(format!("governed roots overlap at Rust file: {relative}"));
            }
        }
    }

    Audit {
        repository,
        source_lines,
        violations,
    }
}
