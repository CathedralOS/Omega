use super::*;

fn fixed_boundary_module(overlapping: bool) -> TerminalModule {
    let mut module = boundary_call_module();
    module.services = vec![
        ServiceDeclaration {
            id: service_id(1),
            identity: "Installer".into(),
            parents: vec![service_id(2)],
        },
        ServiceDeclaration {
            id: service_id(2),
            identity: "Queryable".into(),
            parents: Vec::new(),
        },
        ServiceDeclaration {
            id: service_id(3),
            identity: "Console".into(),
            parents: Vec::new(),
        },
    ];
    let published = vec![service_id(1), service_id(2), service_id(3)];
    let fixed = if overlapping {
        published.clone()
    } else {
        vec![service_id(1), service_id(2)]
    };
    module.boundary_machines[0].identity = "Installer::step".into();
    module.boundary_machines[0].fixed_service_reach = fixed.clone();
    module.boundary_machines[0].published_service_ceiling = published.clone();
    module.machines[0].published_service_ceiling = published;
    module.root_service_reach.concrete = fixed;
    module.root_service_reach.installation_dependencies = vec![InstallationReachDependency {
        requirement_identity: "Installer::step".into(),
        upper_bound: vec![service_id(3)],
    }];
    module
}

#[test]
fn fixed_boundary_reach_preserves_disjoint_and_overlapping_installation_contributions() {
    for overlapping in [false, true] {
        let module = fixed_boundary_module(overlapping);
        validate_module(&module).expect("fixed boundary reach is independent of its bound");
        assert_eq!(
            module.root_service_reach.installation_dependencies[0].upper_bound,
            vec![service_id(3)]
        );
        let mut missing_root = module.clone();
        missing_root.root_service_reach.concrete.clear();
        assert!(matches!(
            validate_module(&missing_root),
            Err(ModuleError::RootConcreteServiceReachMismatch { .. })
        ));
        let mut missing_fixed = module.clone();
        missing_fixed.boundary_machines[0]
            .fixed_service_reach
            .clear();
        assert_eq!(
            validate_module(&missing_fixed).unwrap_err(),
            ModuleError::InstallationReachBoundaryMismatch(boundary_id(1))
        );
    }
}

#[test]
fn fixed_boundary_reach_requires_exact_bound_union_and_known_parent_closed_rows() {
    let module = fixed_boundary_module(false);
    let mut wrong_bound = module.clone();
    wrong_bound.root_service_reach.installation_dependencies[0]
        .upper_bound
        .clear();
    assert_eq!(
        validate_module(&wrong_bound).unwrap_err(),
        ModuleError::InstallationReachBoundaryMismatch(boundary_id(1))
    );
    let mut wrong_ceiling = module.clone();
    wrong_ceiling.boundary_machines[0]
        .published_service_ceiling
        .pop();
    assert_eq!(
        validate_module(&wrong_ceiling).unwrap_err(),
        ModuleError::InstallationReachBoundaryMismatch(boundary_id(1))
    );
    let mut outside_ceiling = module.clone();
    outside_ceiling.boundary_machines[0].published_service_ceiling = vec![service_id(3)];
    assert_eq!(
        validate_module(&outside_ceiling).unwrap_err(),
        ModuleError::FixedBoundaryServiceOutsidePublishedCeiling {
            boundary: boundary_id(1),
            service: service_id(1),
        }
    );
    let mut missing_parent = module.clone();
    missing_parent.boundary_machines[0].fixed_service_reach = vec![service_id(1)];
    assert!(matches!(
        validate_module(&missing_parent),
        Err(ModuleError::IncompletePublishedServiceClosure { .. })
    ));
    let mut unknown = module.clone();
    unknown.boundary_machines[0].fixed_service_reach = vec![service_id(4)];
    assert!(matches!(
        validate_module(&unknown),
        Err(ModuleError::UnknownPublishedService { .. })
    ));
    let mut duplicate = module.clone();
    duplicate.boundary_machines[0]
        .fixed_service_reach
        .push(service_id(2));
    assert!(matches!(
        validate_module(&duplicate),
        Err(ModuleError::DuplicatePublishedService { .. })
    ));
    let mut reordered = module;
    reordered.boundary_machines[0].fixed_service_reach.reverse();
    assert!(matches!(
        validate_module(&reordered),
        Err(ModuleError::NonCanonicalPublishedServiceCeiling(_))
    ));
}

