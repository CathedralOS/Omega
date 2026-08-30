use super::*;

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
            binding: ProviderBinding::Import {
                locator: omega_target::normalize_foreign_locator(
                    omega_target::ForeignLocatorCandidate::ElfVersioned {
                        object: b"libomega-test.so".to_vec(),
                        symbol: symbol.to_vec(),
                        version: b"OMEGA_TEST_1".to_vec(),
                    },
                    profile,
                )
                .unwrap(),
            },
        }],
        origin_package_identity: None,
        origin_package: "test".into(),
    }
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
