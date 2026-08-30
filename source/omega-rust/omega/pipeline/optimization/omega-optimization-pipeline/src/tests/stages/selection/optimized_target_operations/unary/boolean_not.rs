use super::*;

#[test]
fn optimized_target_lowering_retains_exact_boolean_not_parameter_custody() {
    for (target_profile, register, stack) in parameter_location_cases() {
        for (parameter_count, expected_location) in [(1, register), (9, stack)] {
            let (semantic, proof) = boolean_not_parameter_return_artifact(parameter_count);
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
            )
            .unwrap();
            let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
            let receipt = target.translation_validation();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanNotParameter(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("optimized Boolean-not parameter must retain its family row")
            };
            assert_eq!(row.not_operation(), OperationId::new(30_005).unwrap());
            assert_eq!(
                row.operand_value(),
                ValueId::new(30_100 + parameter_count as u64 - 1).unwrap()
            );
            assert_eq!(row.parameter_index(), parameter_count - 1);
            assert_eq!(row.location(), expected_location);
            assert!(matches!(
                target.target_operations().functions[0].operation,
                TargetOperation::ReturnBooleanNotParameter { location, .. }
                    if location == expected_location
            ));
        }
    }
}
