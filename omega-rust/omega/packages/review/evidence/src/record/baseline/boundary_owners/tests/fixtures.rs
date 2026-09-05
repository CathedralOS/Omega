use super::*;

pub(super) fn package(value: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([value; 32]).unwrap()
}

pub(super) fn nominal(package: PackageKeyIdentity, path: &str) -> PackageReviewNominalIdentity {
    PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::Package(package),
        path: path.into(),
    }
}

pub(super) fn parameter() -> PackagePolicyTypeParameter {
    PackagePolicyTypeParameter {
        kind: PackagePolicyTypeParameterKind::Type,
        bounds: PackageReviewDataProperties {
            multiplicity: language_semantics::Multiplicity::Unrestricted,
            carry: None,
        },
    }
}

pub(super) fn baseline(package: PackageKeyIdentity) -> PackagePolicyBaseline {
    let target = TargetProfile::LinuxX64;
    PackagePolicyBaseline {
        package,
        target,
        public_api: PackagePolicyPublicApi {
            traits: vec![],
            conformances: vec![],
            domains: vec![],
            propositions: vec![],
            consts: vec![],
            operators: vec![],
            data: vec![],
        },
        callables: PackagePolicyCallables {
            package,
            target,
            callables: vec![],
        },
        selected_providers: PackagePolicySelectedProviders {
            package,
            target,
            plans: vec![],
            families: vec![],
        },
        terminal_permissions: PackagePolicyTerminalPermissions {
            package,
            target,
            services: vec![],
        },
        representation: PackagePolicyRepresentation {
            package,
            target: PackageReviewRepresentationTarget {
                profile: PackageReviewRepresentationTargetProfile::LinuxX64,
                architecture: PackageReviewRepresentationArchitecture::X86_64,
                object_format: PackageReviewRepresentationObjectFormat::Elf,
                pointer_size: 8,
                pointer_alignment: 8,
            },
            declarations: vec![],
            producer_availability: vec![],
            selected_availability: vec![],
            demands: vec![],
        },
        external_supplies: vec![],
        dangerous_capabilities: vec![],
        slack_uses: vec![],
        semantic_dependencies: vec![],
        boundary_applications: PackagePolicyBoundaryApplications {
            demands: vec![],
            realizations: vec![],
        },
    }
}

pub(super) fn pair() -> (PackagePolicyBaseline, PackagePolicyBaseline) {
    let mut consumer = baseline(package(1));
    let mut owner = baseline(package(2));
    let coordinate = PackageReviewOperatorCoordinate {
        identity: nominal(owner.package, "Math::identity"),
        parameter_dispatch: "type-binder:0".into(),
        result_dispatch: "type-binder:0".into(),
    };
    owner.public_api.operators.push(PackagePolicyOperatorShape {
        coordinate: coordinate.clone(),
        is_boundary: true,
        spelling: None,
        lifetime_parameter_count: 0,
        type_parameters: vec![parameter()],
        parameters: vec![],
        return_type: None,
        contracts: vec![],
        published_crash: vec![],
    });
    let identity = nominal(consumer.package, "apply");
    consumer
        .boundary_applications
        .demands
        .push(PackagePolicyBoundaryApplicationDemand {
            operator_coordinate: coordinate,
            producer_callable: identity.clone(),
            arguments: vec![
                PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
                    requirement_binder_ordinal: 0,
                    producer_binder_ordinal: 0,
                },
            ],
        });
    consumer.callables.callables.push(PackagePolicyCallable {
        role: PackagePolicyCallableRole::Public,
        identity,
        supply: PackageReviewCallableSupply::CheckedBody,
        lifetime_parameter_count: 0,
        type_parameters: vec![parameter()],
        conformance_bounds: vec![],
        parameters: vec![],
        return_type: None,
        conformances: vec![],
        operator_realizations: vec![],
        contracts: vec![],
        declared_service_reach: Some(vec![]),
        checked_service_reach: PackageReviewCheckedServiceReach::CheckedBody {
            realized: vec![],
            concrete: vec![],
        },
        unresolved_installation_reaches: vec![],
        declared_synchronous_invocations: Some(vec![]),
        realized_synchronous_invocations: vec![],
        capability_flows: vec![],
        reachable_capability_flows: vec![],
        checked_may_suspend: false,
        checked_may_block: false,
        declared_may_suspend: Some(false),
        declared_may_block: Some(false),
        declared_termination: None,
        checked_termination: PackagePolicyTermination::Terminates { premises: vec![] },
        checked_crash: PackagePolicyCrash {
            interface: PackageReviewCrashInterface::PublishedCeiling,
            published: vec![],
            structural_runtime_requirements: None,
            inferred: PackagePolicyInferredCrash::Complete { causes: vec![] },
        },
        mutation: PackagePolicyMutation {
            completeness: PackageReviewWriteFrameCompleteness::Complete,
            paths: vec![],
        },
    });
    (consumer, owner)
}
