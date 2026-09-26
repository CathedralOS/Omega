use super::{
    NativeSelectedProviderClosureDigest, NativeSelectedProviderPlan,
    validate_selected_provider_closure,
};
use effects::SelectedProviderPlanFacts;
use effects::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
};

fn selected_plan() -> ProviderPlan {
    ProviderPlan {
        name: "SelectedIndexedProvider".into(),
        provider_type: "IndexedProvider".into(),
        provider_type_package_identity: None,
        target: "test".into(),
        schema: ServiceSchema {
            trait_name: "IndexedRequirement".into(),
            trait_package_identity: None,
            methods: vec![ServiceMethod {
                name: "apply".into(),
                requirement_owner: "IndexedRequirement".into(),
                requirement_owner_package_identity: None,
                requirement_identity: "IndexedRequirement::apply".into(),
                parameter_count: 0,
                parameter_type_identities: Vec::new(),
                entry_claims: Vec::new(),
                has_result: false,
                result_type_identity: None,
                result_claims: Vec::new(),
                service_reach: vec!["IndexedRequirement".into()],
                synchronous_invocations: Vec::new(),
                may_suspend: false,
                may_block: false,
                terminates_guarantee: false,
                termination_premises: Vec::new(),
                calling_plan_report_fingerprint: None,
                calling_plan_commitment: None,
            }],
        },
        rows: vec![ProviderPlanRow {
            method: "apply".into(),
            requirement_identity: "IndexedRequirement::apply".into(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::CheckedAdapter {
                machine_identity: "IndexedProvider::apply".into(),
                machine_package_identity: None,
            },
        }],
        origin_package_identity: None,
        origin_package: "test".into(),
    }
}

fn projected(plan: &ProviderPlan) -> Vec<NativeSelectedProviderPlan> {
    vec![NativeSelectedProviderPlan::new(
        plan.report_fingerprint(),
        native_artifact::NativeSelectedProviderPlanDigest::from_digest(
            *plan.identity_digest().as_bytes(),
        ),
        vec!["IndexedRequirement::apply".into()],
    )]
}

#[test]
fn component_replay_rejects_compact_equal_provider_plan_substitution() {
    let original_plan = selected_plan();
    let mut substituted_plan = original_plan.clone();
    substituted_plan.schema.methods[0].requirement_owner = "OtherIndexedRequirement".into();
    assert_eq!(
        original_plan.report_fingerprint(),
        substituted_plan.report_fingerprint(),
        "the legacy compact plan identity omits the readable requirement owner"
    );

    let original = SelectedProviderPlanFacts::from_selected_plans(vec![original_plan.clone()])
        .expect("original selected closure");
    let substituted =
        SelectedProviderPlanFacts::from_selected_plans(vec![substituted_plan.clone()])
            .expect("compact-equal substituted closure");
    assert_eq!(
        original.compatibility_report_identity(),
        substituted.compatibility_report_identity(),
    );
    assert_ne!(original.identity_digest(), substituted.identity_digest());

    let native_plans = projected(&original_plan);
    assert_ne!(
        native_plans,
        projected(&substituted_plan),
        "the native projection now retains the strong exact plan digest",
    );
    assert_eq!(
        validate_selected_provider_closure(
            original.compatibility_report_identity(),
            NativeSelectedProviderClosureDigest::from_digest(
                *original.identity_digest().as_bytes(),
            ),
            &native_plans,
            &substituted,
        ),
        Err(
            "component candidate selected provider closure digest disagrees with its native artifact"
        )
    );
    assert_eq!(
        validate_selected_provider_closure(
            substituted.compatibility_report_identity(),
            NativeSelectedProviderClosureDigest::from_digest(
                *substituted.identity_digest().as_bytes(),
            ),
            &native_plans,
            &substituted,
        ),
        Err("component candidate selected provider facts disagree with its native artifact"),
    );
}
