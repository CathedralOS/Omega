//! Per-rule coverage-axis custody for the PER-RULE-COVERAGE matrix.
//!
//! The release inventory proves each exact rule has a row in `rules.md`;
//! this table proves each rule carries the board's full coverage matrix —
//! positive, negative, boundary, disabled, budget, determinism,
//! fixed-point, and corruption legs — by naming the test that exercises
//! each axis. The check resolves every named test against the source tree,
//! so a rule that lands without completing its row, a row that keeps a
//! renamed or deleted covering test, and an absent axis that does not
//! record why the axis cannot exist all fail this repository gate instead
//! of drifting as prose.
//!
//! Rows are keyed by the canonical `Optimization` case name. The mandatory
//! `runtime_spill` and `runtime_rematerialization` recovery rewrites own no
//! selection member, so they key by module name and record the disabled
//! axis as absent by design. Uncalled rewrite modules are not rows: one
//! becomes a row automatically when it gains a selection name and enters
//! the canonical vocabulary.

use std::collections::BTreeMap;
use std::fs;

use super::inventory::CanonicalRule;
use crate::Audit;

/// One axis leg of a rule's coverage row.
#[derive(Debug, Clone, Copy)]
enum Leg {
    /// `file` contains `fn <test>(` — the named test exercises this axis.
    Covered {
        file: &'static str,
        test: &'static str,
    },
    /// The axis does not exist for this rule, for the recorded reason.
    Absent(AbsentReason),
}

/// Why an axis does not exist for a rule. The closed set is deliberate:
/// recording an axis as absent requires a sanctioned reason here, so a
/// merely unwritten leg cannot hide behind prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AbsentReason {
    /// The rule owns no `Optimization` selection member — it is a mandatory
    /// recovery rewrite applied by the stage itself — so no
    /// selection-policy axis exists. Never legal for a canonical rule.
    NoSelectionVocabulary,
    /// The owning phase validator carries no measured work budget, so the
    /// budget axis does not exist. Only legal on the `budget` axis.
    NoStepBudget,
}

const fn covered(file: &'static str, test: &'static str) -> Leg {
    Leg::Covered { file, test }
}

const NO_SELECTION_VOCABULARY: Leg = Leg::Absent(AbsentReason::NoSelectionVocabulary);
const NO_STEP_BUDGET: Leg = Leg::Absent(AbsentReason::NoStepBudget);

/// One rule's complete coverage row, in the board's axis order.
struct RuleCoverage {
    rule: &'static str,
    positive: Leg,
    negative: Leg,
    boundary: Leg,
    disabled: Leg,
    budget: Leg,
    determinism: Leg,
    fixed_point: Leg,
    corruption: Leg,
}

