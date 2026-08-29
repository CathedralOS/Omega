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

const MAX_PRODUCTION_RUST_FILE_LINES: usize = 1_000;
const MAX_LEGACY_PRODUCTION_RUST_FILE_LINES: usize = 1_300;
const MAX_TEST_RUST_FILE_LINES: usize = 1_500;
const PREFERRED_ENTRANCE_LINES: usize = 100;
const MAX_ENTRANCE_LINES: usize = 200;

/// Exact production leaves that still exceed the default ceiling.
///
/// Each ceiling is pinned to the current file size. An exception cannot grow,
/// and becomes stale as soon as the file is split below the default. New files
/// never enter this table.
const LEGACY_PRODUCTION_FILE_CEILINGS: &[(&str, usize)] = &[];

/// The optimizer surfaces whose source organization is architecture-governed.
/// Keep these roots explicit: silently losing a moved or renamed tree must
/// fail this test rather than shrinking its jurisdiction.
const GOVERNED_ROOTS: &[&str] = &[
    "source/omega-rust/omega/pipeline/optimization",
    "source/omega-rust/omega/representations/omega-optimization-core",
    "source/omega-rust/omega/representations/omega-optimization-unit",
    "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations",
    "source/omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations",
    "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations",
    "source/omega-rust/omega/pipeline/omega-target-operations-to-assigned-target-operations",
    "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions",
    "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact",
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
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/replay/mod.rs",
        ceiling: 120,
        semantic_reason: "owns whole-plan independent replay across scalar, Unit, and structural Unit function families",
    },
    EntranceException {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/source/mod.rs",
        ceiling: 160,
        semantic_reason: "owns the three canonical source-projection rosters and their shared custody preflight",
    },
    EntranceException {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/validation/mod.rs",
        ceiling: 140,
        semantic_reason: "owns complete selected-plan custody, roster traversal, independent validation, and receipt admission",
    },
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
        path: "source/omega-rust/omega/representations/omega-optimization-unit/src/model.rs",
        coordination_marker: "pub struct PsiOptimizationUnit",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/representations/omega-optimization-unit/src/rewrite/candidate/mod.rs",
        coordination_marker: "fn new(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/mod.rs",
        coordination_marker: "fn integer_evaluation_contract",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/live_ranges/validate.rs",
        coordination_marker: "pub fn validate_live_ranges",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/live_ranges/compute.rs",
        coordination_marker: "pub(crate) fn compute_terminal_live_ranges",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/liveness/validate/mod.rs",
        coordination_marker: "pub fn validate_liveness",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/src/artifact/mod.rs",
        coordination_marker: "pub fn lower_artifact_sections",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/src/optimization/mod.rs",
        coordination_marker: "pub fn build_verified_psi_optimization_unit",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/src/provider_installation/mod.rs",
        coordination_marker: "pub fn admit_provider_installation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/src/lowering/mod.rs",
        coordination_marker: "pub(crate) fn lower_decoded_verified_module",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/src/lowering/machine.rs",
        coordination_marker: "pub(super) fn lower_machine",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-assigned-target-operations/src/assignment/mod.rs",
        coordination_marker: "pub fn assign_registers",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-assigned-target-operations/src/assignment/function/mod.rs",
        coordination_marker: "pub(super) fn assign_function",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-assigned-target-operations/src/assignment/function/operation_routes.rs",
        coordination_marker: "pub(super) fn assign_operation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/mod.rs",
        coordination_marker: "pub fn lower_to_target_operations_with_provider_executions_and_installation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/function/mod.rs",
        coordination_marker: "pub(super) fn lower_function",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/scalar/mod.rs",
        coordination_marker: "pub(super) fn lower_scalar_function",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/scalar/straight_line/mod.rs",
        coordination_marker: "pub(super) fn lower_straight_line",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/scalar/conditional_scalar/mod.rs",
        coordination_marker: "pub(super) fn lower_conditional_scalar_operation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/structural/mod.rs",
        coordination_marker: "pub(super) fn lower_structural_function",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/mod.rs",
        coordination_marker: "pub fn legalize_target_operations",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/replay/leaf/mod.rs",
        coordination_marker: "pub(super) fn replay_leaf",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/mod.rs",
        coordination_marker: "pub fn select_instructions",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/mod.rs",
        coordination_marker: "pub fn built_in_psi_registries",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/mod.rs",
        coordination_marker: "pub fn selected_allocation_recovery_rule",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/mod.rs",
        coordination_marker: "pub fn selected_lowering_rule_policy",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/mod.rs",
        coordination_marker: "pub fn selected_post_allocation_machine_rule",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/layout/x86_branch_relaxation/mod.rs",
        coordination_marker: "pub fn stage_optimized_x86_branch_relaxation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/pressure_rematerialization/mod.rs",
        coordination_marker: "pub fn rematerialize_selected_active_resident",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/prephysical_manifest/mod.rs",
        coordination_marker: "pub fn project_pre_physical_optimization_manifest",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/rewrite_accounting/mod.rs",
        coordination_marker: "fn preserve_edge_custody",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/global_value_numbering/mod.rs",
        coordination_marker: "fn validate_candidate_origin",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/dead_scalar_elimination/mod.rs",
        coordination_marker: "fn validate_candidate_contract",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/copy_propagation/mod.rs",
        coordination_marker: "fn validate_candidate_contract",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/operation_contracts/mod.rs",
        coordination_marker: "fn validate_values_and_bindings",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/function_structure/mod.rs",
        coordination_marker: "pub(crate) fn validate_function",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/derived_metadata/mod.rs",
        coordination_marker: "pub(crate) fn validate_places_and_claims",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/context/mod.rs",
        coordination_marker: "fn validate_psi_optimization_unit_with_context",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/core/mod.rs",
        coordination_marker: "pub fn validate_psi_optimization_unit",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/current_value_ranges/mod.rs",
        coordination_marker: "pub fn validate_current_value_range_fact",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/projection/mod.rs",
        coordination_marker: "pub fn validate_optimized_abstract_plan_projection",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/structural_catalog/mod.rs",
        coordination_marker: "fn index_structural_catalogs",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/current_ownership/mod.rs",
        coordination_marker: "fn validate_current_ownership_frontier",
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
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/layout/whole_function_exit_contract/mod.rs",
        coordination_marker: "stage_whole_function_exit_contract_with_post_allocation_machine_optimization",
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
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/allocation_recovery_function_relative_realization/mod.rs",
        coordination_marker: "pub fn stage_allocation_recovery_function_relative_realization",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/coordination/physical_pipeline/routes/allocation_recovery/mod.rs",
        coordination_marker: "fn stage_allocation_recovery_pipeline",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/unit_function_relative_realization/mod.rs",
        coordination_marker: "pub fn stage_optimized_unit_function_relative_realization",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/entry_settlement/mod.rs",
        coordination_marker: "pub fn validate_native_program_entry_settlement",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/mod.rs",
        coordination_marker: "pub fn realize_native_artifact",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/mod.rs",
        coordination_marker: "pub(crate) fn admit_native_providers",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/optimized_semantic_wrapper_encoding/mod.rs",
        coordination_marker: "pub fn select_optimized_program_storage_semantic_wrapper_encoding",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/optimized_semantic_wrapper_object/mod.rs",
        coordination_marker: "pub fn stage_validated_optimized_program_storage_semantic_wrapper_object",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/optimized_semantic_wrapper_object/validation/mod.rs",
        coordination_marker: "pub fn validate_optimized_program_storage_semantic_wrapper_object",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/optimized_semantic_wrapper_object/object/mod.rs",
        coordination_marker: "pub(crate) fn construct_object",
    },
];

