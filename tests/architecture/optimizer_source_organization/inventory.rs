//! Governed roots and deterministic repository inventory.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::Audit;

/// The optimizer surfaces whose source organization is architecture-governed.
/// Keep these roots explicit: silently losing a moved or renamed tree must
/// fail this test rather than shrinking its jurisdiction.
const GOVERNED_ROOTS: &[&str] = &[
    "source/omega-rust/omega/backend/images/omega-image-emission/src/ranked_u32_countdown",
    "source/omega-rust/omega/pipeline/optimization",
    "source/omega-rust/omega/representations/omega-legalized-operations",
    "source/omega-rust/omega/representations/omega-optimization-core",
    "source/omega-rust/omega/representations/omega-optimization-unit",
    "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations",
    "source/omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations",
    "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations",
    "source/omega-rust/omega/pipeline/omega-target-operations-to-assigned-target-operations",
    "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions",
    "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact",
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
        entrance: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/mod.rs",
        catalog: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/catalog.rs",
        coordination_marker: "pub fn legalize_target_operations",
        catalog_marker: "LEGALIZATION_FORMS",
        next_rungs: &[
            "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/source",
            "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/replay",
        ],
    },
    RuleStageDescriptor {
        entrance: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/mod.rs",
        catalog: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/catalog.rs",
        coordination_marker: "pub fn built_in_psi_registries",
        catalog_marker: "PSI_PASS_CATALOG",
        next_rungs: &[
            "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes",
        ],
    },
    RuleStageDescriptor {
        entrance: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/selected_lowering/mod.rs",
        catalog: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/selected_lowering/catalog.rs",
        coordination_marker: "pub fn resolve_selected_lowering_rules",
        catalog_marker: "SELECTED_LOWERING_RULE_CATALOG",
        next_rungs: &[
            "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/selected_lowering/literal_fold",
        ],
    },
    RuleStageDescriptor {
        entrance: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/mod.rs",
        catalog: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/catalog.rs",
        coordination_marker: "pub fn selected_allocation_recovery_rule",
        catalog_marker: "ALLOCATION_RECOVERY_RULE_CATALOG",
        next_rungs: &[
            "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy",
            "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/pressure_rematerialization",
        ],
    },
    RuleStageDescriptor {
        entrance: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/mod.rs",
        catalog: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/catalog.rs",
        coordination_marker: "pub fn selected_post_allocation_machine_rule",
        catalog_marker: "POST_ALLOCATION_MACHINE_RULE_CATALOG",
        next_rungs: &[
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64",
            "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/x86_64",
        ],
    },
    RuleStageDescriptor {
        entrance: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/layout/x86_branch_relaxation/mod.rs",
        catalog: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/layout/x86_branch_relaxation/catalog.rs",
        coordination_marker: "pub fn stage_optimized_x86_branch_relaxation",
        catalog_marker: "FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG",
        next_rungs: &[
            "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/layout/x86_branch_relaxation/compute.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/layout/x86_branch_relaxation/validation.rs",
        ],
    },
];

pub(super) fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|candidate| {
            candidate.join("Cargo.toml").is_file() && candidate.join("source/omega-rust").is_dir()
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