impl RuleCoverage {
    fn axes(&self) -> [(&'static str, Leg); 8] {
        [
            ("positive", self.positive),
            ("negative", self.negative),
            ("boundary", self.boundary),
            ("disabled", self.disabled),
            ("budget", self.budget),
            ("determinism", self.determinism),
            ("fixed-point", self.fixed_point),
            ("corruption", self.corruption),
        ]
    }
}

/// Mandatory recovery rewrites the stage applies without a selection name.
/// Each entry must still resolve to a rewrite module under `REWRITES` so a
/// retired rewrite cannot keep a stale row.
const MANDATORY_RECOVERY_REWRITES: &[&str] = &["runtime_rematerialization", "runtime_spill"];

const REWRITES: &str =
    "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites";

const COVERAGE: &[RuleCoverage] = &[
    // -- Psi selection members: the abstract-operations evidence matrix
    //    carries one axis-named file per rule through the public
    //    `run_psi_pipeline`/`publish_optimization_run` entrances.
    RuleCoverage {
        rule: "ControlFlowCleanup",
        positive: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/control_flow_cleanup.rs",
            "positive_threads_the_single_predecessor_empty_block",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/control_flow_cleanup.rs",
            "negative_leaves_an_empty_unit_untouched",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/control_flow_cleanup.rs",
            "boundary_declines_a_real_join_with_distinct_returns",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/control_flow_cleanup.rs",
            "disabled_sibling_selection_leaves_the_unit_untouched",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/control_flow_cleanup.rs",
            "measured_budget_admits_exact_usage_and_refuses_one_less",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/control_flow_cleanup.rs",
            "repeated_runs_are_deterministic",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/control_flow_cleanup.rs",
            "published_unit_is_a_legal_second_input_fixed_point",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/control_flow_cleanup.rs",
            "forged_run_axes_fail_publication_replay",
        ),
    },
    RuleCoverage {
        rule: "SparseConditionalConstantPropagation",
        positive: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/sparse_conditional_constant_propagation.rs",
            "positive_folds_the_certified_exact_add_constants",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/sparse_conditional_constant_propagation.rs",
            "negative_leaves_an_empty_unit_untouched",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/sparse_conditional_constant_propagation.rs",
            "boundary_declines_an_add_over_unknown_parameters",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/sparse_conditional_constant_propagation.rs",
            "disabled_sibling_selection_leaves_the_unit_untouched",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/sparse_conditional_constant_propagation.rs",
            "measured_budget_admits_exact_usage_and_refuses_one_less",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/sparse_conditional_constant_propagation.rs",
            "repeated_runs_are_deterministic",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/sparse_conditional_constant_propagation.rs",
            "published_unit_is_a_legal_second_input_fixed_point",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/sparse_conditional_constant_propagation.rs",
            "forged_run_axes_fail_publication_replay",
        ),
    },
    RuleCoverage {
        rule: "CopyPropagation",
        positive: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/copy_propagation.rs",
            "positive_eliminates_the_redundant_merge_parameter",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/copy_propagation.rs",
            "negative_leaves_an_empty_unit_untouched",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/copy_propagation.rs",
            "boundary_declines_a_parameter_with_a_distinct_false_arm_source",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/copy_propagation.rs",
            "disabled_sibling_selection_leaves_the_unit_untouched",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/copy_propagation.rs",
            "measured_budget_admits_exact_usage_and_refuses_one_less",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/copy_propagation.rs",
            "repeated_runs_are_deterministic",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/copy_propagation.rs",
            "published_unit_is_a_legal_second_input_fixed_point",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/copy_propagation.rs",
            "forged_run_axes_fail_publication_replay",
        ),
    },
    RuleCoverage {
        rule: "GlobalValueNumbering",
        positive: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/global_value_numbering.rs",
            "positive_unifies_the_commuted_proof_certified_pair",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/global_value_numbering.rs",
            "negative_leaves_an_empty_unit_untouched",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/global_value_numbering.rs",
            "boundary_declines_a_lone_expression_with_no_duplicate",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/global_value_numbering.rs",
            "disabled_sibling_selection_leaves_the_unit_untouched",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/global_value_numbering.rs",
            "measured_budget_admits_exact_usage_and_refuses_one_less",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/global_value_numbering.rs",
            "repeated_runs_are_deterministic",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/global_value_numbering.rs",
            "published_unit_is_a_legal_second_input_fixed_point",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/global_value_numbering.rs",
            "forged_run_axes_fail_publication_replay",
        ),
    },
    RuleCoverage {
        rule: "DeadPureScalarElimination",
        positive: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/dead_pure_scalar_elimination.rs",
            "positive_eliminates_unused_scalar_literals",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/dead_pure_scalar_elimination.rs",
            "negative_leaves_an_empty_unit_untouched",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/dead_pure_scalar_elimination.rs",
            "boundary_declines_scalar_work_whose_results_all_live",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/dead_pure_scalar_elimination.rs",
            "disabled_sibling_selection_leaves_the_unit_untouched",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/dead_pure_scalar_elimination.rs",
            "measured_budget_admits_exact_usage_and_refuses_one_less",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/dead_pure_scalar_elimination.rs",
            "repeated_runs_are_deterministic",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/dead_pure_scalar_elimination.rs",
            "published_unit_is_a_legal_second_input_fixed_point",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/dead_pure_scalar_elimination.rs",
            "forged_run_axes_fail_publication_replay",
        ),
    },
    RuleCoverage {
        rule: "ProofCheckElision",
        positive: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/proof_check_elision.rs",
            "positive_elides_the_certified_self_divide_obligation",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/proof_check_elision.rs",
            "negative_leaves_an_empty_unit_untouched",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/proof_check_elision.rs",
            "boundary_declines_a_certified_obligation_no_rule_covers",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/proof_check_elision.rs",
            "disabled_sibling_selection_leaves_the_unit_untouched",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/proof_check_elision.rs",
            "measured_budget_admits_exact_usage_and_refuses_one_less",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/proof_check_elision.rs",
            "repeated_runs_are_deterministic",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/proof_check_elision.rs",
            "published_unit_is_a_legal_second_input_fixed_point",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/proof_check_elision.rs",
            "forged_run_axes_fail_publication_replay",
        ),
    },
    // -- SelectedLowering selection members: literal-fold test files beside
    //    the rule, driven through `run_selected_lowering_optimizations`.
    RuleCoverage {
        rule: "SelectedIncomingU12ExactAddImmediate",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "add_immediate_fold_commutes_the_literal_operand_on_both_targets",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "add_fold_rejects_a_literal_claiming_the_wrong_operand_position",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "add_fold_admits_the_u12_boundary_immediate_and_rejects_beyond_it",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "add_fold_is_disabled_without_the_add_bit",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "add_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "add_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "add_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "add_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingU12ExactSubtractImmediate",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "subtract_immediate_fold_replaces_the_flag_clobbering_consumer_on_both_targets",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "subtract_fold_rejects_unadmitted_candidate_shapes",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "subtract_fold_admits_the_u12_boundary_immediate_and_rejects_beyond_it",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "subtract_fold_is_disabled_without_the_subtract_bit",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "subtract_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "subtract_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "subtract_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "subtract_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingU12CompareImmediate",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "compare_immediate_fold_rewrites_the_flag_defining_consumer_on_both_linux_targets",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "compare_fold_rejects_unadmitted_candidate_shapes",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "compare_fold_admits_the_u12_boundary_immediate_and_rejects_beyond_it",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "compare_fold_is_disabled_without_the_compare_bit",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "compare_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "compare_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "compare_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/compare_subtract_add_folds.rs",
            "compare_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingLiteralExtensionElimination",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/extension_and_copy_folds.rs",
            "extension_elimination_folds_every_unary_consumer_to_a_materialization_on_both_targets",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/extension_and_copy_folds.rs",
            "extension_fold_rejects_unadmitted_candidate_shapes",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/extension_and_copy_folds.rs",
            "extension_fold_rejects_result_types_that_cannot_admit_the_folded_constant",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/extension_and_copy_folds.rs",
            "extension_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/extension_and_copy_folds.rs",
            "extension_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/extension_and_copy_folds.rs",
            "extension_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/extension_and_copy_folds.rs",
            "extension_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/extension_and_copy_folds.rs",
            "extension_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingU12Load8IndexedOffset",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/extension_and_copy_folds.rs",
            "load8_indexed_fold_rewrites_the_index_operand_on_both_linux_targets",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/load_and_byte_view_folds.rs",
            "load8_indexed_fold_rejects_unadmitted_candidate_shapes",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/load_and_byte_view_folds.rs",
            "load8_indexed_fold_admits_the_widest_encodable_byte_offset",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/load_and_byte_view_folds.rs",
            "load8_indexed_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/load_and_byte_view_folds.rs",
            "load8_indexed_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/load_and_byte_view_folds.rs",
            "load8_indexed_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/load_and_byte_view_folds.rs",
            "load8_indexed_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/load_and_byte_view_folds.rs",
            "load8_indexed_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingLiteralCopyMaterialization",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/extension_and_copy_folds.rs",
            "copy_materialization_folds_the_unary_copy_to_a_materialization_on_both_targets",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/extension_and_copy_folds.rs",
            "copy_materialization_fold_rejects_unadmitted_candidate_shapes",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/extension_and_copy_folds.rs",
            "copy_materialization_fold_rejects_result_types_that_cannot_admit_the_literal",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/extension_and_copy_folds.rs",
            "copy_materialization_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/extension_and_copy_folds.rs",
            "copy_materialization_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/extension_and_copy_folds.rs",
            "copy_materialization_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/extension_and_copy_folds.rs",
            "copy_materialization_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/extension_and_copy_folds.rs",
            "copy_materialization_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingU12ByteViewAddressOffset",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/load_and_byte_view_folds.rs",
            "byte_view_address_fold_rewrites_the_offset_operand_on_both_linux_targets",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/load_and_byte_view_folds.rs",
            "byte_view_address_fold_rejects_unadmitted_candidate_shapes",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/load_and_byte_view_folds.rs",
            "byte_view_address_fold_admits_the_widest_encodable_byte_offset",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/load_and_byte_view_folds.rs",
            "byte_view_address_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/load_and_byte_view_folds.rs",
            "byte_view_address_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/load_and_byte_view_folds.rs",
            "byte_view_address_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/load_and_byte_view_folds.rs",
            "byte_view_address_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/load_and_byte_view_folds.rs",
            "byte_view_address_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingExactDivideIdentityCopy",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/load_and_byte_view_folds.rs",
            "exact_divide_identity_fold_rewrites_the_divide_to_a_copy_on_both_linux_targets",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/load_and_byte_view_folds.rs",
            "divide_fold_rejects_an_auxiliary_operand_without_zero_custody",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/load_and_byte_view_folds.rs",
            "divide_fold_rejects_a_non_unit_divisor",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "divide_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "divide_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "divide_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "divide_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "divide_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingWrappingRemainderOneZeroMaterialization",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "wrapping_remainder_one_fold_rewrites_the_remainder_to_a_zero_materialization_on_both_linux_targets",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_fold_rejects_a_dropped_def_without_dead_custody",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_fold_rejects_a_non_unit_divisor",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingBitwiseAndZeroMaterialization",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_and_zero_folds.rs",
            "and_zero_fold_rewrites_the_consumer_to_a_zero_materialization_on_both_linux_targets",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_and_zero_folds.rs",
            "and_zero_fold_rejects_malformed_operand_arrangements",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_and_zero_folds.rs",
            "and_zero_fold_rejects_a_nonzero_literal",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_and_zero_folds.rs",
            "and_zero_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_and_zero_folds.rs",
            "and_zero_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_and_zero_folds.rs",
            "and_zero_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_and_zero_folds.rs",
            "and_zero_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_and_zero_folds.rs",
            "and_zero_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingBitwiseXorZeroIdentityCopy",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_xor_zero_copies.rs",
            "xor_zero_fold_rewrites_the_consumer_to_a_surviving_operand_copy_on_both_linux_targets",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_xor_zero_copies.rs",
            "xor_zero_fold_rejects_malformed_operand_arrangements",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_xor_zero_copies.rs",
            "xor_zero_fold_rejects_a_nonzero_literal",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_xor_zero_copies.rs",
            "xor_zero_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_xor_zero_copies.rs",
            "xor_zero_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_xor_zero_copies.rs",
            "xor_zero_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_xor_zero_copies.rs",
            "xor_zero_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_xor_zero_copies.rs",
            "xor_zero_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingWrappingAddZeroIdentityCopy",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/wrapping_add_zero_copies.rs",
            "wrapping_add_zero_fold_rewrites_the_consumer_to_a_surviving_operand_copy_on_both_linux_targets",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/wrapping_add_zero_copies.rs",
            "wrapping_add_zero_fold_rejects_malformed_operand_arrangements",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/wrapping_add_zero_copies.rs",
            "wrapping_add_zero_fold_rejects_a_nonzero_literal",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/wrapping_add_zero_copies.rs",
            "wrapping_add_zero_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/wrapping_add_zero_copies.rs",
            "wrapping_add_zero_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/wrapping_add_zero_copies.rs",
            "wrapping_add_zero_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/wrapping_add_zero_copies.rs",
            "wrapping_add_zero_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/wrapping_add_zero_copies.rs",
            "wrapping_add_zero_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingBitwiseAndOnesIdentityCopy",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_and_ones_copies.rs",
            "and_ones_fold_rewrites_the_consumer_to_a_surviving_operand_copy_on_both_linux_targets",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_and_ones_copies.rs",
            "and_ones_fold_rejects_malformed_operand_arrangements",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_and_ones_copies.rs",
            "and_ones_fold_rejects_a_literal_that_is_not_all_ones",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_and_ones_copies.rs",
            "and_ones_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_and_ones_copies.rs",
            "and_ones_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_and_ones_copies.rs",
            "and_ones_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_and_ones_copies.rs",
            "and_ones_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/bitwise_and_ones_copies.rs",
            "and_ones_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingWrappingRemainderZeroDividendZeroMaterialization",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "wrapping_remainder_zero_dividend_fold_rewrites_the_remainder_to_a_zero_materialization_on_both_linux_targets",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_zero_dividend_fold_rejects_a_missing_obligation",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_zero_dividend_fold_rejects_a_non_zero_dividend",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_zero_dividend_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_zero_dividend_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_zero_dividend_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_zero_dividend_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_zero_dividend_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingExactDivideZeroDividendZeroMaterialization",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "exact_divide_zero_dividend_fold_rewrites_the_divide_to_a_zero_materialization_on_both_linux_targets",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "divide_zero_dividend_fold_rejects_a_missing_obligation",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "divide_zero_dividend_fold_rejects_a_non_zero_dividend",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "divide_zero_dividend_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "divide_zero_dividend_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "divide_zero_dividend_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "divide_zero_dividend_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "divide_zero_dividend_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingSaturatingAddZeroIdentityCopy",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_add_zero_copies.rs",
            "saturating_add_zero_fold_rewrites_the_consumer_to_a_surviving_operand_copy",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_add_zero_copies.rs",
            "saturating_add_zero_fold_rejects_malformed_operand_arrangements",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_add_zero_copies.rs",
            "saturating_add_zero_fold_rejects_a_nonzero_literal",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_add_zero_copies.rs",
            "saturating_add_zero_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_add_zero_copies.rs",
            "saturating_add_zero_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_add_zero_copies.rs",
            "saturating_add_zero_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_add_zero_copies.rs",
            "saturating_add_zero_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_add_zero_copies.rs",
            "saturating_add_zero_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingSaturatingSubtractZeroIdentityCopy",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_zero_copies.rs",
            "saturating_subtract_zero_fold_rewrites_the_consumer_to_a_surviving_operand_copy",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_zero_copies.rs",
            "saturating_subtract_zero_fold_rejects_malformed_operand_arrangements",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_zero_copies.rs",
            "saturating_subtract_zero_fold_rejects_the_left_literal_form",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_zero_copies.rs",
            "saturating_subtract_zero_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_zero_copies.rs",
            "saturating_subtract_zero_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_zero_copies.rs",
            "saturating_subtract_zero_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_zero_copies.rs",
            "saturating_subtract_zero_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_zero_copies.rs",
            "saturating_subtract_zero_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingSaturatingDivideOneIdentityCopy",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_divide_one_copies.rs",
            "saturating_divide_one_fold_rewrites_every_unsigned_carrier_consumer",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_divide_one_copies.rs",
            "saturating_divide_one_fold_rejects_a_literal_claiming_the_wrong_operand_position",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_divide_one_copies.rs",
            "saturating_divide_one_fold_rejects_a_non_one_divisor",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_divide_one_copies.rs",
            "saturating_divide_one_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_divide_one_copies.rs",
            "saturating_divide_one_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_divide_one_copies.rs",
            "saturating_divide_one_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_divide_one_copies.rs",
            "saturating_divide_one_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_divide_one_copies.rs",
            "saturating_divide_one_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingSaturatingDivideZeroDividendZeroMaterialization",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_divide_zero_dividend_materializations.rs",
            "saturating_divide_zero_dividend_fold_rewrites_every_unsigned_carrier_consumer",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_divide_zero_dividend_materializations.rs",
            "saturating_divide_zero_dividend_fold_rejects_a_missing_obligation",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_divide_zero_dividend_materializations.rs",
            "saturating_divide_zero_dividend_fold_rejects_a_non_zero_dividend",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_divide_zero_dividend_materializations.rs",
            "saturating_divide_zero_dividend_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_divide_zero_dividend_materializations.rs",
            "saturating_divide_zero_dividend_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_divide_zero_dividend_materializations.rs",
            "saturating_divide_zero_dividend_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_divide_zero_dividend_materializations.rs",
            "saturating_divide_zero_dividend_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_divide_zero_dividend_materializations.rs",
            "saturating_divide_zero_dividend_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingSaturatingSubtractZeroMinuendZeroMaterialization",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_zero_minuend_materializations.rs",
            "saturating_subtract_zero_minuend_fold_rewrites_every_unsigned_carrier_consumer",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_zero_minuend_materializations.rs",
            "saturating_subtract_zero_minuend_fold_rejects_malformed_operand_arrangements",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_zero_minuend_materializations.rs",
            "saturating_subtract_zero_minuend_fold_rejects_every_signed_carrier",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_zero_minuend_materializations.rs",
            "saturating_subtract_zero_minuend_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_zero_minuend_materializations.rs",
            "saturating_subtract_zero_minuend_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_zero_minuend_materializations.rs",
            "saturating_subtract_zero_minuend_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_zero_minuend_materializations.rs",
            "saturating_subtract_zero_minuend_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_zero_minuend_materializations.rs",
            "saturating_subtract_zero_minuend_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingSaturatingAddUpperBoundMaterialization",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_add_upper_bound_materializations.rs",
            "saturating_add_upper_bound_fold_rewrites_every_unsigned_carrier_consumer",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_add_upper_bound_materializations.rs",
            "saturating_add_upper_bound_fold_rejects_malformed_operand_arrangements",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_add_upper_bound_materializations.rs",
            "saturating_add_upper_bound_fold_rejects_a_sub_maximum_literal",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_add_upper_bound_materializations.rs",
            "saturating_add_upper_bound_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_add_upper_bound_materializations.rs",
            "saturating_add_upper_bound_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_add_upper_bound_materializations.rs",
            "saturating_add_upper_bound_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_add_upper_bound_materializations.rs",
            "saturating_add_upper_bound_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_add_upper_bound_materializations.rs",
            "saturating_add_upper_bound_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingWrappingRemainderMinusOneZeroMaterialization",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "wrapping_remainder_minus_one_fold_rewrites_the_remainder_to_a_zero_materialization_on_both_linux_targets",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_minus_one_fold_rejects_a_dropped_def_without_dead_custody",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_minus_one_fold_rejects_a_non_minus_one_divisor",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_minus_one_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_minus_one_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_minus_one_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_minus_one_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/divide_and_remainder_folds.rs",
            "remainder_minus_one_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    RuleCoverage {
        rule: "SelectedIncomingSaturatingSubtractUpperBoundSubtrahendZeroMaterialization",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_upper_bound_subtrahend_materializations.rs",
            "saturating_subtract_upper_bound_fold_rewrites_every_unsigned_carrier_consumer",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_upper_bound_subtrahend_materializations.rs",
            "saturating_subtract_upper_bound_fold_rejects_malformed_operand_arrangements",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_upper_bound_subtrahend_materializations.rs",
            "saturating_subtract_upper_bound_fold_rejects_a_sub_maximum_subtrahend",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_upper_bound_subtrahend_materializations.rs",
            "saturating_subtract_upper_bound_fold_rejects_consumers_the_selection_does_not_enable",
        ),
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_upper_bound_subtrahend_materializations.rs",
            "saturating_subtract_upper_bound_fold_reports_and_enforces_its_measured_work",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_upper_bound_subtrahend_materializations.rs",
            "saturating_subtract_upper_bound_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_upper_bound_subtrahend_materializations.rs",
            "saturating_subtract_upper_bound_fold_is_deterministic_and_a_fixed_point_on_its_output",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/tests/saturating_subtract_upper_bound_subtrahend_materializations.rs",
            "saturating_subtract_upper_bound_fold_replay_rejects_every_decision_field_substitution",
        ),
    },
    // -- CheckedTrees selection member: the rule's own module tests carry
    //    the matrix; the phase has no measured work budget.
    RuleCoverage {
        rule: "CheckedTreeProductPruning",
        positive: covered(
            "omega-rust/psi/pipeline/typed-trees-to-checked-trees/src/product_pruning/mod.rs",
            "unreachable_checked_body_machine_is_pruned",
        ),
        negative: covered(
            "omega-rust/psi/pipeline/typed-trees-to-checked-trees/src/product_pruning/mod.rs",
            "unknown_root_is_rejected_before_pruning",
        ),
        boundary: covered(
            "omega-rust/psi/pipeline/typed-trees-to-checked-trees/src/product_pruning/mod.rs",
            "boundary_machines_are_interface_surface_not_pruning_candidates",
        ),
        disabled: covered(
            "omega-rust/omega/compiler/compiler/tests/optimizer_opt_in/product_pruning.rs",
            "absent_checked_tree_pruning_selection_is_the_identity_boundary",
        ),
        budget: NO_STEP_BUDGET,
        determinism: covered(
            "omega-rust/psi/pipeline/typed-trees-to-checked-trees/src/product_pruning/mod.rs",
            "pruning_is_deterministic_for_the_same_plan",
        ),
        fixed_point: covered(
            "omega-rust/psi/pipeline/typed-trees-to-checked-trees/src/product_pruning/mod.rs",
            "the_pruned_product_is_a_fixed_point_for_the_same_plan",
        ),
        corruption: covered(
            "omega-rust/psi/pipeline/typed-trees-to-checked-trees/src/product_pruning/mod.rs",
            "independent_product_validation_rejects_roster_corruption",
        ),
    },
    // -- AllocationRecovery selection members.
    RuleCoverage {
        rule: "SharedEntryFixedViewCopyAfterCompareBeforeBranchV1",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/fixed_view_copy/compute/tests.rs",
            "shared_entry_policy_inserts_one_copy_after_compare_and_rewrites_both_returns",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/fixed_view_copy/compute/tests.rs",
            "shared_entry_policy_rejects_noncanonical_compare_copy_branch_shape",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/fixed_view_copy/compute/tests.rs",
            "shared_entry_copy_is_deterministic_bounded_and_terminal",
        ),
        disabled: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/tests.rs",
            "unselected_allocation_recovery_phase_declines",
        ),
        budget: covered(
            "tests/native-differential/tests/pipeline_ownership/stages/allocation/register_allocation/fixed_view_copy_operational/budget.rs",
            "shared_entry_fixed_view_copy_pins_exact_work_and_every_budget_domain_boundary",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/fixed_view_copy/compute/tests.rs",
            "shared_entry_copy_is_deterministic_bounded_and_terminal",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/fixed_view_copy/validate/shared_entry/tests.rs",
            "independent_shared_copy_replay_is_terminal_on_the_transformed_function",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/fixed_view_copy/validate/shared_entry/tests.rs",
            "independent_shared_copy_replay_rejects_invalid_source_and_boundary_premises",
        ),
    },
    RuleCoverage {
        rule: "ActiveResidentImmediateU64MultiUseRematerializationV1",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/pressure_rematerialization/tests/multiple_use.rs",
            "active_resident_is_split_once_before_a_multiple_use_suffix_and_reanalyzes",
        ),
        negative: covered(
            "tests/native-differential/tests/pipeline_ownership/stages/machine/active_resident/operational/execution.rs",
            "active_resident_rule_declines_when_ordinary_allocation_has_no_pressure",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/pressure_rematerialization/tests/sole_use.rs",
            "active_resident_is_split_before_sole_future_use_and_reanalyzes",
        ),
        disabled: covered(
            "tests/native-differential/tests/pipeline_ownership/stages/machine/active_resident/operational/execution.rs",
            "active_resident_rule_is_disabled_without_its_exact_selection",
        ),
        budget: covered(
            "tests/native-differential/tests/pipeline_ownership/stages/machine/active_resident/operational/budget.rs",
            "active_resident_rule_pins_exact_work_and_every_representable_first_over_boundary",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/pressure_rematerialization/tests/multiple_use.rs",
            "multiple_use_rematerialization_is_deterministic_and_terminal",
        ),
        fixed_point: covered(
            "tests/native-differential/tests/pipeline_ownership/stages/machine/active_resident/operational/execution.rs",
            "active_resident_rule_reconstructs_deterministically_and_reaches_a_rule_core_fixed_point",
        ),
        corruption: covered(
            "tests/native-differential/tests/pipeline_ownership/stages/machine/active_resident/operational/corruption.rs",
            "active_resident_rule_rejects_action_and_enclosing_custody_corruption",
        ),
    },
    // -- FunctionRelativeLayout selection member.
    RuleCoverage {
        rule: "X86RelaxConditionalBranchesToRel8V1",
        positive: covered(
            "omega-rust/omega/pipeline/resolved-layout-to-resolved-layout/src/x86_branch_relaxation/compute/tests.rs",
            "eligible_near_branch_shrinks_and_both_reflow_implementations_agree",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/resolved-layout-to-resolved-layout/src/x86_branch_relaxation/compute/tests.rs",
            "out_of_range_near_branch_is_a_verified_no_change_attempt",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/resolved-layout-to-resolved-layout/src/x86_branch_relaxation/compute/tests.rs",
            "backward_branch_at_the_i8_floor_relaxes_and_both_reflow_implementations_agree",
        ),
        disabled: covered(
            "tests/native-differential/tests/pipeline_ownership/stages/layout/x86_branch_relaxation/phase.rs",
            "layout_phase_replays_exact_selection_current_and_evidence",
        ),
        budget: covered(
            "tests/native-differential/tests/pipeline_ownership/stages/layout/x86_branch_relaxation/work_boundaries.rs",
            "exact_usage_and_every_one_below_budget_are_typed",
        ),
        determinism: covered(
            "tests/native-differential/tests/pipeline_ownership/stages/layout/x86_branch_relaxation/determinism.rs",
            "relaxation_and_phase_artifacts_are_deterministic_across_repeated_runs",
        ),
        fixed_point: covered(
            "tests/native-differential/tests/pipeline_ownership/stages/layout/x86_branch_relaxation/fixed_point.rs",
            "terminal_sweep_declines_every_branch_and_relaxed_layout_is_not_a_second_input",
        ),
        corruption: covered(
            "tests/native-differential/tests/pipeline_ownership/stages/layout/x86_branch_relaxation/action_corruption.rs",
            "authenticated_action_corruption_rejects_at_the_public_realization_boundary",
        ),
    },
    // -- Mandatory recovery rewrites: no selection member, so no disabled
    //    axis exists; admission is the stage's own validated call.
    RuleCoverage {
        rule: "runtime_rematerialization",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/runtime_rematerialization/tests.rs",
            "every_flexible_use_regenerates_the_same_immediate",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/runtime_rematerialization/tests.rs",
            "non_materialize_definitions_and_fixed_uses_gain_no_regeneration",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/runtime_rematerialization/tests.rs",
            "dominated_successor_uses_regenerate_in_their_own_block",
        ),
        disabled: NO_SELECTION_VOCABULARY,
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/runtime_rematerialization/tests.rs",
            "validation_budget_covers_the_admission_scan",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/runtime_rematerialization/tests.rs",
            "rematerialization_is_deterministic_and_per_victim_terminal",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/runtime_rematerialization/tests.rs",
            "rematerialization_is_deterministic_and_per_victim_terminal",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/runtime_rematerialization/tests.rs",
            "independent_replay_rejects_regeneration_and_lineage_corruption",
        ),
    },
    RuleCoverage {
        rule: "runtime_spill",
        positive: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/runtime_spill/tests.rs",
            "every_future_flexible_use_names_the_block_shared_reload",
        ),
        negative: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/runtime_spill/tests.rs",
            "ieee_raw_bit_spills_retain_type_and_reject_fp_register_residence",
        ),
        boundary: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/runtime_spill/tests.rs",
            "a_call_closes_the_shared_reload_for_later_flexible_uses",
        ),
        disabled: NO_SELECTION_VOCABULARY,
        budget: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/runtime_spill/tests.rs",
            "measured_validation_step_boundary_admits_and_rejects",
        ),
        determinism: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/runtime_spill/tests.rs",
            "spill_is_deterministic_and_the_published_plan_re_admits",
        ),
        fixed_point: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/runtime_spill/tests.rs",
            "spill_is_deterministic_and_the_published_plan_re_admits",
        ),
        corruption: covered(
            "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/runtime_spill/tests.rs",
            "independent_replay_rejects_storage_use_source_and_fuel_corruption",
        ),
    },
];

