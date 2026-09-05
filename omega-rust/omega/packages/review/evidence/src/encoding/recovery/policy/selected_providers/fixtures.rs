use crate::record::*;
use omega_effects::provider_plan::*;
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;
use psi_language_semantics::*;

pub(super) fn nominal(path: &str) -> PackageReviewNominalIdentity {
    PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::Package(package()),
        path: path.to_owned(),
    }
}

pub(super) fn package() -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([7; 32]).unwrap()
}

pub(super) fn empty() -> PackagePolicySelectedProviders {
    PackagePolicySelectedProviders {
        package: package(),
        target: TargetProfile::LinuxX64,
        plans: Vec::new(),
        families: Vec::new(),
    }
}

pub(super) fn carry() -> CarryPolicy {
    CarryPolicy {
        suspension: CarrySuspension::Allowed,
        cpu: CarryCpu::Any,
        host_thread: CarryHostThread::Origin,
        address: CarryAddress::Stable,
    }
}

pub(in crate::encoding::recovery::policy) fn method() -> PackagePolicyServiceMethod {
    let calling = super::super::calling_application::tests::complete_fixture();
    let signature = PackagePolicyServiceSignature {
        schema_arguments: calling.boundary_arguments.clone(),
        schema_lifetime_parameter_count: calling.boundary_lifetime_parameter_count,
        requirement_arguments: calling.requirement_arguments.clone(),
        requirement_lifetime_arguments: calling.requirement_lifetime_arguments.clone(),
        requirement_lifetime_parameter_count: calling.requirement_lifetime_parameter_count,
        static_parameters: calling.static_parameters.clone(),
        parameters: calling
            .semantic_parameters
            .iter()
            .map(|parameter| PackageReviewTraitRequirementParameter {
                name: parameter.name.clone(),
                type_identity: parameter.value_type.clone(),
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
                is_self: false,
            })
            .collect(),
        result: calling.semantic_result.clone(),
    };
    PackagePolicyServiceMethod {
        name: "call".into(),
        requirement_owner: nominal("Boundary"),
        requirement: nominal("Boundary::call"),
        signature,
        authority: PackagePolicyServiceAuthority {
            service_reach: vec![nominal("Boundary"), nominal("Storage")],
            synchronous_invocations: vec![PackageReviewSynchronousInvocation::Service(nominal(
                "Storage",
            ))],
            progress_premises: vec![PackagePolicyServiceProgressPremise {
                profile: nominal("Progress"),
                subject: ServiceProgressSubject::Parameter(1),
                subject_projections: vec![nominal("Context::progress")],
                establishment_routes: vec![
                    PackagePolicyServiceProgressRoute {
                        kind: ServiceProgressEstablishmentRouteKind::CheckedRequirement,
                        requirement_owner: nominal("Progress"),
                        requirement: nominal("Progress::checked"),
                    },
                    PackagePolicyServiceProgressRoute {
                        kind: ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                        requirement_owner: nominal("Progress"),
                        requirement: nominal("Progress::boundary"),
                    },
                ],
            }],
        },
        parameter_count: 2,
        parameter_type_identities: vec!["u64".into(), "&Context".into()],
        entry_claims: vec![ServiceEntryClaim {
            parameter_index: 0,
            carrier_identity: "u64".into(),
            domain: "Authorized".into(),
            predicate_body: DomainPredicateBody::Present,
            effective_carry: carry(),
            authority_flow: ServiceEntryAuthorityFlow::Accepts,
        }],
        has_result: false,
        result_type_identity: None,
        result_claims: Vec::new(),
        service_reach: vec!["Boundary".into(), "Storage".into()],
        synchronous_invocations: vec!["Storage".into()],
        may_suspend: true,
        may_block: false,
        terminates_guarantee: true,
        termination_premises: vec![ServiceProgressPremise {
            profile: "Progress".into(),
            subject: ServiceProgressSubject::Parameter(1),
            subject_projections: vec!["Context::progress".into()],
            establishment_routes: vec![
                ServiceProgressEstablishmentRoute {
                    kind: ServiceProgressEstablishmentRouteKind::CheckedRequirement,
                    requirement_identity: "Progress::checked".into(),
                },
                ServiceProgressEstablishmentRoute {
                    kind: ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                    requirement_identity: "Progress::boundary".into(),
                },
            ],
        }],
        calling: Some(calling),
    }
}

pub(in crate::encoding::recovery::policy) fn complete() -> PackagePolicySelectedProviders {
    let mut policy = empty();
    let method = method();
    let row = PackagePolicyProviderRow {
        method: method.name.clone(),
        requirement: method.requirement.clone(),
        realization: nominal("Provider::call"),
        requirement_lifetime_partition: vec![0, 1, 0],
        binding: PackagePolicyProviderBinding::Syscall {
            number: 19,
            evaluated: None,
        },
        compiler_intrinsic_execution: None,
        installation_reach: Some(PackageReviewSelectedInstallationReach {
            upper_bound: vec![nominal("Boundary"), nominal("Storage")],
            resolved: vec![nominal("Storage")],
        }),
    };
    let plan = PackagePolicyProviderPlan {
        plan_name: "ZBoundary".into(),
        realizing_package: Some(package()),
        schema_declaration: nominal("Boundary"),
        provider_type: "Provider".into(),
        provider_type_declaration: Some(nominal("Provider")),
        target: policy.target.target_name().to_owned(),
        methods: vec![method],
        rows: vec![row],
        grants: vec![
            PackageReviewProviderGrantSelectorKind::PlanName,
            PackageReviewProviderGrantSelectorKind::ProviderSlot,
        ],
    };
    let mut operator = plan.clone();
    operator.plan_name = "AOperator".into();
    operator.schema_declaration = nominal("operator");
    operator.methods[0].name = "operator".into();
    operator.methods[0].requirement = nominal("operator");
    operator.methods[0].requirement_owner = nominal("operator");
    operator.methods[0].calling = None;
    operator.rows[0].method = "operator".into();
    operator.rows[0].requirement = nominal("operator");
    operator.rows[0].requirement_lifetime_partition.clear();
    policy.plans = vec![operator, plan];
    policy.families.push(PackagePolicyProviderFamily {
        family_identity: nominal("operator"),
        provider_type_declaration: nominal("Provider"),
        target: policy.target,
        authority: PackageReviewProviderSelectionAuthority::BuildOverride,
        coverage: PackageReviewProviderFamilyCoverage::CompleteDeclarationFamily,
        coordinates: vec![PackagePolicyProviderFamilyCoordinate {
            requirement_identity: "operator".into(),
            operator_declaration: nominal("operator"),
            plan_index: 0,
        }],
    });
    policy
}

pub(super) fn producer() -> PackagePolicyEvaluatedBindingProducer {
    PackagePolicyEvaluatedBindingProducer {
        declaration: nominal("binding"),
        package: Some(package()),
        callable_identity: "binding".into(),
    }
}
