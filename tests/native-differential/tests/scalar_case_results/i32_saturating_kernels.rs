//! Signed 32-bit saturating arithmetic clamps to the carrier bounds through
//! native publication, and its proof-carrying divide replays independently.

use super::{
    AdmissionProfile, NativeTarget, OptimizationSelections, compiler_baseline_request_v1,
    membership, optimize_artifact_sections, produce_source, publish,
};

const ADD: &str = "machine add(left: i32, right: i32) -> i32 {
    ((left as i32 in Saturating) + (right as i32 in Saturating)) as i32
}";

const SUBTRACT: &str = "machine subtract(left: i32, right: i32) -> i32 {
    ((left as i32 in Saturating) - (right as i32 in Saturating)) as i32
}";

/// The negative divisor range includes -1, so `i32::MIN / -1` is executable.
const DIVIDE_NEGATIVE: &str =
    "machine divide(numerator: i32, denominator: i32 [-2147483648..=-1]) -> i32 {
    ((numerator as i32 in Saturating) / (denominator as i32 in Saturating)) as i32
}";

const DIVIDE_POSITIVE: &str =
    "machine divide(numerator: i32, denominator: i32 [1..=2147483647]) -> i32 {
    ((numerator as i32 in Saturating) / (denominator as i32 in Saturating)) as i32
}";

const VALUES: &str = "INT32_MIN, INT32_MIN + 1, -40, -7, -1, 0, 1, 7, 40, INT32_MAX - 1, INT32_MAX";

fn all_targets() -> [NativeTarget; 4] {
    [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ]
}

/// The C driver clamps a 64-bit reference result to the i32 range, so every
/// pair including `INT32_MAX + 1`, `INT32_MIN - 1`, and `INT32_MIN / -1`
/// checks against the arithmetic definition rather than the target.
fn driver(operator: &str, denominators: &str) -> String {
    format!(
        "#include <stdint.h>\n
        extern int32_t omega_entry(int32_t, int32_t);
        static int32_t clamp(int64_t value) {{
            if (value > INT32_MAX) return INT32_MAX;
            if (value < INT32_MIN) return INT32_MIN;
            return (int32_t)value;
        }}
        int main(void) {{
            const int32_t lefts[] = {{ {VALUES} }};
            const int32_t rights[] = {{ {denominators} }};
            for (unsigned left = 0; left < sizeof lefts / sizeof lefts[0]; ++left)
                for (unsigned right = 0; right < sizeof rights / sizeof rights[0]; ++right) {{
                    int32_t expected = clamp((int64_t)lefts[left] {operator} (int64_t)rights[right]);
                    if (omega_entry(lefts[left], rights[right]) != expected) return 1;
                }}
            return 0;
        }}"
    )
}

#[test]
fn saturating_add_i32_publishes_four_targets_and_clamps_carrier_edges() {
    let artifact = produce_source("add", ADD);
    for target in all_targets() {
        publish(&artifact, target);
    }
    membership::execute(&artifact, &driver("+", VALUES));
}

#[test]
fn saturating_subtract_i32_publishes_four_targets_and_clamps_carrier_edges() {
    let artifact = produce_source("subtract", SUBTRACT);
    for target in all_targets() {
        publish(&artifact, target);
    }
    membership::execute(&artifact, &driver("-", VALUES));
}

#[test]
fn saturating_divide_i32_publishes_four_targets_and_clamps_minimum_over_minus_one() {
    for (source, denominators) in [
        (DIVIDE_NEGATIVE, "INT32_MIN, INT32_MIN + 1, -40, -7, -2, -1"),
        (DIVIDE_POSITIVE, "1, 2, 7, 40, INT32_MAX - 1, INT32_MAX"),
    ] {
        let artifact = produce_source("divide", source);
        for target in all_targets() {
            publish(&artifact, target);
        }
        membership::execute(&artifact, &driver("/", denominators));
    }
}

