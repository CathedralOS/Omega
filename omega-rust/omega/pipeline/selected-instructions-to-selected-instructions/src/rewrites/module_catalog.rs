//! Optimizer module role: module catalog. One row per `mod` declared in
//! `rewrites/mod.rs` and then in `rewrites/unexecuted/mod.rs` — the stage's
//! complete rewrite-module inventory and its disposition roster.
//!
//! `optimize_selected_instructions` executes the `Routed` rows through the
//! named production caller; every `Routed` row lives directly under
//! `rewrites/`. `Orphaned` rows are invoked only from their own tests and
//! name the board item that owns the retain-or-delete decision (a catalog
//! row executed by the stage entrance, or removal); every `Orphaned` row
//! lives under `rewrites/unexecuted/`, so the tree states the disposition
//! the roster records. `Shared` rows are audit and vocabulary helpers the
//! rewrite modules read; they carry no rewrite entrance of their own. The
//! table below is the audit the row counts and family owners on the
//! optimizer board quote; the unit tests at the bottom reconcile it against
//! both `mod.rs` files, so a rewrite module cannot be added, renamed, moved
//! between areas, or change status without a catalog row.

/// How a rewrite module reaches — or fails to reach — the production route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RewriteModuleRoute {
    /// An executable route reaches the module: `caller` names the production
    /// file whose call runs or selects it (repository-relative), and
    /// `evidence` is the route symbol that file must contain — the reconcile
    /// test fails a `Routed` row whose named caller stops calling it.
    Routed {
        caller: &'static str,
        evidence: &'static str,
    },
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
        module: "allocation_recovery",
        // `selected_allocation_recovery_rule` is consulted during native
        // phase selection and its `fixed_view_copy` materialization runs
        // under the routed `fixed_view` stage entrances.
        route: RewriteModuleRoute::Routed {
            caller: "omega-rust/omega/compiler/native-realization/src/native_pipeline/physical_pipeline/phase_selections.rs",
            evidence: "selected_allocation_recovery_rule",
        },
    },
    RewriteModuleRow {
        module: "block_edges",
        route: RewriteModuleRoute::Shared,
    },
    RewriteModuleRow {
        module: "catalog",
        route: RewriteModuleRoute::Shared,
    },
    RewriteModuleRow {
        module: "copy_removal",
        // The pre-allocation executor: `run_pre_allocation_optimizations` is
        // the rewrite the stage entrance runs for that slice.
        route: RewriteModuleRoute::Routed {
            caller: "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/selected_optimization.rs",
            evidence: "run_pre_allocation_optimizations",
        },
    },
    RewriteModuleRow {
        module: "fixed_view",
        // Register allocation stages `stage_optimized_fixed_precolored_
        // segment_homes` and `stage_optimized_fixed_view_copies` from its
        // assignment recovery.
        route: RewriteModuleRoute::Routed {
            caller: "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/assignment/recovery.rs",
            evidence: "stage_optimized_fixed_view_copies",
        },
    },
    RewriteModuleRow {
        module: "literal_folds",
        // The selected-lowering executor: `run_selected_lowering_
        // optimizations` is the only rewrite the stage entrance runs.
        route: RewriteModuleRoute::Routed {
            caller: "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/selected_optimization.rs",
            evidence: "run_selected_lowering_optimizations",
        },
    },
    RewriteModuleRow {
        module: "module_catalog",
        route: RewriteModuleRoute::Shared,
    },
    RewriteModuleRow {
        module: "pre_allocation",
        // The pre-allocation executor: `run_pre_allocation_optimizations` is
        // the rewrite the stage entrance runs for that slice.
        route: RewriteModuleRoute::Routed {
            caller: "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/selected_optimization.rs",
            evidence: "run_pre_allocation_optimizations",
        },
    },
    RewriteModuleRow {
        module: "redundant_extension",
        // The pre-allocation executor: its discovery pass calls
        // `remove_selected_redundant_extension` for the exact rule the
        // catalog admits.
        route: RewriteModuleRoute::Routed {
            caller: "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/pre_allocation/execution.rs",
            evidence: "remove_selected_redundant_extension",
        },
    },
    RewriteModuleRow {
        module: "runtime_rematerialization",
        // Register allocation replays `rematerialize_selected_runtime_value`
        // through its runtime-spill route.
        route: RewriteModuleRoute::Routed {
            caller: "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/assignment/runtime_spill/replay.rs",
            evidence: "rematerialize_selected_runtime_value",
        },
    },
    RewriteModuleRow {
        module: "runtime_spill",
        // Register allocation's `assignment/runtime_spill` executes and
        // replays `spill_selected_runtime_value`.
        route: RewriteModuleRoute::Routed {
            caller: "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/assignment/runtime_spill/recovery.rs",
            evidence: "spill_selected_runtime_value",
        },
    },
    RewriteModuleRow {
        module: "selected_lowering",
        // Its rule catalog is resolved by the `literal_folds` executor and
        // consulted during native phase selection.
        route: RewriteModuleRoute::Routed {
            caller: "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/literal_folds/mod.rs",
            evidence: "resolve_selected_lowering_rules",
        },
    },
    RewriteModuleRow {
        module: "test_support",
        route: RewriteModuleRoute::TestSupport,
    },
    RewriteModuleRow {
        module: "unexecuted",
        // The catalogued-but-unexecuted area; its own rows follow.
        route: RewriteModuleRoute::Shared,
    },
    RewriteModuleRow {
        module: "window_hazards",
        route: RewriteModuleRoute::Shared,
    },
    RewriteModuleRow {
        module: "address_fold",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "arm_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
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
        module: "commuting_accesses",
        route: RewriteModuleRoute::Shared,
    },
    RewriteModuleRow {
        module: "commuting_relocation",
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
        module: "constant_boolean",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "constant_branch",
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
        module: "inflow_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "interchange",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "load_forwarding",
        route: RewriteModuleRoute::Orphaned("ALIAS-AWARE-MEMORY"),
    },
    RewriteModuleRow {
        // The four declarative pair families beyond the literal-fold
        // grammar (`condition_materialization`, `copied_call_operand`,
        // `projected_access`, `terminator_pair`) and their shared
        // `condition_flow` walk.
        module: "peepholes",
        route: RewriteModuleRoute::Orphaned("DECLARATIVE-PEEPHOLES"),
    },
    RewriteModuleRow {
        module: "place_storage",
        route: RewriteModuleRoute::Shared,
    },
    RewriteModuleRow {
        // The one member-run relocation admission every scheduling family
        // delegates to once the window derivation is shared.
        module: "relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "scheduled_relocation",
        route: RewriteModuleRoute::Orphaned("EXACT-MACHINE-SIMPLIFICATIONS"),
    },
    RewriteModuleRow {
        module: "store_motion",
        route: RewriteModuleRoute::Orphaned("ALIAS-AWARE-MEMORY"),
    },
];

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    use super::{REWRITE_MODULE_CATALOG, RewriteModuleRoute};

    /// Modules declared in `rewrites/mod.rs` followed by those declared in
    /// `rewrites/unexecuted/mod.rs`; the second list is the unexecuted area.
    fn declared_modules() -> Vec<String> {
        let mut modules = declarations(include_str!("mod.rs"));
        modules.extend(declarations(include_str!("unexecuted/mod.rs")));
        modules
    }

    fn unexecuted_modules() -> BTreeSet<String> {
        declarations(include_str!("unexecuted/mod.rs"))
            .into_iter()
            .collect()
    }

    fn declarations(source: &str) -> Vec<String> {
        source
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

        let rewrites_dir = repository_root().join(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites",
        );
        let unexecuted = unexecuted_modules();
        let missing_backing: Vec<&String> = declared
            .iter()
            .filter(|name| {
                let dir = if unexecuted.contains(*name) {
                    rewrites_dir.join("unexecuted")
                } else {
                    rewrites_dir.clone()
                };
                !dir.join(format!("{name}.rs")).is_file()
                    && !dir.join(name).join("mod.rs").is_file()
            })
            .collect();
        assert!(
            missing_backing.is_empty(),
            "declared modules with no backing file (<name>.rs or <name>/mod.rs): {missing_backing:?}"
        );

        // The tree states the disposition: every executed (`Routed`) family
        // sits directly under `rewrites/`, every `Orphaned` family under
        // `rewrites/unexecuted/`.
        let misplaced: Vec<String> = REWRITE_MODULE_CATALOG
            .iter()
            .filter_map(|row| match row.route {
                RewriteModuleRoute::Routed { .. } if unexecuted.contains(row.module) => Some(
                    format!("{}: routed family declared under unexecuted/", row.module),
                ),
                RewriteModuleRoute::Orphaned(_) if !unexecuted.contains(row.module) => {
                    Some(format!(
                        "{}: unrouted family declared outside unexecuted/",
                        row.module
                    ))
                }
                _ => None,
            })
            .collect();
        assert!(
            misplaced.is_empty(),
            "rewrite families declared in the wrong area:\n{}",
            misplaced.join("\n")
        );
    }

    /// A `Routed` row's named production caller must exist and still contain
    /// the row's route symbol; an `Orphaned` row's owner must be a live
    /// execution-board item (`**ITEM.**` in `TASKS_OPTIMIZER.md`).
    #[test]
    fn module_catalog_dispositions_resolve() {
        let repository = repository_root();
        let board = std::fs::read_to_string(repository.join("TASKS_OPTIMIZER.md"))
            .expect("TASKS_OPTIMIZER.md must be readable");
        let mut violations = Vec::new();
        for row in REWRITE_MODULE_CATALOG {
            match row.route {
                RewriteModuleRoute::Routed { caller, evidence } => {
                    match std::fs::read_to_string(repository.join(caller)) {
                        Ok(contents) if contents.contains(evidence) => {}
                        Ok(_) => violations.push(format!(
                            "{}: routed caller {caller} no longer contains route symbol `{evidence}`",
                            row.module
                        )),
                        Err(_) => violations.push(format!(
                            "{}: routed caller does not exist: {caller}",
                            row.module
                        )),
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

    /// An `Orphaned` row must stay called only from tests. A production-file
    /// reference to one of its public functions means the module gained a
    /// route: the row must move to `Routed { caller, evidence }` rather than
    /// let the catalog go stale.
    #[test]
    fn orphaned_modules_have_no_production_callers() {
        let repository = repository_root();
        let rewrites_dir = repository.join(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites",
        );

        /// Public functions declared by a source file (`pub fn name`).
        fn public_functions(contents: &str) -> Vec<String> {
            contents
                .match_indices("pub fn ")
                .filter_map(|(offset, _)| {
                    let name: String = contents[offset + "pub fn ".len()..]
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    (!name.is_empty()).then_some(name)
                })
                .collect()
        }

        /// One module's backing sources: its `<module>.rs` root plus the
        /// `<module>/**/*.rs` members it declares — test members excluded
        /// (their helpers are not entrances).
        fn module_files(module: &str, rewrites_dir: &Path) -> Vec<PathBuf> {
            let mut files = Vec::new();
            let rewrites_dir = &if unexecuted_modules().contains(module) {
                rewrites_dir.join("unexecuted")
            } else {
                rewrites_dir.to_path_buf()
            };
            let root = rewrites_dir.join(format!("{module}.rs"));
            if root.is_file() {
                files.push(root);
            }
            collect_rust_files(&rewrites_dir.join(module), &mut files);
            files.retain(|path| path.file_name().unwrap() != "tests.rs");
            files
        }

        fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_rust_files(&path, out);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    out.push(path);
                }
            }
        }

        // Function name → set of cataloged modules defining it. A name defined
        // by exactly one module is that module's entrance; shared result
        // accessors (`transformed`, `shared_transformed`, …) are defined by
        // many modules and are not call evidence.
        let mut definitions: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut backing: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
        for module in declared_modules() {
            let files = module_files(&module, &rewrites_dir);
            for path in &files {
                let contents = std::fs::read_to_string(path).unwrap_or_default();
                for name in public_functions(&contents) {
                    definitions.entry(name).or_default().insert(module.clone());
                }
            }
            backing.insert(module, files);
        }

        // Production callers are workspace Rust files outside test code; an
        // orphan's own backing files always name its functions by definition.
        let mut rust_files = Vec::new();
        collect_rust_files(&repository.join("omega-rust"), &mut rust_files);
        let is_test_file = |path: &Path| {
            path.components().any(|component| {
                component.as_os_str() == "tests" || component.as_os_str() == "test_support"
            }) || path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with("tests.rs")
        };

        let identifier = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        /// Drop `use …;` items: a named re-export or import is wiring, not a
        /// call; a production caller is caught at its call site.
        fn without_use_items(contents: &str) -> String {
            let mut out = String::with_capacity(contents.len());
            let mut rest = contents;
            while let Some(start) = rest.find("use ") {
                let at_item_start = rest[..start].rsplit('\n').next().is_some_and(|prefix| {
                    prefix.trim().is_empty()
                        || prefix.trim() == "pub"
                        || prefix.trim().starts_with("pub(")
                });
                let Some(end) = rest[start..].find(';') else {
                    break;
                };
                if at_item_start {
                    out.push_str(&rest[..start]);
                } else {
                    out.push_str(&rest[..start + end + 1]);
                }
                rest = &rest[start + end + 1..];
            }
            out.push_str(rest);
            out
        }
        let mut violations = Vec::new();
        for row in REWRITE_MODULE_CATALOG {
            let RewriteModuleRoute::Orphaned(_) = row.route else {
                continue;
            };
            let entrances: Vec<&str> = definitions
                .iter()
                .filter(|(_, owners)| owners.len() == 1 && owners.contains(row.module))
                .map(|(name, _)| name.as_str())
                .collect();
            let own: BTreeSet<&PathBuf> = backing[row.module].iter().collect();
            for file in rust_files
                .iter()
                .filter(|f| !is_test_file(f) && !own.contains(*f))
            {
                let contents =
                    without_use_items(&std::fs::read_to_string(file).unwrap_or_default());
                for entrance in &entrances {
                    let called = contents.match_indices(entrance).any(|(offset, _)| {
                        !identifier(contents[..offset].chars().next_back())
                            && !identifier(contents[offset + entrance.len()..].chars().next())
                    });
                    if called {
                        violations.push(format!(
                            "{}: orphaned entrance `{entrance}` is named by production file {}; \
                             promote the row to Routed or move the call back under test code",
                            row.module,
                            file.strip_prefix(&repository).unwrap_or(file).display(),
                        ));
                    }
                }
            }
        }
        assert!(
            violations.is_empty(),
            "orphaned rewrite modules with production callers:\n{}",
            violations.join("\n")
        );
    }
}
