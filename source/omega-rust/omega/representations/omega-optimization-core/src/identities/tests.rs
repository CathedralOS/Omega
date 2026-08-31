use super::*;
use crate::{Optimization, OptimizationSelections};

fn bundle() -> OptimizationIdentityBundle {
    let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup])
        .expect("unique selection")
        .identity();
    let rules = [
        OptimizationRuleIdentity::from_canonical_bytes(b"cfg-cleanup/v1"),
        OptimizationRuleIdentity::from_canonical_bytes(b"branch-fold/v2"),
    ];
    OptimizationIdentityBundle::new(
        selections,
        OptimizationRuleSetIdentity::from_ordered_rules(&rules).expect("unique rules"),
        TargetCostModelIdentity::from_canonical_bytes(b"target-cost/x86-64/v1"),
        Some(OptimizationDecisionLogIdentity::from_canonical_bytes(
            b"decision-log",
        )),
        None,
        TransformationLedgerIdentity::from_canonical_bytes(b"ledger"),
    )
}

#[test]
fn identity_domains_are_distinct_for_equal_canonical_bytes() {
    assert_ne!(
        OptimizationRuleIdentity::from_canonical_bytes(b"same").bytes(),
        TargetCostModelIdentity::from_canonical_bytes(b"same").bytes()
    );
    assert_ne!(
        OptimizationDecisionLogIdentity::from_canonical_bytes(b"same").bytes(),
        TransformationLedgerIdentity::from_canonical_bytes(b"same").bytes()
    );
    let rule = OptimizationRuleIdentity::from_canonical_bytes(b"same").bytes();
    for identity in [
        OptimizationPassIdentity::from_canonical_bytes(b"same").bytes(),
        OptimizationCandidateIdentity::from_canonical_bytes(b"same").bytes(),
        ScalarConstantFactIdentity::from_canonical_bytes(b"same").bytes(),
        PostAllocationOptimizationManifestIdentity::from_canonical_bytes(b"same").bytes(),
        SelectedLoweringOptimizationCompletionIdentity::from_canonical_bytes(b"same").bytes(),
        FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(b"same")
            .bytes(),
        TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"same").bytes(),
        FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(b"same").bytes(),
        RelocationFreeObjectPlanIdentity::from_canonical_bytes(b"same").bytes(),
        RelocationFreeObjectContainerIdentity::from_canonical_bytes(b"same").bytes(),
        FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(b"same").bytes(),
        OptimizedObjectArtifactIdentity::from_canonical_bytes(b"same").bytes(),
        OptimizedObjectArtifactManifestIdentity::from_canonical_bytes(b"same").bytes(),
        OptimizedTerminalOrdinaryCallableEntryIdentity::from_canonical_bytes(b"same").bytes(),
        OptimizedOrdinaryCallableEntryManifestIdentity::from_canonical_bytes(b"same").bytes(),
        OptimizedProgramStorageSemanticWrapperObjectIdentity::from_canonical_bytes(b"same").bytes(),
        OptimizedProgramStorageSemanticWrapperObjectContainerIdentity::from_canonical_bytes(
            b"same",
        )
        .bytes(),
        OptimizedProgramStorageSemanticWrapperObjectManifestIdentity::from_canonical_bytes(b"same")
            .bytes(),
        OptimizationDecisionIdentity::from_canonical_bytes(b"same").bytes(),
        OptimizationDecisionSchemaIdentity::from_canonical_bytes(b"same").bytes(),
        OptimizationDecisionTargetIdentity::from_canonical_bytes(b"same").bytes(),
        OptimizationValidatorIdentity::from_canonical_bytes(b"same").bytes(),
        OptimizationUnitIdentity::from_canonical_bytes(b"same").bytes(),
        OptimizationRuleSetIdentity::from_canonical_bytes(b"same").bytes(),
        OptimizationWorkloadProfileIdentity::from_canonical_bytes(b"same").bytes(),
        OptimizationIdentityBundleIdentity::from_canonical_bytes(b"same").bytes(),
        OptimizedAbstractPlanProjectionIdentity::from_canonical_bytes(b"same").bytes(),
    ] {
        assert_ne!(rule, identity);
    }
}

#[test]
fn every_fixed_width_identity_round_trips() {
    macro_rules! round_trip {
        ($identity:ty) => {{
            let identity = <$identity>::from_canonical_bytes(stringify!($identity).as_bytes());
            assert_eq!(<$identity>::decode(&identity.encode()), Ok(identity));
        }};
    }
    round_trip!(OptimizationRuleIdentity);
    round_trip!(OptimizationPassIdentity);
    round_trip!(OptimizationCandidateIdentity);
    round_trip!(ScalarConstantFactIdentity);
    round_trip!(ValueRangeFactIdentity);
    round_trip!(AcceptedObligationFactIdentity);
    round_trip!(ProofQuestionIdentity);
    round_trip!(OwnershipFrontierFactIdentity);
    round_trip!(PrePhysicalOptimizationManifestIdentity);
    round_trip!(PostAllocationOptimizationManifestIdentity);
    round_trip!(SelectedLoweringOptimizationCompletionIdentity);
    round_trip!(FunctionRelativeOptimizationRealizationManifestIdentity);
    round_trip!(FunctionFragmentEmissionIdentity);
    round_trip!(FunctionFragmentEmissionManifestIdentity);
    round_trip!(TerminalRelocationFreeTextSectionIdentity);
    round_trip!(FunctionFragmentTextSectionManifestIdentity);
    round_trip!(RelocationFreeObjectPlanIdentity);
    round_trip!(RelocationFreeObjectContainerIdentity);
    round_trip!(FunctionFragmentObjectContainerManifestIdentity);
    round_trip!(OptimizedObjectArtifactIdentity);
    round_trip!(OptimizedObjectArtifactManifestIdentity);
    round_trip!(OptimizedTerminalOrdinaryCallableEntryIdentity);
    round_trip!(OptimizedOrdinaryCallableEntryManifestIdentity);
    round_trip!(OptimizedProgramStorageSemanticWrapperObjectIdentity);
    round_trip!(OptimizedProgramStorageSemanticWrapperObjectContainerIdentity);
    round_trip!(OptimizedProgramStorageSemanticWrapperObjectManifestIdentity);
    round_trip!(OptimizationDecisionIdentity);
    round_trip!(OptimizationDecisionSchemaIdentity);
    round_trip!(OptimizationDecisionTargetIdentity);
    round_trip!(OptimizationValidatorIdentity);
    round_trip!(OptimizationUnitIdentity);
    round_trip!(OptimizationRuleSetIdentity);
    round_trip!(TargetCostModelIdentity);
    round_trip!(OptimizationDecisionLogIdentity);
    round_trip!(OptimizationWorkloadProfileIdentity);
    round_trip!(TransformationLedgerIdentity);
    round_trip!(OptimizationIdentityBundleIdentity);
    round_trip!(OptimizedAbstractPlanProjectionIdentity);
}