#[test]
fn saturating_divide_i32_legalization_replay_rejects_forged_proof_and_other_carriers() {
    use legalized_operations::{
        LegalizedExactIntegerOperator as Operator, LegalizedScalarInstructionKind as Instruction,
    };
    use target_operations_to_selected_instructions::{
        legalize_target_operations, validate_legalized_operations,
    };
    let artifact = produce_source("divide", DIVIDE_NEGATIVE);
    let selections = OptimizationSelections::new([]).unwrap();
    for target in all_targets() {
        let optimized = optimize_artifact_sections(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &AdmissionProfile::default(),
            compiler_baseline_request_v1(&selections),
        )
        .unwrap();
        let compiled =
            abstract_operations_to_target_operations::lower_optimized_to_target_operations(
                optimized,
                abstract_operations_to_target_operations::OptimizedTargetLoweringRequest::new(
                    target,
                ),
            )
            .unwrap();
        let legalized = legalize_target_operations(
            compiled.target_operations(),
            compiled.optimized().plan(),
            compiled.optimized(),
        )
        .unwrap();
        assert!(
            legalized
                .plan()
                .scalar_functions
                .iter()
                .flat_map(|function| &function.blocks)
                .flat_map(|block| &block.instructions)
                .any(|row| matches!(row.kind, Instruction::SaturatingDivideI32 { .. })),
            "{target:?} legalizes the signed i32 saturating divide"
        );
        for mutation in 0..8 {
            let mut proposed = legalized.plan().clone();
            let row = proposed
                .scalar_functions
                .iter_mut()
                .flat_map(|function| &mut function.blocks)
                .flat_map(|block| &mut block.instructions)
                .find(|row| matches!(row.kind, Instruction::SaturatingDivideI32 { .. }))
                .unwrap();
            let Instruction::SaturatingDivideI32 {
                left,
                right,
                obligation,
                accepted_fact,
            } = &mut row.kind
            else {
                unreachable!()
            };
            match mutation {
                0 => std::mem::swap(left, right),
                1 => *obligation = semantic_vocabulary::ObligationId::new(999999).unwrap(),
                2 => {
                    *accepted_fact =
                        optimization_core::AcceptedObligationFactIdentity::from_bytes([0x5a; 32])
                }
                3 => row.operation = semantic_vocabulary::OperationId::new(999999).unwrap(),
                4 => {
                    // The same operands and proof under the Exact policy would
                    // trap on i32::MIN / -1 instead of clamping.
                    row.kind = Instruction::ExactBinary {
                        operator: Operator::Divide,
                        left: *left,
                        right: *right,
                        obligation: *obligation,
                        accepted_fact: *accepted_fact,
                    }
                }
                5 => {
                    row.kind = Instruction::WrappingRemainder {
                        left: *left,
                        right: *right,
                        obligation: *obligation,
                        accepted_fact: *accepted_fact,
                    }
                }
                6 => {
                    row.kind = Instruction::SaturatingSubtractU64 {
                        left: *left,
                        right: *right,
                    }
                }
                _ => {
                    row.kind = Instruction::SaturatingAddI32 {
                        left: *left,
                        right: *right,
                    }
                }
            }
            assert!(
                validate_legalized_operations(
                    compiled.target_operations(),
                    compiled.optimized().plan(),
                    compiled.optimized(),
                    proposed
                )
                .is_err(),
                "saturating divide mutation {mutation} on {target:?}"
            );
        }
    }
}

#[test]
fn saturating_i32_selection_rejects_forged_proof_other_carriers_and_scratch_drift() {
    use selected_instructions::SelectedInstructionKind;
    use target_operations_to_selected_instructions::{
        selection_constraints, stage_optimized_instruction_selection,
        validate_selected_instructions,
    };
    for (entry, source, divides) in [("divide", DIVIDE_NEGATIVE, true), ("add", ADD, false)] {
        let artifact = produce_source(entry, source);
        let selections = OptimizationSelections::new([]).unwrap();
        for target in all_targets() {
            let optimized = optimize_artifact_sections(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &AdmissionProfile::default(),
                compiler_baseline_request_v1(&selections),
            )
            .unwrap();
            let compiled =
                abstract_operations_to_target_operations::lower_optimized_to_target_operations(
                    optimized,
                    abstract_operations_to_target_operations::OptimizedTargetLoweringRequest::new(
                        target,
                    ),
                )
                .unwrap();
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let staged = stage_optimized_instruction_selection(compiled, environment).unwrap();
            let environment = staged.register_environment();
            let constraints = selection_constraints(staged.legalized(), environment);
            let validate = |raw| {
                validate_selected_instructions(
                    staged.legalized(),
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    raw,
                )
            };
            validate(staged.selected().plan().clone()).unwrap();
            let is_saturating = |kind: &SelectedInstructionKind| {
                if divides {
                    matches!(kind, SelectedInstructionKind::SaturatingDivideI32 { .. })
                } else {
                    matches!(kind, SelectedInstructionKind::SaturatingAddI32)
                }
            };
            for mutation in 0..7 {
                let mut proposed = staged.selected().plan().clone();
                let instruction = proposed
                    .functions
                    .iter_mut()
                    .flat_map(|function| &mut function.blocks)
                    .flat_map(|block| &mut block.instructions)
                    .find(|instruction| is_saturating(&instruction.kind))
                    .unwrap();
                assert_eq!(
                    instruction.operands.len(),
                    4,
                    "{target:?} carries a bound scratch"
                );
                match (mutation, &mut instruction.kind) {
                    (0, SelectedInstructionKind::SaturatingDivideI32 { obligation, .. }) => {
                        *obligation = semantic_vocabulary::ObligationId::new(999999).unwrap()
                    }
                    (0, _) => instruction.kind = SelectedInstructionKind::SaturatingAddU64,
                    (1, SelectedInstructionKind::SaturatingDivideI32 { accepted_fact, .. }) => {
                        *accepted_fact =
                            optimization_core::AcceptedObligationFactIdentity::from_bytes(
                                [0x5a; 32],
                            )
                    }
                    (1, _) => instruction.kind = SelectedInstructionKind::SaturatingSubtractI32,
                    (2, _) => instruction.operands.swap(0, 1),
                    (3, _) => instruction.operands.swap(2, 3),
                    (4, _) => instruction.provenance.operations.clear(),
                    (5, _) => instruction.provenance.fuel.clear(),
                    (_, SelectedInstructionKind::SaturatingDivideI32 { .. }) => {
                        instruction.kind = SelectedInstructionKind::WrappingRemainderI64 {
                            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                            accepted_fact:
                                optimization_core::AcceptedObligationFactIdentity::from_bytes(
                                    [0; 32],
                                ),
                        }
                    }
                    (_, _) => instruction.kind = SelectedInstructionKind::WrappingAddI64,
                }
                assert!(
                    validate(proposed).is_err(),
                    "{entry} selected mutation {mutation} on {target:?}"
                );
            }
        }
    }
}
