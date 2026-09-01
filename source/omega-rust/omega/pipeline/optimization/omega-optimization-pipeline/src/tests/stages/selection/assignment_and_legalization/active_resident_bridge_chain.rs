//! Public-validator coverage for the pressure-bearing legalized bridge chain.

use crate::tests::*;

#[test]
fn bridge_chain_legalization_is_independently_validated_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (semantic, proof) = conditional_active_resident_exact_add_bridge_chain_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(
                OptimizationSelections::new([
                    Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
                ])
                .unwrap(),
            ),
        )
        .unwrap();
        let target = lower_optimized_to_target_operations(optimized, target).unwrap();
        let legalized = legalize_target_operations(
            target.target_operations(),
            target.optimized().plan(),
            target.optimized().unit(),
        )
        .unwrap();
        let function = &legalized.plan().functions[0];
        assert_eq!(
            function.recipe,
            LegalizationRecipe::ReturnU64ActiveResidentExactAddBridgeChainConditionalV1,
        );
        let LegalizedLeafValue::ActiveResidentExactAddBridgeChain(chain) =
            &function.when_true.value
        else {
            panic!("bridge source must retain its distinct legalized carrier")
        };
        assert_ne!(chain.bridge.source_value, chain.middle.source_value);
        assert_ne!(chain.bridge.operation, chain.result.operation);

        let mut corrupted = legalized.plan().clone();
        let LegalizedLeafValue::ActiveResidentExactAddBridgeChain(chain) =
            &mut corrupted.functions[0].when_true.value
        else {
            unreachable!()
        };
        std::mem::swap(&mut chain.bridge.operation, &mut chain.middle.operation);
        assert!(
            validate_legalized_operations(
                target.target_operations(),
                target.optimized().plan(),
                target.optimized().unit(),
                corrupted,
            )
            .is_err()
        );
    }
}
