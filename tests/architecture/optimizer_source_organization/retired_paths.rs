//! Retired flat paths and prohibited proxy schedules.

use std::fs;

use crate::Audit;

use super::bounds::is_test_source;

pub(crate) fn check(audit: &mut Audit) {
    let repository = &audit.repository;
    let source_lines = &audit.source_lines;
    let violations = &mut audit.violations;

    let psi_pass_root =
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/";
    for path in source_lines.keys().filter(|path| {
        path.starts_with(psi_pass_root)
            && !is_test_source(path)
            && (path.ends_with("/rule.rs") || path.ends_with("/rules.rs"))
    }) {
        violations.insert(format!(
            "Psi pass retains a generic rule leaf instead of an exact optimization name: {path}"
        ));
    }

    for obsolete in [
        "source/omega-rust/omega/representations/omega-optimization-core/src/manifest.rs",
        "source/omega-rust/omega/representations/omega-legalized-operations/src/validation/call_source.rs",
        "source/omega-rust/omega/representations/omega-optimization-unit/src/identity/operation_encoding.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/sparse_conditional_constant_propagation/candidate_validation.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/sparse_conditional_constant_propagation/boolean_evaluation/integer_comparisons.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/sparse_conditional_constant_propagation/range_comparisons.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/copy_propagation/redundant_block_parameter.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/global_value_numbering/expression_keys.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/tests/structural_catalog.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/live_ranges/validate/replay.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/live_ranges/validate/tests.rs",
        "source/omega-rust/omega/representations/omega-optimization-unit/src/ledger.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/machine/selected_lowering.rs",
        "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/scalar/conditional_control.rs",
        "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/compute.rs",
        "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/validation/blocks.rs",
        "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/source/leaves.rs",
        "source/omega-rust/omega/representations/omega-optimization-unit/src/rewrite/model.rs",
        "source/omega-rust/omega/representations/omega-optimization-unit/src/construction.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/control_flow_cleanup/block_merging.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/control_flow_cleanup/empty_block_threading.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/control_flow_cleanup/block_merging.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/control_flow_cleanup/empty_block_threading.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/control_flow_cleanup/constant_conditionals.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/control_flow_cleanup/shared_jump_fusion.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/control_flow_cleanup/unreachable_private_machines.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/control_flow_cleanup/empty_block_threading/linear.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/control_flow_cleanup/empty_block_threading/path_qualified.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/control_flow_cleanup/block_merging/adjacent.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/control_flow_cleanup/block_merging/non_adjacent.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/accounting.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/proof_check_elision/identity_rewrite.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/dead_scalar_elimination/proof_certified/mod.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/dead_scalar_elimination/family.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/dead_scalar_elimination/proposal.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/dead_scalar_elimination/shapes.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/dead_scalar_elimination/accounting.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/boolean.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/cast.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/unary.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/arithmetic.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/quotient.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/shifts.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/bitwise.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/range_comparisons.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/tests/sparse_conditional_constant_propagation.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/tests/sparse_conditional_constant_propagation/constant_evaluation.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/tests/sparse_conditional_constant_propagation/constant_evaluation/integer.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/tests/global_value_numbering/identities.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/tests/global_value_numbering/multiply_zero.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/tests/global_value_numbering/saturating_neutral.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/tests/global_value_numbering/saturating_multiply_zero.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/tests/global_value_numbering/bitwise_neutral.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/tests/global_value_numbering/bitwise_absorbing.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/tests/global_value_numbering/same_block.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/tests/global_value_numbering/dominating.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/tests/global_value_numbering/phi_translated.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/tests/global_value_numbering/compatible_policy.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/tests/global_value_numbering/contract_custody.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/tests/dead_scalar_elimination.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/validate.rs",
        "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/src/lowering/machine/operation.rs",
    ] {
        if repository.join(obsolete).exists() {
            violations.insert(format!(
                "optimizer representation restored a retired flat or forwarding-wall path: {obsolete}"
            ));
        }
    }

    for (family, relative_root) in [
        ("proof-check elision", "proof_check_elision/"),
        ("control-flow cleanup", "control_flow_cleanup/"),
        ("global value numbering", "global_value_numbering/"),
        (
            "SCCP range comparisons",
            "sparse_conditional_constant_propagation/range_comparisons/",
        ),
        (
            "SCCP constant evaluation",
            "sparse_conditional_constant_propagation/constant_evaluation/",
        ),
    ] {
        let family_root = format!("{psi_pass_root}{relative_root}");
        for path in source_lines
            .keys()
            .filter(|path| path.starts_with(&family_root) && !is_test_source(path))
        {
            match fs::read_to_string(repository.join(path)) {
                Ok(contents)
                    if contents.contains("use super::*;")
                        || contents.contains("use super::super::*;") =>
                {
                    violations.insert(format!(
                        "{family} restored an inherited parent glob dependency: {path}"
                    ));
                }
                Ok(_) => {}
                Err(error) => {
                    violations.insert(format!("cannot read {path}: {error}"));
                }
            }
        }
    }

    for obsolete in [
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/rule.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/shapes.rs",
    ] {
        if repository.join(obsolete).exists() {
            violations.insert(format!(
                "global-value-numbering identities retain a mixed catch-all: {obsolete}"
            ));
        }
    }

    let obsolete_post_allocation_manifest = "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/post_allocation_manifest.rs";
    if repository.join(obsolete_post_allocation_manifest).exists() {
        violations.insert(format!(
            "register allocation retains the mixed post-allocation manifest file: {obsolete_post_allocation_manifest}"
        ));
    }

    for obsolete in [
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/home_assignment/compute.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/home_assignment/validate.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/home_assignment/compute_tests.rs",
    ] {
        if repository.join(obsolete).exists() {
            violations.insert(format!(
                "register-home assignment retains a retired flat compute/replay/fixture leaf: {obsolete}"
            ));
        }
    }

    for obsolete in [
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/logical_spill_operations.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/logical_spill_operations/compute.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/logical_spill_operations/validate.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/logical_spill_operations/codec.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/logical_spill_operations/tests.rs",
    ] {
        if repository.join(obsolete).exists() {
            violations.insert(format!(
                "logical spill planning retains a retired flat entrance or mixed leaf: {obsolete}"
            ));
        }
    }

    for obsolete in [
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/codec.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/codec_tests.rs",
    ] {
        if repository.join(obsolete).exists() {
            violations.insert(format!(
                "fixed-view-copy retains a retired flat codec surface: {obsolete}"
            ));
        }
    }

    for obsolete in [
        "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/tests/translation_validation.rs",
        "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/tests/translation_validation_boolean.rs",
        "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/tests/translation_validation_crash.rs",
        "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/tests/translation_validation_integer_bitwise_not_parameter.rs",
        "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/tests/translation_validation_integer_less_or_equal_parameters.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/selection/optimized_target_operations/comparison.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/selection/optimized_target_operations/unary.rs",
        "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/tests/parameter_translation_fixture/bitwise.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/fixtures/target_translation/bitwise.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/selection/optimized_target_operations/bitwise.rs",
    ] {
        if repository.join(obsolete).exists() {
            violations.insert(format!(
                "translation validation retains a retired mixed test leaf: {obsolete}"
            ));
        }
    }

    let obsolete_selected_lowering_schedule = "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/machine/literal_folds/schedule.rs";
    if repository
        .join(obsolete_selected_lowering_schedule)
        .exists()
    {
        violations.insert(format!(
            "selected lowering retains a proxy schedule beside its owning rule catalog: {obsolete_selected_lowering_schedule}"
        ));
    }

    let post_allocation_dispatch = "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/machine/post_allocation_optimizations/execution/dispatch.rs";
    match fs::read_to_string(repository.join(post_allocation_dispatch)) {
        Ok(contents)
            if [
                "Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1",
                "Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1",
                "Optimization::X86SelectXorZeroI64MaterializationV1",
                "Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1",
                "Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1",
                "stage_optimized_post_allocation_machine_optimization_for_rule",
                "stage_optimized_post_allocation_machine_optimization_after_selected_lowering_for_rule",
            ]
            .into_iter()
            .any(|retired| contents.contains(retired)) =>
        {
            violations.insert(format!(
                "post-allocation execution restored a proxy name schedule beside its owning catalog: {post_allocation_dispatch}"
            ));
        }
        Ok(_) => {}
        Err(error) => {
            violations.insert(format!("cannot read {post_allocation_dispatch}: {error}"));
        }
    }

    let cbnz_root = "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/compare_zero_branch_nonzero";
    let cbnz_compute = format!("{cbnz_root}/compute.rs");
    let cbnz_validate = format!("{cbnz_root}/validate.rs");
    match fs::read_to_string(repository.join(&cbnz_compute)) {
        Ok(contents)
            if !contents.contains("match_terminal_pair")
                || !contents.contains("AARCH64_CBNZ_TERMINAL_PAIR_V1") =>
        {
            violations.insert(format!(
                "CBNZ producer no longer enters through its declarative terminal-pair contract: {cbnz_compute}"
            ));
        }
        Ok(_) => {}
        Err(error) => {
            violations.insert(format!("cannot read {cbnz_compute}: {error}"));
        }
    }
    match fs::read_to_string(repository.join(&cbnz_validate)) {
        Ok(contents)
            if contents.contains("match_terminal_pair")
                || contents.contains("peephole_matching")
                || contents.contains("compute::") =>
        {
            violations.insert(format!(
                "independent CBNZ validation imports producer matcher mechanics: {cbnz_validate}"
            ));
        }
        Ok(_) => {}
        Err(error) => {
            violations.insert(format!("cannot read {cbnz_validate}: {error}"));
        }
    }

    let machine_rule_root =
        "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/";
    for path in source_lines.keys().filter(|path| {
        path.starts_with(machine_rule_root)
            && !is_test_source(path)
            && (path.ends_with("/validate.rs") || path.contains("/validate/"))
    }) {
        match fs::read_to_string(repository.join(path)) {
            Ok(contents)
                if [
                    "crate::costs",
                    "omega_machine_optimizer::costs",
                    "target_cost_model",
                    "TargetCostModel",
                    "NonAuthoritativeMachineCost",
                    "NonAuthoritativeMachineSizeCost",
                    "NonAuthoritativeLatencyCost",
                ]
                .into_iter()
                .any(|marker| contents.contains(marker)) =>
            {
                violations.insert(format!(
                    "machine-rule semantic validator imports non-authoritative target costs: {path}"
                ));
            }
            Ok(_) => {}
            Err(error) => {
                violations.insert(format!("cannot read {path}: {error}"));
            }
        }
    }

    let obsolete_external_policy_schema = "source/omega-rust/omega/pipeline/optimization/omega-optimization-policy/src/external_schema.rs";
    if repository.join(obsolete_external_policy_schema).exists() {
        violations.insert(format!(
            "external policy retains the mixed flat schema beside its governed entrance: {obsolete_external_policy_schema}"
        ));
    }
    for obsolete in [
        "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/construction/plan.rs",
        "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/construction/scalar.rs",
    ] {
        if repository.join(obsolete).exists() {
            violations.insert(format!(
                "selected construction retains an opaque flat coordinator: {obsolete}"
            ));
        }
    }
    for path in source_lines.keys().filter(|path| {
        !is_test_source(path)
            && (path.starts_with(
                "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/",
            ) || path.starts_with(
                "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/machine/literal_folds/",
            ))
    }) {
        match fs::read_to_string(repository.join(path)) {
            Ok(contents)
                if contents.contains("SelectedIncomingU12ExactAddAndSubtractImmediateV1")
                    || contents.contains("SelectedLoweringOptimizationSchedule") =>
            {
                violations.insert(format!(
                    "selected lowering retains a hidden combined policy or proxy schedule in {path}"
                ));
            }
            Ok(_) => {}
            Err(error) => {
                violations.insert(format!("cannot read {path}: {error}"));
            }
        }
    }
}
