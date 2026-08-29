//! Repository guard for the optimizer source-navigation contract.
//!
//! The governing design brief is
//! `wiki/design_briefs/optimizer/source_organization.md`. Optimizer source
//! files have a hard size ceiling, while `lib.rs` and `mod.rs` entrances have
//! a tighter default ceiling. A short, exact exception table permits an
//! entrance to cross the preferred 100-line boundary only when that entrance
//! still owns one stated semantic coordination responsibility.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_RUST_FILE_LINES: usize = 1_500;
const PREFERRED_ENTRANCE_LINES: usize = 100;
const MAX_ENTRANCE_LINES: usize = 200;

/// The optimizer surfaces whose source organization is architecture-governed.
/// Keep these roots explicit: silently losing a moved or renamed tree must
/// fail this test rather than shrinking its jurisdiction.
const GOVERNED_ROOTS: &[&str] = &[
    "source/omega-rust/omega/pipeline/optimization",
    "source/omega-rust/omega/representations/omega-optimization-core",
    "source/omega-rust/omega/representations/omega-optimization-unit",
    "source/omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations",
];

#[derive(Clone, Copy)]
struct EntranceException {
    path: &'static str,
    ceiling: usize,
    semantic_reason: &'static str,
}

/// Exact exceptions to the preferred 100-line entrance ceiling.
///
/// An exception is stale when its file disappears, ceases to be an entrance,
/// or returns to 100 lines or fewer. Ceilings may never exceed the hard
/// 200-line entrance limit.
const ENTRANCE_EXCEPTIONS: &[EntranceException] = &[
    EntranceException {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/assembly/mod.rs",
        ceiling: 120,
        semantic_reason: "owns the paired build-and-validate orchestration seam for function-relative realization",
    },
    EntranceException {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/tests/mod.rs",
        ceiling: 170,
        semantic_reason: "owns shared validated-unit construction helpers consumed by the validation test leaves",
    },
];

struct RequiredCoordinationEntrance {
    path: &'static str,
    coordination_marker: &'static str,
}

/// Entrances that must visibly own a real stage join or catalog route. Merely
/// keeping these paths small is insufficient: deleting the coordination seam
/// and leaving a re-export wall must fail this architecture test.
const REQUIRED_COORDINATION_ENTRANCES: &[RequiredCoordinationEntrance] = &[
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/mod.rs",
        coordination_marker: "pub fn built_in_psi_registries",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/pressure_rematerialization/mod.rs",
        coordination_marker: "pub fn rematerialize_selected_active_resident",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/register_environment/mod.rs",
        coordination_marker: "pub fn baseline_target_register_environment",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/selection/selection/mod.rs",
        coordination_marker: "pub fn stage_optimized_instruction_selection",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/selection/optimized_target_operations/mod.rs",
        coordination_marker: "pub fn lower_optimized_to_target_operations",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/selection/assignment/mod.rs",
        coordination_marker: "fn stage_optimized_assignment",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/allocation_legality/mod.rs",
        coordination_marker: "pub fn stage_optimized_allocation_legality_with_availability",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/register_homes/mod.rs",
        coordination_marker: "pub fn stage_optimized_register_homes",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/liveness/mod.rs",
        coordination_marker: "pub fn stage_optimized_liveness",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/live_ranges/mod.rs",
        coordination_marker: "pub fn stage_optimized_live_ranges",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/fixed_view_copies/mod.rs",
        coordination_marker: "pub fn stage_optimized_fixed_view_copies",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/selected_reanalysis/mod.rs",
        coordination_marker: "pub fn stage_optimized_selected_reanalysis",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/machine/post_allocation_optimizations/mod.rs",
        coordination_marker: "OptimizedPostAllocationMachineOptimizationError",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/machine/post_allocation_machine_effects/mod.rs",
        coordination_marker: "fn seal_staged_post_allocation_machine",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/machine/active_resident_rematerialization/mod.rs",
        coordination_marker: "pub fn stage_optimized_active_resident_rematerialization",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/machine/machine_effects/mod.rs",
        coordination_marker: "fn admit_machine_effects",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/machine/literal_folds/mod.rs",
        coordination_marker: "pub fn run_selected_lowering_optimizations",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/machine/literal_fold_homes/mod.rs",
        coordination_marker: "pub fn stage_optimized_register_homes_after_literal_folds",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/encoding/active_resident_selected_form_encoding/mod.rs",
        coordination_marker: "pub fn stage_optimized_active_resident_rematerialization_selected_form_encoding",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/encoding/post_allocation_selected_form_encoding/mod.rs",
        coordination_marker: "stage_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/layout/active_resident_resolved_selected_form_layout/mod.rs",
        coordination_marker: "pub fn stage_optimized_active_resident_rematerialization_resolved_selected_form_layout",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/layout/resolved_selected_form_layout/mod.rs",
        coordination_marker: "stage_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/layout/x86_branch_relaxation/mod.rs",
        coordination_marker: "stage_optimized_x86_branch_relaxation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/artifacts/function_fragment_emission/mod.rs",
        coordination_marker: "stage_optimized_function_fragment_emission",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/artifacts/function_fragment_text_section/mod.rs",
        coordination_marker: "stage_optimized_relocation_free_text_section",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/artifacts/function_fragment_object_container/mod.rs",
        coordination_marker: "stage_optimized_relocation_free_object_container",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/artifacts/object_artifact/mod.rs",
        coordination_marker: "stage_validated_optimized_object_artifact",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/ordinary_callable_entry/mod.rs",
        coordination_marker: "stage_validated_optimized_ordinary_callable_entry",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/structural_unit_function_relative_realization/mod.rs",
        coordination_marker: "pub fn stage_optimized_structural_unit_function_relative_realization",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/active_resident_function_relative_realization/mod.rs",
        coordination_marker: "pub fn stage_optimized_active_resident_rematerialization_function_relative_realization",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/unit_function_relative_realization/mod.rs",
        coordination_marker: "pub fn stage_optimized_unit_function_relative_realization",
    },
];