#[test]
fn ordered_rule_set_binds_order_and_rejects_duplicates() {
    let first = OptimizationRuleIdentity::from_canonical_bytes(b"first");
    let second = OptimizationRuleIdentity::from_canonical_bytes(b"second");
    assert_ne!(
        OptimizationRuleSetIdentity::from_ordered_rules(&[first, second]).unwrap(),
        OptimizationRuleSetIdentity::from_ordered_rules(&[second, first]).unwrap()
    );
    assert_eq!(
        OptimizationRuleSetIdentity::from_ordered_rules(&[first, first]),
        Err(DuplicateOptimizationRuleIdentity(first))
    );
}

#[test]
fn bundle_round_trip_and_optional_presence_are_canonical() {
    let bundle = bundle();
    let encoded = bundle.encode();
    assert_eq!(OptimizationIdentityBundle::decode(&encoded), Ok(bundle));
    assert_eq!(bundle.identity(), bundle.identity());

    let without_decisions = OptimizationIdentityBundle::new(
        bundle.selections(),
        bundle.rule_set(),
        bundle.target_cost_model(),
        None,
        bundle.workload_profile(),
        bundle.transformation_ledger(),
    );
    assert_ne!(bundle.identity(), without_decisions.identity());
}

#[test]
fn every_bundle_component_changes_the_composite_identity() {
    let baseline = bundle();
    let changed = [
        OptimizationIdentityBundle::new(
            OptimizationSelections::new([Optimization::CopyPropagation])
                .unwrap()
                .identity(),
            baseline.rule_set(),
            baseline.target_cost_model(),
            baseline.decision_log(),
            baseline.workload_profile(),
            baseline.transformation_ledger(),
        ),
        OptimizationIdentityBundle::new(
            baseline.selections(),
            OptimizationRuleSetIdentity::from_canonical_bytes(b"rules-2"),
            baseline.target_cost_model(),
            baseline.decision_log(),
            baseline.workload_profile(),
            baseline.transformation_ledger(),
        ),
        OptimizationIdentityBundle::new(
            baseline.selections(),
            baseline.rule_set(),
            TargetCostModelIdentity::from_canonical_bytes(b"cost-2"),
            baseline.decision_log(),
            baseline.workload_profile(),
            baseline.transformation_ledger(),
        ),
        OptimizationIdentityBundle::new(
            baseline.selections(),
            baseline.rule_set(),
            baseline.target_cost_model(),
            Some(OptimizationDecisionLogIdentity::from_canonical_bytes(
                b"decisions-2",
            )),
            baseline.workload_profile(),
            baseline.transformation_ledger(),
        ),
        OptimizationIdentityBundle::new(
            baseline.selections(),
            baseline.rule_set(),
            baseline.target_cost_model(),
            baseline.decision_log(),
            Some(OptimizationWorkloadProfileIdentity::from_canonical_bytes(
                b"workload-2",
            )),
            baseline.transformation_ledger(),
        ),
        OptimizationIdentityBundle::new(
            baseline.selections(),
            baseline.rule_set(),
            baseline.target_cost_model(),
            baseline.decision_log(),
            baseline.workload_profile(),
            TransformationLedgerIdentity::from_canonical_bytes(b"ledger-2"),
        ),
    ];
    for candidate in changed {
        assert_ne!(baseline.identity(), candidate.identity());
    }
}

#[test]
fn malformed_identity_and_bundle_encodings_reject() {
    assert_eq!(
        OptimizationRuleIdentity::decode(&[0; 31]),
        Err(IdentityDecodeError::WrongLength {
            expected: 32,
            actual: 31,
        })
    );
    let mut trailing = bundle().encode();
    trailing.push(0);
    assert_eq!(
        OptimizationIdentityBundle::decode(&trailing),
        Err(IdentityBundleDecodeError::TrailingBytes)
    );
    let mut invalid_tag = bundle().encode();
    let decision_tag = 12 + IDENTITY_WIDTH * 3;
    invalid_tag[decision_tag] = 2;
    assert_eq!(
        OptimizationIdentityBundle::decode(&invalid_tag),
        Err(IdentityBundleDecodeError::InvalidOptionalTag(2))
    );
}
