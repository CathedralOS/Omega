//! Optimized scalar production retains graph values and exact entry placements.
use super::*;

#[test]
fn optimized_scalar_parameters_use_only_the_common_graph() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    for boolean in [false, true] {
        for (target_profile, register, stack) in parameter_location_cases() {
            for (parameter_count, location) in [(1, register), (9, stack)] {
                let (semantic, proof) = if boolean {
                    boolean_parameter_return_artifact(parameter_count)
                } else {
                    integer_parameter_return_artifact(integer_type, parameter_count)
                };
                let optimized = optimize_artifact_sections(
                    &semantic,
                    &proof,
                    &AdmissionProfile::default(),
                    request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
                )
                .unwrap();
                let target =
                    lower_optimized_to_target_operations(optimized, target_profile).unwrap();
                assert!(matches!(
                    target.translation_validation().function_roster()[0].translation(),
                    AbstractToTargetFunctionTranslationDisposition::Uncovered
                ));
                let function = &target.target_operations().functions[0];
                let TargetOperation::ControlGraph(graph) = &function.operation else {
                    panic!("scalar body must remain block-owned");
                };
                let abi = function.scalar_abi.as_ref().unwrap();
                assert_eq!(graph.call_plan, abi.call_plan);
                assert_eq!(graph.scalar_parameters, abi.parameters);
                let placement = &graph.scalar_parameters[parameter_count - 1].placement;
                match (location, placement.locations.as_slice()) {
                    (
                        ScalarParameterLocation::Register(expected),
                        [calling_conventions::ValueLocation::Register { register, .. }],
                    ) => assert_eq!(*register, expected),
                    (
                        ScalarParameterLocation::IncomingStack { byte_offset },
                        [
                            calling_conventions::ValueLocation::Stack {
                                stack_byte_offset, ..
                            },
                        ],
                    ) => assert_eq!(*stack_byte_offset, byte_offset),
                    _ => panic!("entry placement must retain the target ABI"),
                }
            }
        }
    }
}
