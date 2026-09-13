//! Ordinary full-width arithmetic retains its policy through native publication.

use super::*;

const SUBTRACT: &str = "machine subtract(left: u64, right: u64) -> u64 {
    ((left as u64 in Saturating) - (right as u64 in Saturating)) as u64
}";

const DIVIDE: &str =
    "machine divide(numerator: u64, denominator: u64 [1..=18446744073709551615]) -> u64 {
    numerator / denominator
}";

#[test]
fn exact_divide_u64_publishes_four_targets_and_runs_non_power_of_two_divisors() {
    let artifact = produce_source("divide", DIVIDE);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        publish(&artifact, target);
    }
    membership::execute(
        &artifact,
        "#include <stdint.h>\nextern uint64_t omega_entry(uint64_t, uint64_t);\nint main(void) { const uint64_t numerators[] = {0,1,2,3,7,8,UINT64_C(0x7fffffffffffffff),UINT64_C(0x8000000000000000),UINT64_MAX-1,UINT64_MAX}; const uint64_t denominators[] = {1,2,3,5,7,8,UINT64_C(0x8000000000000001),UINT64_MAX}; for (unsigned numerator=0; numerator<10; ++numerator) for (unsigned denominator=0; denominator<8; ++denominator) if (omega_entry(numerators[numerator], denominators[denominator])!=numerators[numerator]/denominators[denominator]) return 1; return 0; }",
    );
}

#[test]
fn exact_divide_replay_rejects_missing_proof_changed_policy_and_forged_fact() {
    use legalized_operations::{
        LegalizedExactIntegerOperator as Operator, LegalizedScalarInstructionKind as Instruction,
    };
    use target_operations_to_selected_instructions::{
        legalize_target_operations, validate_legalized_operations,
    };
    let artifact = produce_source("divide", DIVIDE);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let mut proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let obligation = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            terminal_psi::OperationKind::ExactIntegerDivide { obligation, .. } => Some(obligation),
            _ => None,
        })
        .unwrap();
    proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(
        terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).is_err()
    );
    let selections = OptimizationSelections::new([]).unwrap();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let optimized = optimize_artifact_sections(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &AdmissionProfile::default(),
            compiler_baseline_request_v1(&selections),
        )
        .unwrap();
        let compiled =
            abstract_operations_to_target_operations::lower_optimized_to_target_operations(
                optimized, target,
            )
            .unwrap();
        let legalized = legalize_target_operations(
            compiled.target_operations(),
            compiled.optimized().plan(),
            compiled.optimized(),
        )
        .unwrap();
        for mutation in 0..6 {
            let mut proposed = legalized.plan().clone();
            let row = proposed
                .scalar_functions
                .iter_mut()
                .flat_map(|function| &mut function.blocks)
                .flat_map(|block| &mut block.instructions)
                .find(|row| {
                    matches!(
                        row.kind,
                        Instruction::ExactBinary {
                            operator: Operator::Divide,
                            ..
                        }
                    )
                })
                .unwrap();
            let Instruction::ExactBinary {
                operator,
                left,
                right,
                obligation,
                accepted_fact,
            } = &mut row.kind
            else {
                unreachable!()
            };
            match mutation {
                0 => *operator = Operator::Subtract,
                1 => std::mem::swap(left, right),
                2 => *obligation = semantic_vocabulary::ObligationId::new(999999).unwrap(),
                3 => {
                    *accepted_fact =
                        optimization_core::AcceptedObligationFactIdentity::from_bytes([0x5a; 32])
                }
                4 => row.operation = semantic_vocabulary::OperationId::new(999999).unwrap(),
                _ => {
                    row.kind = Instruction::SaturatingSubtractU64 {
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
                "division mutation {mutation} on {target:?}"
            );
        }
    }
}
#[test]
fn exact_divide_selection_rejects_forged_proof_and_high_half_setup() {
    use selected_instructions::SelectedInstructionKind;
    use target_operations_to_selected_instructions::{
        selection_constraints, stage_optimized_instruction_selection,
        validate_selected_instructions,
    };
    let artifact = produce_source("divide", DIVIDE);
    let selections = OptimizationSelections::new([]).unwrap();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let optimized = optimize_artifact_sections(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &AdmissionProfile::default(),
            compiler_baseline_request_v1(&selections),
        )
        .unwrap();
        let compiled =
            abstract_operations_to_target_operations::lower_optimized_to_target_operations(
                optimized, target,
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
        for mutation in 0..6 {
            let mut proposed = staged.selected().plan().clone();
            let divide = proposed
                .functions
                .iter_mut()
                .flat_map(|function| &mut function.blocks)
                .flat_map(|block| &mut block.instructions)
                .find(|instruction| {
                    matches!(
                        instruction.kind,
                        SelectedInstructionKind::ExactDivideU64 { .. }
                    )
                })
                .unwrap();
            let SelectedInstructionKind::ExactDivideU64 {
                obligation,
                accepted_fact,
            } = &mut divide.kind
            else {
                unreachable!()
            };
            match mutation {
                0 => *obligation = semantic_vocabulary::ObligationId::new(999999).unwrap(),
                1 => {
                    *accepted_fact =
                        optimization_core::AcceptedObligationFactIdentity::from_bytes([0x5a; 32])
                }
                2 => divide.operands.swap(0, 1),
                3 => divide.provenance.obligations.clear(),
                4 => divide.provenance.operations.clear(),
                _ => {
                    divide.kind = SelectedInstructionKind::ExactSubtractI64 {
                        obligation: *obligation,
                        accepted_fact: *accepted_fact,
                    }
                }
            }
            assert!(
                validate(proposed).is_err(),
                "divide selected mutation {mutation} on {target:?}"
            );
        }
        if target.architecture == target::Architecture::X86_64 {
            let mut proposed = staged.selected().plan().clone();
            let setup = proposed
                .functions
                .iter_mut()
                .flat_map(|function| &mut function.blocks)
                .flat_map(|block| &mut block.instructions)
                .find(|instruction| {
                    matches!(
                        instruction.kind,
                        SelectedInstructionKind::MaterializeI64 { .. }
                    )
                })
                .unwrap();
            setup.kind = SelectedInstructionKind::MaterializeI64 {
                value: semantic_vocabulary::IntegerValue::Unsigned(1),
            };
            assert!(
                validate(proposed).is_err(),
                "division must retain an explicitly zero high half"
            );
        }
    }
}