pub(super) fn check(audit: &mut Audit, canonical: &BTreeMap<String, CanonicalRule>) {
    let mut required: Vec<&str> = canonical.keys().map(String::as_str).collect();
    for &mandatory in MANDATORY_RECOVERY_REWRITES {
        let module = audit.repository.join(REWRITES).join(mandatory);
        let flat = audit
            .repository
            .join(REWRITES)
            .join(format!("{mandatory}.rs"));
        if !module.is_dir() && !flat.is_file() {
            audit.violations.insert(format!(
                "mandatory recovery rewrite `{mandatory}` has no module under {REWRITES}"
            ));
            continue;
        }
        required.push(mandatory);
    }
    check_table(audit, canonical, &required, COVERAGE);
}

fn check_table(
    audit: &mut Audit,
    canonical: &BTreeMap<String, CanonicalRule>,
    required: &[&str],
    table: &[RuleCoverage],
) {
    let mut published = std::collections::BTreeSet::new();
    for row in table {
        if !published.insert(row.rule) {
            audit
                .violations
                .insert(format!("coverage table repeats exact rule `{}`", row.rule));
        }
    }
    for name in required {
        if !published.contains(name) {
            audit.violations.insert(format!(
                "coverage table has no axis row for exact rule `{name}`"
            ));
        }
    }
    for name in &published {
        if !required.contains(name) {
            audit.violations.insert(format!(
                "coverage row `{name}` is not a canonical rule or mandatory recovery rewrite"
            ));
        }
    }

    for row in table {
        let member = canonical.contains_key(row.rule);
        for (axis, leg) in row.axes() {
            match leg {
                Leg::Covered { file, test } => {
                    let marker = format!("fn {test}(");
                    match fs::read_to_string(audit.repository.join(file)) {
                        Ok(contents) => {
                            if !contents.contains(&marker) {
                                audit.violations.insert(format!(
                                    "coverage row `{}` {axis} axis names `{file}` but no `fn {test}(` exists there",
                                    row.rule
                                ));
                            }
                        }
                        Err(_) => {
                            audit.violations.insert(format!(
                                "coverage row `{}` {axis} axis names missing test file `{file}`",
                                row.rule
                            ));
                        }
                    }
                }
                Leg::Absent(AbsentReason::NoSelectionVocabulary) => {
                    if member {
                        audit.violations.insert(format!(
                            "coverage row `{}` records the {axis} axis as absent for a selection-vocabulary reason, but the rule is an `Optimization` member",
                            row.rule
                        ));
                    }
                }
                Leg::Absent(AbsentReason::NoStepBudget) => {
                    if axis != "budget" {
                        audit.violations.insert(format!(
                            "coverage row `{}` records a no-step-budget absence on the {axis} axis",
                            row.rule
                        ));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use super::{Leg, RuleCoverage, check_table, covered};
    use crate::Audit;
    use crate::inventory::CanonicalRule;

    fn canonical_pair(name: &str) -> (String, CanonicalRule) {
        (
            name.to_owned(),
            CanonicalRule {
                phase: "Psi".to_owned(),
                applicability: "Target-independent".to_owned(),
            },
        )
    }

    fn fixture_repository() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "omega_per_rule_coverage_gate_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(root.join("tests")).expect("fixture root");
        fs::write(root.join("tests/fake.rs"), "fn present_test() {}\n").expect("fixture test file");
        root
    }

    #[test]
    fn the_gate_flags_every_table_failure_mode() {
        let repository = fixture_repository();
        let mut audit = Audit {
            repository: repository.clone(),
            violations: std::collections::BTreeSet::new(),
        };
        let canonical: BTreeMap<String, CanonicalRule> = [
            canonical_pair("RealRule"),
            canonical_pair("SelectionAbuseRule"),
        ]
        .into_iter()
        .collect();
        let required = ["RealRule", "SelectionAbuseRule", "fake_recovery_rewrite"];
        let ok = covered("tests/fake.rs", "present_test");
        let table = [
            // A sound row: every leg resolves or carries a legal absence.
            RuleCoverage {
                rule: "RealRule",
                positive: ok,
                negative: ok,
                boundary: ok,
                disabled: ok,
                budget: Leg::Absent(super::AbsentReason::NoStepBudget),
                determinism: ok,
                fixed_point: ok,
                corruption: ok,
            },
            // A canonical member must not claim a no-selection-vocabulary absence.
            RuleCoverage {
                rule: "SelectionAbuseRule",
                positive: ok,
                negative: ok,
                boundary: ok,
                disabled: Leg::Absent(super::AbsentReason::NoSelectionVocabulary),
                budget: ok,
                determinism: ok,
                fixed_point: ok,
                corruption: ok,
            },
            // An unknown row.
            RuleCoverage {
                rule: "UnknownRule",
                positive: ok,
                negative: ok,
                boundary: ok,
                disabled: Leg::Absent(super::AbsentReason::NoSelectionVocabulary),
                budget: ok,
                determinism: ok,
                fixed_point: ok,
                corruption: ok,
            },
            // A mandatory-style row with a missing file, a missing fn, and a
            // budget absence on a non-budget axis.
            RuleCoverage {
                rule: "fake_recovery_rewrite",
                positive: covered("tests/fake.rs", "absent_test"),
                negative: covered("tests/missing.rs", "anything"),
                boundary: Leg::Absent(super::AbsentReason::NoStepBudget),
                disabled: Leg::Absent(super::AbsentReason::NoSelectionVocabulary),
                budget: ok,
                determinism: ok,
                fixed_point: ok,
                corruption: ok,
            },
            // A duplicate row for an already-covered rule.
            RuleCoverage {
                rule: "RealRule",
                positive: ok,
                negative: ok,
                boundary: ok,
                disabled: ok,
                budget: ok,
                determinism: ok,
                fixed_point: ok,
                corruption: ok,
            },
        ];
        check_table(&mut audit, &canonical, &required, &table);

        let violations: Vec<String> = audit.violations.iter().cloned().collect();
        for expected in [
            "coverage table repeats exact rule `RealRule`",
            "coverage row `UnknownRule` is not a canonical rule or mandatory recovery rewrite",
            "coverage row `SelectionAbuseRule` records the disabled axis as absent for a selection-vocabulary reason, but the rule is an `Optimization` member",
            "coverage row `fake_recovery_rewrite` records a no-step-budget absence on the boundary axis",
            "coverage row `fake_recovery_rewrite` positive axis names `tests/fake.rs` but no `fn absent_test(` exists there",
            "coverage row `fake_recovery_rewrite` negative axis names missing test file `tests/missing.rs`",
        ] {
            assert!(
                violations.iter().any(|violation| violation == expected),
                "missing violation `{expected}`; saw {violations:?}"
            );
        }
        // `missing_rule` is required but absent from the table.
        let mut short = audit;
        short.violations.clear();
        check_table(&mut short, &canonical, &["missing_rule"], &table[..1]);
        assert!(
            short
                .violations
                .contains("coverage table has no axis row for exact rule `missing_rule`")
        );
        let _ = fs::remove_dir_all(repository);
    }
}
