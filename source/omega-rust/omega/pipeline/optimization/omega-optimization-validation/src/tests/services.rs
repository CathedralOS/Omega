//! Service hierarchy and root-reach tests.

use super::*;

#[test]
fn replays_service_catalog_hierarchy_ceilings_and_concrete_effects() {
    let baseline = service_effect_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("complete service closure and published PortWrite should validate");
    let root = id(701, ServiceId::new);
    let middle = id(702, ServiceId::new);
    let leaf = id(703, ServiceId::new);
    let unknown = id(799, ServiceId::new);

    let mut duplicate_id = baseline.clone();
    duplicate_id.services = duplicate_id
        .services
        .iter()
        .cloned()
        .chain(std::iter::once(duplicate_id.services[0].clone()))
        .collect::<Vec<_>>()
        .into();
    refresh_identity(&mut duplicate_id);
    assert_eq!(
        validate_psi_optimization_unit(&duplicate_id),
        Err(OptimizationUnitValidationError::DuplicateService(root))
    );

    let mut empty_identity = baseline.clone();
    let mut services = empty_identity.services.to_vec();
    services[0].identity.clear();
    empty_identity.services = services.into();
    refresh_identity(&mut empty_identity);
    assert_eq!(
        validate_psi_optimization_unit(&empty_identity),
        Err(OptimizationUnitValidationError::InvalidServiceIdentity(
            root
        ))
    );

    let mut duplicate_identity = baseline.clone();
    let mut services = duplicate_identity.services.to_vec();
    services[1].identity = services[0].identity.clone();
    duplicate_identity.services = services.into();
    refresh_identity(&mut duplicate_identity);
    assert_eq!(
        validate_psi_optimization_unit(&duplicate_identity),
        Err(OptimizationUnitValidationError::InvalidServiceIdentity(
            middle
        ))
    );

    for (parents, expected) in [
        (
            vec![leaf],
            OptimizationUnitValidationError::InvalidServiceParent {
                service: leaf,
                parent: leaf,
            },
        ),
        (
            vec![unknown],
            OptimizationUnitValidationError::InvalidServiceParent {
                service: leaf,
                parent: unknown,
            },
        ),
        (
            vec![root, root],
            OptimizationUnitValidationError::InvalidServiceParent {
                service: leaf,
                parent: root,
            },
        ),
        (
            vec![middle, root],
            OptimizationUnitValidationError::NonCanonicalServiceParents(leaf),
        ),
    ] {
        let mut candidate = baseline.clone();
        let mut services = candidate.services.to_vec();
        services[2].parents = parents;
        candidate.services = services.into();
        refresh_identity(&mut candidate);
        assert_eq!(validate_psi_optimization_unit(&candidate), Err(expected));
    }

    let mut cycle = baseline.clone();
    let mut services = cycle.services.to_vec();
    services[0].parents = vec![leaf];
    cycle.services = services.into();
    refresh_identity(&mut cycle);
    assert_eq!(
        validate_psi_optimization_unit(&cycle),
        Err(OptimizationUnitValidationError::RecursiveServiceHierarchy(
            root
        ))
    );

    let mut incomplete = baseline.clone();
    let mut services = incomplete.services.to_vec();
    services[2].parents = vec![middle];
    incomplete.services = services.into();
    refresh_identity(&mut incomplete);
    assert_eq!(
        validate_psi_optimization_unit(&incomplete),
        Err(
            OptimizationUnitValidationError::IncompleteServiceParentClosure {
                service: leaf,
                ancestor: root,
            }
        )
    );

    for ceiling in [
        vec![unknown],
        vec![root, root],
        vec![leaf, middle, root],
        vec![leaf],
    ] {
        let mut candidate = baseline.clone();
        candidate.functions[0].published_service_ceiling = ceiling;
        refresh_identity(&mut candidate);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(
                OptimizationUnitValidationError::InvalidFunctionServiceCeiling(
                    candidate.functions[0].machine
                )
            )
        );
    }

    let mut unknown_effect = baseline.clone();
    let AbstractOperation::PortWrite { service, .. } =
        &mut unknown_effect.functions[0].blocks[0].nodes[1].operation
    else {
        panic!("service fixture contains PortWrite")
    };
    *service = unknown;
    refresh_node_derivatives(&mut unknown_effect, 0, 0, 1);
    assert!(matches!(
        validate_psi_optimization_unit(&unknown_effect),
        Err(OptimizationUnitValidationError::OperationServiceContractMismatch { node: 1, .. })
    ));

    let mut outside_ceiling = baseline;
    outside_ceiling.functions[0].published_service_ceiling = vec![root, middle];
    refresh_identity(&mut outside_ceiling);
    assert!(matches!(
        validate_psi_optimization_unit(&outside_ceiling),
        Err(OptimizationUnitValidationError::OperationServiceContractMismatch { node: 1, .. })
    ));

    let mut invalid_boundary = scalar_boundary_call_unit();
    install_service_catalog(&mut invalid_boundary);
    invalid_boundary.boundary_machines[0].published_service_ceiling = vec![leaf];
    refresh_identity(&mut invalid_boundary);
    assert_eq!(
        validate_psi_optimization_unit(&invalid_boundary),
        Err(
            OptimizationUnitValidationError::InvalidBoundaryServiceCeiling(
                invalid_boundary.boundary_machines[0].id
            )
        )
    );
}