#[test]
fn saturating_add_u64_publishes_four_targets_and_runs_end_address_edges() {
    let artifact = produce_source(
        "add",
        "machine add(left: u64, right: u64) -> u64 {
        ((left as u64 in Saturating) + (right as u64 in Saturating)) as u64
    }",
    );
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        publish(&artifact, target);
    }
    membership::execute(
        &artifact,
        "#include <stdint.h>\nextern uint64_t omega_entry(uint64_t, uint64_t);\nint main(void) { const uint64_t values[] = {0,1,2,3,7,8,UINT64_C(0x7fffffffffffffff),UINT64_C(0x8000000000000000),UINT64_MAX-1,UINT64_MAX}; for (unsigned left=0; left<10; ++left) for (unsigned right=0; right<10; ++right) { uint64_t expected=values[left]<=UINT64_MAX-values[right] ? values[left]+values[right] : UINT64_MAX; if (omega_entry(values[left], values[right])!=expected) return 1; } return 0; }",
    );
}

#[test]
fn saturating_subtract_u64_publishes_four_targets_and_runs_full_width_edges() {
    let artifact = produce_source("subtract", SUBTRACT);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        publish(&artifact, target);
    }
    membership::execute(
        &artifact,
        "#include <stdint.h>\nextern uint64_t omega_entry(uint64_t, uint64_t);\nint main(void) { const uint64_t values[] = {0,1,2,3,7,8,UINT64_C(0x7fffffffffffffff),UINT64_C(0x8000000000000000),UINT64_MAX-1,UINT64_MAX}; for (unsigned left=0; left<10; ++left) for (unsigned right=0; right<10; ++right) { uint64_t expected=values[left]>=values[right] ? values[left]-values[right] : 0; if (omega_entry(values[left], values[right])!=expected) return 1; } return 0; }",
    );
}

#[test]
fn saturating_subtract_selection_rejects_changed_operation_operands_and_custody() {
    use selected_instructions::SelectedInstructionKind;
    use target_operations_to_selected_instructions::{
        selection_constraints, stage_optimized_instruction_selection,
        validate_selected_instructions,
    };
    let artifact = produce_source("subtract", SUBTRACT);
    let selections = OptimizationSelections::new([]).unwrap();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let optimized = optimize_artifact_sections(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &AdmissionProfile::default(),
            compiler_baseline_request_v1(&selections),
        )
        .unwrap();
        let compiled =
            abstract_operations_to_target_operations::lower_optimized_to_target_operations(
                optimized, target,
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
        for mutation in 0..5 {
            let mut proposed = staged.selected().plan().clone();
            let subtract = proposed
                .functions
                .iter_mut()
                .flat_map(|function| &mut function.blocks)
                .flat_map(|block| &mut block.instructions)
                .find(|instruction| {
                    matches!(
                        instruction.kind,
                        SelectedInstructionKind::SaturatingSubtractU64
                    )
                })
                .unwrap();
            match mutation {
                0 => subtract.kind = SelectedInstructionKind::BitwiseXorI64,
                1 => subtract.operands.swap(0, 1),
                2 => subtract.operands.swap(1, 2),
                3 => subtract.provenance.operations.clear(),
                _ => subtract.provenance.fuel.clear(),
            }
            assert!(
                validate(proposed).is_err(),
                "subtract mutation {mutation} on {target:?}"
            );
        }
    }
}
