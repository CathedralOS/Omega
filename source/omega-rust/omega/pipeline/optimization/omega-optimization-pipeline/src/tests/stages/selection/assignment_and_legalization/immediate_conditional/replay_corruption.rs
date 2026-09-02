//! Identity-bound legalization replay rejection for target, recipe, provenance, and fuel corruption.

use crate::tests::*;
use omega_legalized_operations::LegalizedCondition;

#[test]
fn legalization_identity_and_replay_reject_target_recipe_provenance_and_fuel_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_conditional(target);
        let original = staged.legalized().plan();
        let identity = legalized_operation_plan_identity(original);
        assert_eq!(identity, staged.legalized().receipt().identity());
        assert_eq!(
            staged.legalized().receipt().validator(),
            legalization_validator_identity()
        );
        assert_eq!(
            staged.selected().receipt().legalization_validator(),
            legalization_validator_identity()
        );
        assert_eq!(
            staged.custody().legalization_validator(),
            legalization_validator_identity()
        );
        assert_eq!(
            identity,
            staged_conditional(target).legalized().receipt().identity()
        );
        assert_eq!(
            original.functions[0].recipe,
            LegalizationRecipe::ReturnU64ImmediateConditionalV1
        );

        let validate = |plan| {
            validate_legalized_operations(
                staged.optimized_target().target_operations(),
                staged.optimized_target().optimized().plan(),
                staged.optimized_target().optimized().unit(),
                plan,
            )
        };

        let mut corrupted = original.clone();
        corrupted.target = if target.architecture == omega_target::Architecture::X86_64 {
            NativeTarget::linux_arm64()
        } else {
            NativeTarget::linux_x64()
        };
        assert_ne!(legalized_operation_plan_identity(&corrupted), identity);
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut corrupted = original.clone();
        corrupted.functions[0].recipe = LegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1;
        assert_ne!(legalized_operation_plan_identity(&corrupted), identity);
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut corrupted = original.clone();
        corrupted.functions[0]
            .provenance
            .operations
            .push(OperationId::new(9_601).unwrap());
        assert_ne!(legalized_operation_plan_identity(&corrupted), identity);
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut corrupted = original.clone();
        corrupted.functions[0].branch_true_fuel[0].units += 1;
        assert_ne!(legalized_operation_plan_identity(&corrupted), identity);
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut corrupted = original.clone();
        let entry_block = corrupted.functions[0].entry_block;
        let LegalizedCondition::DirectParameter {
            definition_site, ..
        } = &mut corrupted.functions[0].condition
        else {
            panic!("fixture must retain a direct condition")
        };
        *definition_site = ValueDefinitionSite::Node {
            block: entry_block,
            node: 0,
        };
        assert_ne!(legalized_operation_plan_identity(&corrupted), identity);
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut corrupted = original.clone();
        corrupted.functions[0].branch_true_fuel[0].site =
            omega_optimization_unit::PsiProvenance::Edge(corrupted.functions[0].branch_false_edge);
        assert_ne!(legalized_operation_plan_identity(&corrupted), identity);
        assert_eq!(
            validate(corrupted),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut corrupted = original.clone();
        corrupted.functions[0].provenance.edges.swap(0, 1);
        assert_ne!(legalized_operation_plan_identity(&corrupted), identity);
        assert!(matches!(
            validate(corrupted),
            Err(LegalizationError::SourceCustodyMismatch)
                | Err(LegalizationError::NonCanonicalLegalizedPlan)
        ));
    }
}