#[test]
fn replays_every_call_reach_lane_and_provider_service_refinement() {
    let root = id(701, ServiceId::new);
    let middle = id(702, ServiceId::new);

    let mut scalar = scalar_call_unit();
    install_service_catalog(&mut scalar);
    scalar.functions[0].published_service_ceiling = vec![root, middle];
    refresh_identity(&mut scalar);
    assert!(matches!(
        validate_psi_optimization_unit(&scalar),
        Err(OptimizationUnitValidationError::OperationServiceContractMismatch { .. })
    ));

    let mut structural_unit = structural_call_unit();
    install_service_catalog(&mut structural_unit);
    structural_unit.functions[0].published_service_ceiling = vec![root, middle];
    refresh_identity(&mut structural_unit);
    assert!(matches!(
        validate_psi_optimization_unit(&structural_unit),
        Err(OptimizationUnitValidationError::OperationServiceContractMismatch { .. })
    ));

    let mut structural_result = structural_result_call_unit();
    install_service_catalog(&mut structural_result);
    structural_result.functions[0].published_service_ceiling = vec![root, middle];
    refresh_identity(&mut structural_result);
    assert!(matches!(
        validate_psi_optimization_unit(&structural_result),
        Err(OptimizationUnitValidationError::OperationServiceContractMismatch { .. })
    ));

    let functions = scalar
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<BTreeMap<_, _>>();
    let services = scalar
        .services
        .iter()
        .map(|service| (service.id, service))
        .collect::<BTreeMap<_, _>>();
    let caller = &scalar.functions[0];
    let callee = scalar.functions[1].machine;
    let dummy_result = AbstractResult {
        value: id(706, ValueId::new),
        scalar_type: ScalarType::Boolean,
    };
    let dummy_structural_result = psi_terminal::StructuralOperationResult {
        place: id(707, PlaceId::new),
        structural_type: id(708, StructuralTypeId::new),
        multiplicity: psi_terminal::StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    let calls = [
        AbstractOperation::Call {
            psi_operation: id(709, OperationId::new),
            result: dummy_result.value,
            scalar_type: dummy_result.scalar_type,
            callee,
            arguments: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
        AbstractOperation::CallUnit {
            psi_operation: id(710, OperationId::new),
            callee,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
        AbstractOperation::CallStructuralScalar {
            psi_operation: id(711, OperationId::new),
            result: dummy_result,
            callee,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
        AbstractOperation::CallStructural {
            psi_operation: id(712, OperationId::new),
            result: dummy_structural_result,
            callee,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            returned_claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            selected_evidence: Vec::new(),
        },
    ];
    for call in &calls {
        assert!(!operation_service_contract_matches(
            caller,
            call,
            &functions,
            &BTreeMap::new(),
            &services,
        ));
    }

    let mut boundary = scalar_boundary_call_unit();
    install_service_catalog(&mut boundary);
    boundary.functions[0].published_service_ceiling = vec![root, middle];
    refresh_identity(&mut boundary);
    assert!(matches!(
        validate_psi_optimization_unit(&boundary),
        Err(OptimizationUnitValidationError::OperationServiceContractMismatch { .. })
    ));

    let provider = provider_service_unit();
    validate_psi_optimization_unit(&provider)
        .expect("provider realized reach exactly refines its boundary");
    let mut mismatched = provider.clone();
    mismatched.provider_candidates[0]
        .refinement
        .realized_service_ceiling
        .pop();
    refresh_identity(&mut mismatched);
    assert!(matches!(
        validate_psi_optimization_unit(&mismatched),
        Err(OptimizationUnitValidationError::InvalidProviderServiceRefinement { .. })
    ));

    let mut outside = provider;
    outside.boundary_machines[0].published_service_ceiling = vec![root, middle];
    refresh_identity(&mut outside);
    assert!(matches!(
        validate_psi_optimization_unit(&outside),
        Err(OptimizationUnitValidationError::InvalidProviderServiceRefinement { .. })
    ));
}

#[test]
fn replays_exact_root_service_reach_shape_and_installation_dependencies() {
    let baseline = installation_root_service_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("one exact installation-bound root dependency validates");
    assert!(baseline.root_service_reach.concrete.is_empty());
    assert_eq!(
        baseline.root_service_reach.installation_dependencies.len(),
        1
    );
    let root = id(701, ServiceId::new);
    let middle = id(702, ServiceId::new);
    let leaf = id(703, ServiceId::new);
    let unknown = id(799, ServiceId::new);

    for concrete in [
        vec![unknown],
        vec![root, root],
        vec![leaf, middle, root],
        vec![leaf],
    ] {
        let mut invalid_concrete = baseline.clone();
        invalid_concrete.root_service_reach.concrete = concrete;
        refresh_identity(&mut invalid_concrete);
        assert_eq!(
            validate_psi_optimization_unit(&invalid_concrete),
            Err(OptimizationUnitValidationError::InvalidRootConcreteServiceReach)
        );
    }

    let mut mismatched_concrete = baseline.clone();
    mismatched_concrete.root_service_reach.concrete = vec![root, middle, leaf];
    refresh_identity(&mut mismatched_concrete);
    assert!(matches!(
        validate_psi_optimization_unit(&mismatched_concrete),
        Err(OptimizationUnitValidationError::RootConcreteServiceReachMismatch { .. })
    ));

    for upper_bound in [
        vec![unknown],
        vec![root, root],
        vec![leaf, middle, root],
        vec![leaf],
    ] {
        let mut invalid = baseline.clone();
        invalid.root_service_reach.installation_dependencies[0].upper_bound = upper_bound;
        refresh_identity(&mut invalid);
        assert_eq!(
            validate_psi_optimization_unit(&invalid),
            Err(OptimizationUnitValidationError::InvalidRootInstallationReachDependency(0))
        );
    }

    let mut empty_identity = baseline.clone();
    empty_identity.root_service_reach.installation_dependencies[0]
        .requirement_identity
        .clear();
    refresh_identity(&mut empty_identity);
    assert_eq!(
        validate_psi_optimization_unit(&empty_identity),
        Err(OptimizationUnitValidationError::InvalidRootInstallationReachDependency(0))
    );

    let mut duplicate = baseline.clone();
    duplicate
        .root_service_reach
        .installation_dependencies
        .push(duplicate.root_service_reach.installation_dependencies[0].clone());
    refresh_identity(&mut duplicate);
    assert_eq!(
        validate_psi_optimization_unit(&duplicate),
        Err(OptimizationUnitValidationError::InvalidRootInstallationReachDependency(1))
    );

    let mut boundary_mismatch = baseline.clone();
    boundary_mismatch
        .root_service_reach
        .installation_dependencies[0]
        .upper_bound = vec![root, middle];
    refresh_identity(&mut boundary_mismatch);
    assert_eq!(
        validate_psi_optimization_unit(&boundary_mismatch),
        Err(
            OptimizationUnitValidationError::RootInstallationReachBoundaryMismatch(
                boundary_mismatch.boundary_machines[0].id
            )
        )
    );

    let mut missing = baseline.clone();
    missing.root_service_reach.installation_dependencies.clear();
    refresh_identity(&mut missing);
    assert!(matches!(
        validate_psi_optimization_unit(&missing),
        Err(OptimizationUnitValidationError::RootConcreteServiceReachMismatch { .. })
    ));

    let mut unused = baseline.clone();
    unused.root_service_reach.installation_dependencies.push(
        psi_terminal::InstallationReachDependency {
            requirement_identity: "zz-validation::unused-boundary".into(),
            upper_bound: vec![root, middle, leaf],
        },
    );
    refresh_identity(&mut unused);
    assert_eq!(
        validate_psi_optimization_unit(&unused),
        Err(OptimizationUnitValidationError::RootInstallationReachDependenciesMismatch)
    );

    let mut noncanonical = multiple_installation_root_service_unit();
    noncanonical
        .root_service_reach
        .installation_dependencies
        .reverse();
    refresh_identity(&mut noncanonical);
    assert_eq!(
        validate_psi_optimization_unit(&noncanonical),
        Err(OptimizationUnitValidationError::NonCanonicalRootInstallationReachDependencies)
    );

    let repeated = multiple_installation_root_service_unit();
    let boundary_call_count = repeated.functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter(|node| matches!(node.operation, AbstractOperation::BoundaryCall { .. }))
        .count();
    assert!(boundary_call_count > repeated.root_service_reach.installation_dependencies.len());
    validate_psi_optimization_unit(&repeated)
        .expect("repeated calls consume one canonical dependency row");

    let mut overlap = baseline;
    let block = overlap.functions[0].blocks[0].id;
    let insertion = overlap.functions[0].blocks[0].nodes.len() - 1;
    let mut write = overlap.functions[0].blocks[0].nodes[0].clone();
    write.operation = AbstractOperation::PortWrite {
        psi_operation: id(729, OperationId::new),
        service: leaf,
        port: 0x3f8,
        value: 0x41,
    };
    overlap.functions[0].blocks[0]
        .nodes
        .insert(insertion, write);
    for (index, node) in overlap.functions[0].blocks[0].nodes.iter_mut().enumerate() {
        node.effect.input = index as u64;
        node.effect.output = index as u64 + 1;
        node.provenance = expected_provenance(&node.operation);
        node.fuel = node
            .provenance
            .iter()
            .copied()
            .map(|site| omega_optimization_unit::FuelSettlement { site, units: 1 })
            .collect();
        node.definitions = expected_definitions(&node.operation, block, index as u32);
        node.uses = expected_uses(&node.operation, block, index as u32);
        node.successors = expected_edges(&node.operation);
        node.ownership = expected_ownership(&node.operation);
    }
    overlap.functions[0].facts = reconstruct_fact_index(&overlap.functions[0]);
    refresh_root_service_reach(&mut overlap)
        .expect("concrete reach remains distinct from installation bounds");
    refresh_identity(&mut overlap);
    validate_psi_optimization_unit(&overlap)
        .expect("concrete and installation-bound reach may overlap");
    assert_eq!(overlap.root_service_reach.concrete, [root, middle, leaf]);
    assert_eq!(
        overlap.root_service_reach.installation_dependencies.len(),
        1
    );
}

#[test]
fn root_service_reach_traverses_every_internal_call_lane_and_ignores_detached_effects() {
    let service = id(703, ServiceId::new);
    let mut baseline = scalar_call_unit();
    install_service_catalog(&mut baseline);
    let callee = baseline.functions[1].machine;
    let mut write = baseline.functions[1].blocks[0].nodes[0].clone();
    write.operation = AbstractOperation::PortWrite {
        psi_operation: id(720, OperationId::new),
        service,
        port: 0x3f8,
        value: 0x41,
    };
    baseline.functions[1].blocks[0].nodes.insert(0, write);
    let calls = [
        AbstractOperation::Call {
            psi_operation: id(721, OperationId::new),
            result: id(722, ValueId::new),
            scalar_type: ScalarType::Boolean,
            callee,
            arguments: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
        AbstractOperation::CallUnit {
            psi_operation: id(723, OperationId::new),
            callee,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
        AbstractOperation::CallStructuralScalar {
            psi_operation: id(724, OperationId::new),
            result: AbstractResult {
                value: id(725, ValueId::new),
                scalar_type: ScalarType::Boolean,
            },
            callee,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
        AbstractOperation::CallStructural {
            psi_operation: id(726, OperationId::new),
            result: psi_terminal::StructuralOperationResult {
                place: id(727, PlaceId::new),
                structural_type: id(728, StructuralTypeId::new),
                multiplicity: psi_terminal::StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            callee,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            returned_claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            selected_evidence: Vec::new(),
        },
    ];
    for call in calls {
        let mut candidate = baseline.clone();
        let call_node = candidate.functions[0].blocks[0]
            .nodes
            .iter_mut()
            .find(|node| matches!(node.operation, AbstractOperation::Call { .. }))
            .expect("scalar fixture contains one internal call");
        call_node.operation = call;
        refresh_root_service_reach(&mut candidate)
            .expect("every internal call lane reaches the concrete effect");
        assert_eq!(
            candidate.root_service_reach.concrete,
            vec![id(701, ServiceId::new), id(702, ServiceId::new), service]
        );
    }

    let mut detached = baseline;
    detached.functions[0].blocks[0].nodes.clear();
    refresh_root_service_reach(&mut detached)
        .expect("detached function effects do not belong to root reach");
    assert!(detached.root_service_reach.concrete.is_empty());
}