struct RequiredRuleCatalog {
    path: &'static str,
    order_marker: &'static str,
}

/// Every rule-owning entrance keeps its only order table immediately below
/// that entrance. Pipeline custody code may consume these catalogs but may not
/// replace them with a second selection table.
const REQUIRED_RULE_CATALOGS: &[RequiredRuleCatalog] = &[
    RequiredRuleCatalog {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/catalog.rs",
        order_marker: "SCALAR_LEGALIZATION_FORMS",
    },
    RequiredRuleCatalog {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/catalog.rs",
        order_marker: "ALLOCATION_RECOVERY_RULE_CATALOG",
    },
    RequiredRuleCatalog {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/catalog.rs",
        order_marker: "SELECTED_LOWERING_RULE_CATALOG",
    },
    RequiredRuleCatalog {
        path: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/catalog.rs",
        order_marker: "POST_ALLOCATION_MACHINE_RULE_CATALOG",
    },
    RequiredRuleCatalog {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/catalog.rs",
        order_marker: "PSI_PASS_CATALOG",
    },
    RequiredRuleCatalog {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/layout/x86_branch_relaxation/catalog.rs",
        order_marker: "FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG",
    },
    RequiredRuleCatalog {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/control_flow_cleanup/catalog.rs",
        order_marker: "fn built_in_registrations",
    },
    RequiredRuleCatalog {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/copy_propagation/catalog.rs",
        order_marker: "fn built_in_registrations",
    },
    RequiredRuleCatalog {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/dead_scalar_elimination/catalog.rs",
        order_marker: "fn built_in_registrations",
    },
    RequiredRuleCatalog {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/catalog.rs",
        order_marker: "fn built_in_registrations",
    },
    RequiredRuleCatalog {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/proof_check_elision/catalog.rs",
        order_marker: "fn built_in_registrations",
    },
    RequiredRuleCatalog {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/catalog.rs",
        order_marker: "fn built_in_registrations",
    },
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

fn is_test_source(path: &str) -> bool {
    path.contains("/tests/") || path.ends_with("/tests.rs") || path.ends_with("_tests.rs")
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

    let mut legacy_file_ceilings = BTreeMap::<&str, usize>::new();
    for (path, ceiling) in LEGACY_PRODUCTION_FILE_CEILINGS {
        if legacy_file_ceilings.insert(path, *ceiling).is_some() {
            violations.insert(format!(
                "duplicate legacy production-file exception: {path}"
            ));
        }
        if !(MAX_PRODUCTION_RUST_FILE_LINES + 1..=MAX_LEGACY_PRODUCTION_RUST_FILE_LINES)
            .contains(ceiling)
        {
            violations.insert(format!(
                "invalid legacy production-file ceiling {ceiling} for {path}"
            ));
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

    let mut observed_legacy_files = BTreeSet::new();
    let mut observed_exceptions = BTreeSet::new();
    for (path, lines) in &source_lines {
        let ceiling = if is_test_source(path) {
            MAX_TEST_RUST_FILE_LINES
        } else {
            match legacy_file_ceilings.get(path.as_str()) {
                Some(ceiling) => {
                    observed_legacy_files.insert(path.as_str());
                    *ceiling
                }
                None => MAX_PRODUCTION_RUST_FILE_LINES,
            }
        };
        if *lines > ceiling {
            violations.insert(format!(
                "Rust file exceeds its {ceiling}-line ceiling: {path} ({lines})"
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

    for (path, _) in LEGACY_PRODUCTION_FILE_CEILINGS {
        let path = *path;
        if !observed_legacy_files.contains(path) {
            violations.insert(format!(
                "stale legacy production-file exception (missing, ungoverned, test-only, or now below {MAX_PRODUCTION_RUST_FILE_LINES} lines): {path}"
            ));
            continue;
        }
        if source_lines[path] <= MAX_PRODUCTION_RUST_FILE_LINES {
            violations.insert(format!(
                "stale legacy production-file exception: {path} is now {} lines",
                source_lines[path]
            ));
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

    assert!(
        violations.is_empty(),
        "optimizer source organization violations:\n{}",
        violations.into_iter().collect::<Vec<_>>().join("\n")
    );
}
