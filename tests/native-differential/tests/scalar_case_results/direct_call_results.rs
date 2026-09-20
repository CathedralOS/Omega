//! One call transport still requires the exact result kind, storage and custody.

use super::{
    AdmissionProfile, NativeTarget, OptimizationSelections, compiler_baseline_request_v1,
    optimize_artifact_sections, produce_source, publish,
};
use target_operations::{TargetCallResult, TargetUnitOperation};
use target_operations_to_selected_instructions::{
    legalize_target_operations, validate_legalized_operations,
};

const SOURCE: &str = "data Pair [copy] { left: u64; right: u64; }
    machine scalar(value: u64) -> u64 { value }
    machine construct(left: u64, right: u64) -> Pair {
        Pair { left: left, right: right }
    }
    machine exercise(value: u64) -> u64 {
        let converted: u64 = scalar(value);
        let pair: Pair = construct(converted, 7);
        pair.left
    }";

#[test]
fn direct_call_result_kinds_cannot_substitute_for_each_other_at_receiving() {
    let artifact = produce_source("exercise", SOURCE);
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let selections = OptimizationSelections::new([]).unwrap();
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
                    native,
                ),
            )
            .unwrap();
        let target = compiled.target_operations();
        let source = compiled.optimized().plan();
        let unit = compiled.optimized();
        let legal = legalize_target_operations(target, source, unit).unwrap();
        validate_legalized_operations(target, source, unit, legal.plan().clone()).unwrap();
        let results = target
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match operation {
                TargetUnitOperation::Call { result, .. } => Some(result.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(results.len(), 2, "one scalar and one record call");
        assert!(
            results
                .iter()
                .any(|result| matches!(result, TargetCallResult::Scalar(_)))
        );
        assert!(
            results
                .iter()
                .any(|result| matches!(result, TargetCallResult::Structural { .. }))
        );
        for (function_index, function) in target.functions.iter().enumerate() {
            for (block_index, block) in function.graph.blocks.iter().enumerate() {
                for (operation_index, operation) in block.operations.iter().enumerate() {
                    let TargetUnitOperation::Call { result, .. } = operation else {
                        continue;
                    };
                    for replacement in results.iter().chain([&TargetCallResult::Unit]) {
                        if std::mem::discriminant(replacement) == std::mem::discriminant(result) {
                            continue;
                        }
                        let mut changed = target.clone();
                        let TargetUnitOperation::Call { result, .. } =
                            &mut changed.functions[function_index].graph.blocks[block_index]
                                .operations[operation_index]
                        else {
                            unreachable!()
                        };
                        *result = replacement.clone();
                        assert!(legalize_target_operations(&changed, source, unit).is_err());
                        assert!(
                            validate_legalized_operations(
                                &changed,
                                source,
                                unit,
                                legal.plan().clone(),
                            )
                            .is_err(),
                            "substituted result custody on {native:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn returned_reference_custody_is_reconstructed_at_receiving() {
    let artifact = produce_source(
        "exercise",
        "machine relay(value: &mut i32) -> &mut i32 { value }
        machine replace(value: &mut i32) { value = 29; }
        machine exercise(value: &mut i32) -> i32 {
            let held: &mut i32 = relay(value);
            replace(held);
            value
        }",
    );
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let selections = OptimizationSelections::new([]).unwrap();
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
                    native,
                ),
            )
            .unwrap();
        let source = compiled.optimized().plan();
        let unit = compiled.optimized();
        let target = compiled.target_operations();
        let legal = legalize_target_operations(target, source, unit).unwrap();
        validate_legalized_operations(target, source, unit, legal.plan().clone()).unwrap();
        for mutation in ["root", "duplicate", "missing", "place"] {
            let mut changed = target.clone();
            let (result, references) = changed
                .functions
                .iter_mut()
                .flat_map(|function| &mut function.graph.blocks)
                .flat_map(|block| &mut block.operations)
                .find_map(|operation| match operation {
                    TargetUnitOperation::Call {
                        result:
                            TargetCallResult::Structural {
                                result,
                                result_home,
                                reference_results,
                                ..
                            },
                        ..
                    } => {
                        assert!(
                            result_home.is_none(),
                            "a bare reference carries custody, not storage"
                        );
                        assert_eq!(reference_results.len(), 1);
                        Some((result, reference_results))
                    }
                    _ => None,
                })
                .expect("returned reference call");
            match mutation {
                "root" => references[0].root = semantic_vocabulary::PlaceId::new(999_999).unwrap(),
                "duplicate" => references.push(references[0].clone()),
                "missing" => references.clear(),
                "place" => result.place = semantic_vocabulary::PlaceId::new(999_999).unwrap(),
                _ => unreachable!(),
            }
            assert!(
                legalize_target_operations(&changed, source, unit).is_err(),
                "{mutation}"
            );
            assert!(
                validate_legalized_operations(&changed, source, unit, legal.plan().clone())
                    .is_err(),
                "receiving {mutation} on {native:?}"
            );
        }
    }
}

#[test]
fn scalar_then_record_call_preserves_the_result_through_native_publication() {
    let artifact = produce_source("exercise", SOURCE);
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (image, entry) = publish(&artifact, native);
        if native != NativeTarget::host() {
            continue;
        }
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        {
            let code = super::native_execution::Code::new(&image.output().final_text_bytes);
            for value in [0, 37, u64::MAX] {
                assert_eq!(code.call_scalar(entry, [value, 0, 0, 0]), value);
            }
        }
        #[cfg(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        ))]
        super::native_function::assert_c_text(
            &image.output().final_text_bytes,
            entry,
            r#"
            #include <stdint.h>
            extern uint64_t omega_entry(uint64_t value);
            int main(void) {
                return omega_entry(0) != 0 || omega_entry(37) != 37 ||
                       omega_entry(UINT64_MAX) != UINT64_MAX;
            }
            "#,
        );
        #[cfg(not(any(
            all(target_os = "windows", target_arch = "x86_64"),
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        )))]
        {
            let _ = (image, entry);
            eprintln!(
                "SKIP: native execution needs a supported host; cross-publication was checked"
            );
        }
    }
}
