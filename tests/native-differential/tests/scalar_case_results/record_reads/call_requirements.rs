//! Receiver calls retain proof-only requirements independently of storage custody.

use super::super::{membership, produce_source};

fn source(getter_requirement: &str) -> String {
    format!(
        r#"
        data Main {{ values: [u8; 8]; }}
        machine Main::at<Count: u8>(&self) -> u8
        {getter_requirement}
        {{ self.values[3] }}
        machine Main::put<Count: u8>(&mut self, value: u8)
        requires Count <= 7;
        {{ self.values[3] = Count; self.values[4] = value; }}
        machine Main::exercise(&mut self, count: u8 [0..=7], next: u8 [0..=7])
        -> i32 in Wrapping {{
            self.put<count>(20);
            let first: u8 = self.at<count>();
            self.put<next>(30);
            let second: u8 = self.at<next>();
            let stored_argument: u8 = self.values[4];
            (first as i32 in Wrapping) + (second as i32 in Wrapping)
                + (stored_argument as i32 in Wrapping)
        }}
    "#
    )
}

fn executes(getter_requirement: &str) {
    let artifact = produce_source("Main::exercise", &source(getter_requirement));
    membership::execute(
        &artifact,
        r#"
        #include <stdint.h>
        #include <string.h>
        struct state { uint8_t values[8]; };
        extern int32_t omega_entry(uint8_t count, uint8_t next, struct state *);
        int main(void) {
            const uint8_t counts[] = { 0, 3, 7 };
            const uint8_t next[] = { 0, 5, 7 };
            for (unsigned ordinal = 0; ordinal < 3; ++ordinal) {
                struct state actual, expected;
                memset(&actual, 0xa5, sizeof actual);
                memcpy(&expected, &actual, sizeof expected);
                expected.values[3] = next[ordinal];
                expected.values[4] = 30;
                if (omega_entry(counts[ordinal], next[ordinal], &actual)
                    != counts[ordinal] + next[ordinal] + 30) return 1;
                if (memcmp(&actual, &expected, sizeof actual)) return 2;
            }
            return 0;
        }
    "#,
    );
}

#[test]
fn receiver_scalar_call_without_requirement_is_the_native_control() {
    executes("");
}

#[test]
fn receiver_scalar_call_preserves_runtime_value_requirement_natively() {
    executes("requires Count <= 7;");
}

#[test]
fn receiver_scalar_call_preserves_multiple_requirements_natively() {
    executes("requires Count <= 7; Count <= 8;");
}

#[test]
#[should_panic(expected = "check scalar-case source")]
fn receiver_scalar_call_rejects_unproved_requirement() {
    let _ = produce_source("Main::exercise", &source("requires Count <= 6;"));
}

#[derive(Clone, Copy, Debug)]
enum RequirementMutation {
    Remove,
    Reorder,
    Duplicate,
    Substitute,
}

impl RequirementMutation {
    fn apply(self, obligations: &mut Vec<semantic_vocabulary::ObligationId>) {
        assert_eq!(obligations.len(), 2);
        assert_ne!(obligations[0], obligations[1]);
        match self {
            Self::Remove => {
                obligations.pop();
            }
            Self::Reorder => obligations.swap(0, 1),
            Self::Duplicate => obligations[1] = obligations[0],
            Self::Substitute => {
                obligations[0] = semantic_vocabulary::ObligationId::new(999_999).unwrap();
            }
        }
    }
}

#[test]
fn receiver_scalar_call_replay_preserves_exact_ordered_requirements() {
    use super::super::{
        AdmissionProfile, NativeTarget, OptimizationSelections, compiler_baseline_request_v1,
        optimize_artifact_sections, publish,
    };
    use legalized_operations::LegalizedScalarInstructionKind;
    use target_operations::TargetUnitOperation;
    use target_operations_to_selected_instructions::{
        legalize_target_operations, select_instructions, selection_constraints,
        validate_legalized_operations, validate_selected_instructions,
    };

    let artifact = produce_source(
        "Main::exercise",
        &source("requires Count <= 7; Count <= 8;"),
    );
    let selections = OptimizationSelections::new([]).unwrap();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        publish(&artifact, target);
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
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = selection_constraints(&legalized, &environment);
        let selected = select_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        for mutation in [
            RequirementMutation::Remove,
            RequirementMutation::Reorder,
            RequirementMutation::Duplicate,
            RequirementMutation::Substitute,
        ] {
            let mut proposed = compiled.target_operations().clone();
            let obligations = proposed
                .functions
                .iter_mut()
                .flat_map(|function| &mut function.graph.blocks)
                .flat_map(|block| &mut block.operations)
                .find_map(|operation| match operation {
                    TargetUnitOperation::Call {
                        requirement_obligations,
                        ..
                    } if requirement_obligations.len() == 2 => Some(requirement_obligations),
                    _ => None,
                })
                .expect("source-produced receiver call requirements");
            mutation.apply(obligations);
            assert!(
                legalize_target_operations(
                    &proposed,
                    compiled.optimized().plan(),
                    compiled.optimized()
                )
                .is_err(),
                "target requirements {mutation:?} on {target:?}"
            );

            let mut proposed = legalized.plan().clone();
            let obligations = proposed
                .scalar_functions
                .iter_mut()
                .flat_map(|function| &mut function.blocks)
                .flat_map(|block| &mut block.instructions)
                .find_map(|instruction| match &mut instruction.kind {
                    LegalizedScalarInstructionKind::Call(call)
                        if call.requirement_obligations.len() == 2 =>
                    {
                        Some(&mut call.requirement_obligations)
                    }
                    _ => None,
                })
                .expect("legalized receiver call requirements");
            mutation.apply(obligations);
            assert!(
                validate_legalized_operations(
                    compiled.target_operations(),
                    compiled.optimized().plan(),
                    compiled.optimized(),
                    proposed
                )
                .is_err(),
                "legalized requirements {mutation:?} on {target:?}"
            );

            // Alter either selected copy separately, then both together. Agreement
            // between forged output copies cannot replace replay from legalized input.
            for (change_contract, change_provenance) in [(true, false), (false, true), (true, true)]
            {
                let mut proposed = selected.plan().clone();
                let function = proposed
                    .functions
                    .iter_mut()
                    .find(|function| {
                        function
                            .calls
                            .iter()
                            .any(|record| record.call.requirement_obligations.len() == 2)
                    })
                    .unwrap();
                let record = function
                    .calls
                    .iter_mut()
                    .find(|record| record.call.requirement_obligations.len() == 2)
                    .unwrap();
                let operation = record.operation;
                if change_contract {
                    mutation.apply(&mut record.call.requirement_obligations);
                }
                if change_provenance {
                    let instruction = function
                        .blocks
                        .iter_mut()
                        .flat_map(|block| &mut block.instructions)
                        .find(|instruction| {
                            matches!(
                                instruction.kind,
                                selected_instructions::SelectedInstructionKind::CallScalar { .. }
                            ) && instruction.provenance.operations.contains(&operation)
                        })
                        .unwrap();
                    mutation.apply(&mut instruction.provenance.obligations);
                }
                assert!(
                    validate_selected_instructions(
                        &legalized,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                        proposed
                    )
                    .is_err(),
                    "selected requirements {mutation:?}, contract={change_contract}, provenance={change_provenance} on {target:?}"
                );
            }
        }
    }
}
