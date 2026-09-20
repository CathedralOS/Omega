//! Optimizer module role: module catalog. One row per `mod` declared in
//! `rewrites/mod.rs` — the stage's complete rewrite-module inventory.
//!
//! `optimize_selected_instructions` executes the `Routed` rows through the
//! named production caller; `Orphaned` rows are invoked only from their own
//! tests and name the board item that owns the retain-or-delete decision
//! (a catalog row executed by the stage entrance, or removal). `Shared`
//! rows are audit and vocabulary helpers the rewrite modules read; they
//! carry no rewrite entrance of their own. The table below is the audit the
//! row counts and family owners on the optimizer board quote; the unit test
//! at the bottom reconciles it against `mod.rs`, so a rewrite module cannot
//! be added, renamed, or change status without a catalog row.

/// How a rewrite module reaches — or fails to reach — the production route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RewriteModuleRoute {
    /// An executable route reaches the module; names the production file
    /// whose call runs or selects it (repository-relative).
    Routed(&'static str),
    /// Called only from the module's own tests. Names the execution-board
    /// item that owns giving the module a stage-catalog row or deleting it.
    Orphaned(&'static str),
    /// Shared audit or vocabulary module with no rewrite entrance.
    Shared,
    /// `cfg(test)`-gated helpers; not a rewrite module.
    TestSupport,
}

/// One rewrite-module inventory row, in `mod.rs` declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RewriteModuleRow {
    /// The module name as declared in `mod.rs`.
    pub(crate) module: &'static str,
    pub(crate) route: RewriteModuleRoute,
}