/// Every Psi pass owns its rule order immediately below its named folder.
const REQUIRED_PSI_PASS_CATALOGS: &[&str] = &[
    "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/control_flow_cleanup/catalog.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/copy_propagation/catalog.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/dead_scalar_elimination/catalog.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/catalog.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/proof_check_elision/catalog.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/catalog.rs",
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|candidate| {
            candidate.join("Cargo.toml").is_file() && candidate.join("source/omega-rust").is_dir()
        })
        .expect("architecture tests must run from within the Omega repository")
        .to_path_buf()
}

fn collect_rust_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
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

fn repository_relative_path(repository: &Path, path: &Path) -> Result<String, String> {
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

fn is_entrance(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|file_name| file_name == "lib.rs" || file_name == "mod.rs")
}

#[test]
fn optimizer_source_organization_is_bounded_and_navigable() {
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

    let mut exceptions = BTreeMap::<&str, &EntranceException>::new();
    for exception in ENTRANCE_EXCEPTIONS {
        if exceptions.insert(exception.path, exception).is_some() {
            violations.insert(format!("duplicate entrance exception: {}", exception.path));
        }
        if !(PREFERRED_ENTRANCE_LINES + 1..=MAX_ENTRANCE_LINES).contains(&exception.ceiling) {
            violations.insert(format!(
                "invalid entrance exception ceiling {} for {} (must be {}..={})",
                exception.ceiling,
                exception.path,
                PREFERRED_ENTRANCE_LINES + 1,
                MAX_ENTRANCE_LINES
            ));
        }
        if exception.semantic_reason.trim().is_empty() {
            violations.insert(format!(
                "entrance exception lacks a semantic reason: {}",
                exception.path
            ));
        }
    }

    let mut observed_exceptions = BTreeSet::new();
    for (path, lines) in &source_lines {
        if *lines > MAX_RUST_FILE_LINES {
            violations.insert(format!(
                "Rust file exceeds {MAX_RUST_FILE_LINES} lines: {path} ({lines})"
            ));
        }

        if !is_entrance(path) {
            continue;
        }
        let exception = exceptions.get(path.as_str()).copied();
        if exception.is_some() {
            observed_exceptions.insert(path.as_str());
        }

        match *lines {
            0..=PREFERRED_ENTRANCE_LINES => {
                if let Some(exception) = exception {
                    violations.insert(format!(
                        "stale entrance exception: {} is now {} lines (reason: {})",
                        path, lines, exception.semantic_reason
                    ));
                }
            }
            lines @ 101..=MAX_ENTRANCE_LINES => match exception {
                None => {
                    violations.insert(format!(
                        "entrance exceeds the preferred {PREFERRED_ENTRANCE_LINES}-line limit without an exact exception: {path} ({lines})"
                    ));
                }
                Some(exception) if lines > exception.ceiling => {
                    violations.insert(format!(
                        "entrance exceeds its exception ceiling {}: {} ({lines}; reason: {})",
                        exception.ceiling, path, exception.semantic_reason
                    ));
                }
                Some(_) => {}
            },
            lines => {
                violations.insert(format!(
                    "entrance exceeds the hard {MAX_ENTRANCE_LINES}-line limit: {path} ({lines})"
                ));
            }
        }
    }

    for exception in ENTRANCE_EXCEPTIONS {
        if observed_exceptions.contains(exception.path) {
            continue;
        }
        match source_lines.get(exception.path) {
            None => {
                violations.insert(format!(
                    "stale entrance exception points to a missing or ungoverned file: {}",
                    exception.path
                ));
            }
            Some(_) => {
                violations.insert(format!(
                    "stale entrance exception points to a non-entrance Rust file: {}",
                    exception.path
                ));
            }
        }
    }

    for entrance in REQUIRED_COORDINATION_ENTRANCES {
        let Some(lines) = source_lines.get(entrance.path) else {
            violations.insert(format!(
                "missing required optimizer coordination entrance: {}",
                entrance.path
            ));
            continue;
        };
        if *lines > PREFERRED_ENTRANCE_LINES {
            violations.insert(format!(
                "required optimizer coordination entrance exceeds {PREFERRED_ENTRANCE_LINES} lines: {} ({lines})",
                entrance.path
            ));
        }
        match fs::read_to_string(repository.join(entrance.path)) {
            Ok(contents) if contents.contains(entrance.coordination_marker) => {}
            Ok(_) => {
                violations.insert(format!(
                    "optimizer entrance became a re-export wall: {} lacks `{}`",
                    entrance.path, entrance.coordination_marker
                ));
            }
            Err(error) => {
                violations.insert(format!(
                    "cannot read required optimizer entrance {}: {error}",
                    entrance.path
                ));
            }
        }
    }

    for catalog in REQUIRED_PSI_PASS_CATALOGS {
        match fs::read_to_string(repository.join(catalog)) {
            Ok(contents) if contents.contains("fn built_in_registrations") => {}
            Ok(_) => {
                violations.insert(format!(
                    "Psi pass catalog lacks its ordered registration point: {catalog}"
                ));
            }
            Err(error) => {
                violations.insert(format!(
                    "missing required Psi pass catalog {catalog}: {error}"
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "optimizer source organization violations:\n{}",
        violations.into_iter().collect::<Vec<_>>().join("\n")
    );
}
