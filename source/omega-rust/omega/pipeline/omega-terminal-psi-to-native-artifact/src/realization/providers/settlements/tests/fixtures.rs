use super::*;

fn evaluated_import(
    locator: omega_target::NormalizedForeignLocator,
) -> omega_effects::provider_plan::EvaluatedForeignImport {
    let usage = omega_effects::provider_plan::EvaluatedBindingUsage::from_evaluator(
        7, 1, 10, 1_000, 0, 0, 4, 12, 3, 0,
    )
    .expect("valid fixture usage");
    let receipt = omega_effects::provider_plan::EvaluatedBindingReceipt::from_evaluation(
        None,
        "fixture::producer".to_owned(),
        omega_effects::provider_plan::EvaluatedBindingProducerClosureDigest::from_bytes([11; 32])
            .unwrap(),
        1,
        usage,
        omega_effects::provider_plan::EvaluatedBindingEvaluationDigest::from_bytes([12; 32])
            .unwrap(),
        1,
        omega_effects::provider_plan::EvaluatedBindingMaterializationDigest::from_bytes([13; 32])
            .unwrap(),
        locator.identity_digest(),
    )
    .expect("valid fixture receipt");
    omega_effects::provider_plan::EvaluatedForeignImport::from_retained_evidence(locator, receipt)
        .expect("receipt matches fixture locator")
}

pub(super) fn import_plan(symbol: &[u8], profile: omega_target::TargetProfile) -> ProviderPlan {
    let requirement = "omega::test::Foreign::leaf()";
    ProviderPlan {
        name: "omega::test::foreign-provider".into(),
        provider_type: String::new(),
        provider_type_package_identity: None,
        target: match profile {
            omega_target::TargetProfile::LinuxX64 => "linux_x86_64",
            omega_target::TargetProfile::LinuxArm64 => "linux_arm64",
            _ => unreachable!(),
        }
        .into(),
        schema: ServiceSchema {
            trait_name: "omega::test::Foreign".into(),
            trait_package_identity: None,
            methods: vec![ServiceMethod {
                name: "leaf".into(),
                requirement_owner: "omega::test::Foreign".into(),
                requirement_owner_package_identity: None,
                requirement_identity: requirement.into(),
                parameter_count: 0,
                parameter_type_identities: Vec::new(),
                entry_claims: Vec::new(),
                has_result: false,
                result_type_identity: None,
                result_claims: Vec::new(),
                service_reach: vec!["omega::test::Foreign".into()],
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
            method: "leaf".into(),
            requirement_identity: requirement.into(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::Import {
                evaluated: evaluated_import(
                    omega_target::normalize_foreign_locator(
                        omega_target::ForeignLocatorCandidate::ElfVersioned {
                            object: b"libomega-test.so".to_vec(),
                            symbol: symbol.to_vec(),
                            version: b"OMEGA_TEST_1".to_vec(),
                        },
                        profile,
                    )
                    .unwrap(),
                ),
            },
        }],
        origin_package_identity: None,
        origin_package: "test".into(),
    }
}

pub(super) fn terminal_policy(
    plan: &ProviderPlan,
    boundary_entry_plan: &omega_calling_conventions::BoundaryEntryPlan,
) -> crate::realization::TerminalAuthorityPolicy {
    let [row] = plan.rows.as_slice() else {
        panic!("fixture plan must contain one import row")
    };
    let ProviderBinding::Import { evaluated } = &row.binding else {
        panic!("fixture plan must contain one normalized import")
    };
    crate::realization::terminal_authority_policy_with_rows(vec![
        crate::realization::TerminalAuthorityPolicyRow::new(
            crate::realization::normalized_foreign_terminal_mechanism(
                evaluated.locator(),
                boundary_entry_plan,
            )
            .expect("fixture boundary plan forms an exact foreign mechanism"),
            omega_effects::TerminalAuthorityDisposition::from_classes([]),
        ),
    ])
    .expect("fixture receiving policy is exact")
}

#[derive(Debug)]
pub(super) struct TestProviderExecution {
    pub(super) requirement: String,
    pub(super) provider_plan_report_identity: u64,
}

impl omega_installation_evidence::ProviderExecutionEvidence for TestProviderExecution {
    fn requirement_identity(&self) -> &str {
        &self.requirement
    }

    fn provider_plan_report_identity(&self) -> u64 {
        self.provider_plan_report_identity
    }

    fn provider_execution_report_identity(&self) -> u64 {
        802
    }

    fn provider_execution_report_fingerprint(&self) -> u64 {
        803
    }

    fn normalized_root_report_identity(&self) -> u64 {
        804
    }

    fn boundary_contract_report_fingerprint(&self) -> u64 {
        805
    }
}