fn declared_callback_module() -> TerminalModule {
    let mut module = call_module();
    module.services.push(ServiceDeclaration {
        id: service_id(1),
        identity: "Console".into(),
        parents: Vec::new(),
    });
    for machine in &mut module.machines {
        machine.published_service_ceiling = vec![service_id(1)];
    }
    module.machines[1].declared_service_reach = vec![service_id(1)];
    module.root_service_reach.concrete = vec![service_id(1)];
    module
}

#[test]
fn inert_callback_declaration_contributes_to_exact_root_reach() {
    let mut module = declared_callback_module();
    validate_module(&module).expect("an inert callback retains its declared contract");
    module.root_service_reach.concrete.clear();
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::RootConcreteServiceReachMismatch {
            declared: Vec::new(),
            derived: vec![service_id(1)],
        }
    );
}

#[test]
fn declared_reach_overlapping_installation_bound_remains_concrete() {
    let mut module = boundary_call_module();
    module.services.push(ServiceDeclaration {
        id: service_id(1),
        identity: "Console".into(),
        parents: Vec::new(),
    });
    module.boundary_machines[0].identity = "Install::step".into();
    module.boundary_machines[0].published_service_ceiling = vec![service_id(1)];
    module.machines[0].published_service_ceiling = vec![service_id(1)];
    module.root_service_reach.installation_dependencies = vec![InstallationReachDependency {
        requirement_identity: "Install::step".into(),
        upper_bound: vec![service_id(1)],
    }];
    validate_module(&module).expect("installation-only upper bound is not concrete");
    let dependency = module.root_service_reach.installation_dependencies.clone();
    module.machines[0].declared_service_reach = vec![service_id(1)];
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::RootConcreteServiceReachMismatch {
            declared: Vec::new(),
            derived: vec![service_id(1)],
        }
    );
    module.root_service_reach.concrete = vec![service_id(1)];
    validate_module(&module).expect("fixed declaration is independent of the overlapping bound");
    assert_eq!(
        module.root_service_reach.installation_dependencies,
        dependency
    );
    module.root_service_reach.installation_dependencies[0]
        .upper_bound
        .clear();
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::InstallationReachBoundaryMismatch(boundary_id(1))
    );
}

#[test]
fn declared_reach_rows_require_known_canonical_parent_closed_services() {
    let original = declared_callback_module();
    let mut unknown = original.clone();
    unknown.machines[1].declared_service_reach = vec![service_id(2)];
    assert!(matches!(
        validate_module(&unknown),
        Err(ModuleError::UnknownPublishedService { .. })
    ));
    let mut duplicate = original.clone();
    duplicate.machines[1]
        .declared_service_reach
        .push(service_id(1));
    assert!(matches!(
        validate_module(&duplicate),
        Err(ModuleError::DuplicatePublishedService { .. })
    ));
    let mut parent = original;
    parent.services.push(ServiceDeclaration {
        id: service_id(2),
        identity: "Queryable".into(),
        parents: Vec::new(),
    });
    parent.services[0].parents = vec![service_id(2)];
    for machine in &mut parent.machines {
        machine.published_service_ceiling = vec![service_id(1), service_id(2)];
    }
    parent.root_service_reach.concrete = vec![service_id(1), service_id(2)];
    assert!(matches!(
        validate_module(&parent),
        Err(ModuleError::IncompletePublishedServiceClosure { .. })
    ));
    parent.machines[1].declared_service_reach = vec![service_id(1), service_id(2)];
    validate_module(&parent).expect("parent-closed declaration");
    parent.machines[1].declared_service_reach.reverse();
    assert!(matches!(
        validate_module(&parent),
        Err(ModuleError::NonCanonicalPublishedServiceCeiling(_))
    ));
}

#[test]
fn declared_reach_cannot_exceed_the_machine_published_ceiling() {
    let mut module = declared_callback_module();
    let machine = module.machines[1].id;
    module.machines[1].published_service_ceiling.clear();
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::DeclaredServiceOutsidePublishedCeiling {
            machine,
            service: service_id(1),
        }
    );
}