pub(crate) const REWRITE_MODULE_CATALOG: &[RewriteModuleRow] = &[
    RewriteModuleRow {
        module: "address_fold",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "allocation_recovery",
        // `selected_allocation_recovery_rule` is consulted during native
        // phase selection and its `fixed_view_copy` materialization runs
        // under the routed `fixed_view` stage entrances.
        route: RewriteModuleRoute::Routed(
            "omega-rust/omega/compiler/native-realization/src/native_pipeline/physical_pipeline/phase_selections.rs",
        ),
    },
    RewriteModuleRow {
        module: "arm_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "block_edges",
        route: RewriteModuleRoute::Shared,
    },
    RewriteModuleRow {
        module: "boundary_boolean",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "boundary_branch",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "bypass_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "bypass_run_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "commuting_accesses",
        route: RewriteModuleRoute::Shared,
    },
    RewriteModuleRow {
        module: "commuting_interchange",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "commuting_member_run_interchange",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "commuting_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "commuting_run_interchange",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "commuting_run_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "condition_state",
        route: RewriteModuleRoute::Shared,
    },
    RewriteModuleRow {
        module: "confluence_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "confluence_run_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "constant_boolean",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "constant_branch",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "copy_removal",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "dead_compare",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "dead_path",
        route: RewriteModuleRoute::Shared,
    },
    RewriteModuleRow {
        module: "dead_store",
        route: RewriteModuleRoute::Orphaned("ALIAS-AWARE-MEMORY"),
    },
    RewriteModuleRow {
        module: "diamond_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "diamond_run_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "edge_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "edge_run_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "fixed_view",
        // Register allocation stages `stage_optimized_fixed_precolored_
        // segment_homes` and `stage_optimized_fixed_view_copies` from its
        // assignment recovery.
        route: RewriteModuleRoute::Routed(
            "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/assignment/recovery.rs",
        ),
    },
    RewriteModuleRow {
        module: "fork_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "fork_run_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "inflow_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "join_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "literal_folds",
        // The selected-lowering executor: `run_selected_lowering_
        // optimizations` is the only rewrite the stage entrance runs.
        route: RewriteModuleRoute::Routed(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/selected_optimization.rs",
        ),
    },
    RewriteModuleRow {
        module: "literal_minuend",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "load_forwarding",
        route: RewriteModuleRoute::Orphaned("ALIAS-AWARE-MEMORY"),
    },
    RewriteModuleRow {
        module: "local_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "local_schedule",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "member_run_interchange",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "module_catalog",
        route: RewriteModuleRoute::Shared,
    },
    RewriteModuleRow {
        module: "place_storage",
        route: RewriteModuleRoute::Shared,
    },
    RewriteModuleRow {
        module: "predecessor_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "predecessor_run_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "redundant_extension",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "run_interchange",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "run_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "runtime_rematerialization",
        // Register allocation replays `rematerialize_selected_runtime_value`
        // through its runtime-spill route.
        route: RewriteModuleRoute::Routed(
            "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/assignment/runtime_spill/replay.rs",
        ),
    },
    RewriteModuleRow {
        module: "runtime_spill",
        // Register allocation's `assignment/runtime_spill` executes and
        // replays `spill_selected_runtime_value`.
        route: RewriteModuleRoute::Routed(
            "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/assignment/runtime_spill/recovery.rs",
        ),
    },
    RewriteModuleRow {
        module: "selected_lowering",
        // Its rule catalog is resolved by the `literal_folds` executor and
        // consulted during native phase selection.
        route: RewriteModuleRoute::Routed(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/literal_folds/mod.rs",
        ),
    },
    RewriteModuleRow {
        module: "store_motion",
        route: RewriteModuleRoute::Orphaned("ALIAS-AWARE-MEMORY"),
    },
    RewriteModuleRow {
        module: "test_support",
        route: RewriteModuleRoute::TestSupport,
    },
    RewriteModuleRow {
        module: "triangle_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "window_hazards",
        route: RewriteModuleRoute::Shared,
    },
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use super::{REWRITE_MODULE_CATALOG, RewriteModuleRoute};

    fn declared_modules() -> Vec<String> {
        include_str!("mod.rs")
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                for prefix in ["mod ", "pub mod ", "pub(crate) mod "] {
                    if let Some(rest) = line.strip_prefix(prefix)
                        && let Some(name) = rest.strip_suffix(';')
                    {
                        return Some(name.to_string());
                    }
                }
                None
            })
            .collect()
    }

    fn repository_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .find(|candidate| candidate.join("omega-rust").is_dir())
            .expect("crate must live under the Omega repository")
            .to_path_buf()
    }

    /// Every `mod` declared in `rewrites/mod.rs` carries exactly one catalog
    /// row, in declaration order; no row names a module that is not declared.
    #[test]
    fn module_catalog_reconciles_with_mod_declarations() {
        let declared = declared_modules();
        let cataloged: Vec<&str> = REWRITE_MODULE_CATALOG
            .iter()
            .map(|row| row.module)
            .collect();

        let declared_set: BTreeSet<&str> = declared.iter().map(String::as_str).collect();
        let cataloged_set: BTreeSet<&str> = cataloged.iter().copied().collect();

        let missing: Vec<&&str> = declared_set.difference(&cataloged_set).collect();
        assert!(
            missing.is_empty(),
            "modules declared in rewrites/mod.rs without a catalog row: {missing:?}"
        );
        let stale: Vec<&&str> = cataloged_set.difference(&declared_set).collect();
        assert!(
            stale.is_empty(),
            "catalog rows for modules no longer declared: {stale:?}"
        );
        assert_eq!(
            declared.len(),
            cataloged.len(),
            "catalog must hold exactly one row per module (duplicate rows?)"
        );
        assert_eq!(
            declared.iter().map(String::as_str).collect::<Vec<_>>(),
            cataloged,
            "catalog row order must match mod.rs declaration order"
        );
    }

    /// A `Routed` row's named production caller must exist; an `Orphaned`
    /// row's owner must be a live execution-board item (`**ITEM.**` in
    /// `TASKS_OPTIMIZER.md`).
    #[test]
    fn module_catalog_dispositions_resolve() {
        let repository = repository_root();
        let board = std::fs::read_to_string(repository.join("TASKS_OPTIMIZER.md"))
            .expect("TASKS_OPTIMIZER.md must be readable");
        let mut violations = Vec::new();
        for row in REWRITE_MODULE_CATALOG {
            match row.route {
                RewriteModuleRoute::Routed(caller) => {
                    if !repository.join(caller).is_file() {
                        violations.push(format!(
                            "{}: routed caller does not exist: {caller}",
                            row.module
                        ));
                    }
                }
                RewriteModuleRoute::Orphaned(owner) => {
                    if !board.contains(&format!("**{owner}.**")) {
                        violations.push(format!(
                            "{}: orphan owner is not a TASKS_OPTIMIZER.md item: {owner}",
                            row.module
                        ));
                    }
                }
                RewriteModuleRoute::Shared | RewriteModuleRoute::TestSupport => {}
            }
        }
        assert!(
            violations.is_empty(),
            "rewrite module catalog violations:\n{}",
            violations.join("\n")
        );
    }
}
