use super::*;

#[test]
fn optimized_target_lowering_retains_exact_integer_parameter_custody() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("native integer type");
    for (target_profile, register, stack) in parameter_location_cases() {
        for (parameter_count, expected_location) in [(1, register), (9, stack)] {
            let (semantic, proof) =
                integer_parameter_return_artifact(integer_type, parameter_count);
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
                AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerParameter(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("optimized parameter lowering must retain its validated family row")
            };
            assert_eq!(row.scalar_type(), integer_type);
            assert_eq!(row.parameter_index(), parameter_count - 1);
            assert_eq!(row.location(), expected_location);
            assert!(matches!(
                target.target_operations().functions[0].operation,
                TargetOperation::ReturnIntegerParameter { location, .. }
                    if location == expected_location
            ));
        }
    }
}

#[test]
fn optimized_target_lowering_retains_exact_boolean_parameter_custody() {
    for (target_profile, register, stack) in parameter_location_cases() {
        for (parameter_count, expected_location) in [(1, register), (9, stack)] {
            let (semantic, proof) = boolean_parameter_return_artifact(parameter_count);
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
                AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanParameter(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("optimized Boolean parameter lowering must retain its family row")
            };
            assert_eq!(row.parameter_index(), parameter_count - 1);
            assert_eq!(row.location(), expected_location);
            assert!(matches!(
                target.target_operations().functions[0].operation,
                TargetOperation::ReturnBooleanParameter { location, .. }
                    if location == expected_location
            ));
        }
    }
}
