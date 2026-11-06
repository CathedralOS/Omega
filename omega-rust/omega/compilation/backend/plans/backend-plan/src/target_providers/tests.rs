use super::*;
use effects::provider_plan::{
    ProviderBinding, ProviderIdentityTable, ProviderPlan, ProviderPlanRow, ServiceMethod,
    ServiceSchema,
};
use terminal_psi::{
    NominalDeclarationIdentity, ProviderCandidateConformance, TerminalIdentityIndex,
    TerminalIdentityTableBuilder, TerminalModule,
};

fn fixture() -> (TerminalTargetProviderPlan, TerminalModule) {
    let mut identities = ProviderIdentityTable::default();
    let requirement = identities.intern("Parent::read");
    let machine = identities.intern("Shared::read");
    let mut schema_identities = ProviderIdentityTable::default();
    let trait_name = schema_identities.intern("Child");
    let schema_requirement = schema_identities.intern("Parent::read");
    let parent_name = schema_identities.intern("Parent");
    let parent_application = schema_identities.intern("Parent<u64>");
    let argument = schema_identities.intern("u64");
    let selected = TerminalTargetProviderPlan {
        provider_declaration: Some("FirstMap".into()),
        row_numeric_realizations: vec![None],
        plan: ProviderPlan {
            name: "Child selected".into(),
            provider_type: "First".into(),
            target: "macos_arm64".into(),
            identities,
            schema: ServiceSchema::from_declaration(std::sync::Arc::new(
                effects::provider_plan::ServiceSurface {
                    trait_name,
                    receiver_applications: vec![
                        effects::provider_plan::ServiceReceiverApplication {
                            application: trait_name,
                            trait_name,
                            trait_package_identity: None,
                            argument_identities: vec![],
                            lifetime_argument_identities: vec![],
                            requirement_identities: vec![schema_requirement],
                        },
                        effects::provider_plan::ServiceReceiverApplication {
                            application: parent_application,
                            trait_name: parent_name,
                            trait_package_identity: None,
                            argument_identities: vec![argument],
                            lifetime_argument_identities: vec![],
                            requirement_identities: vec![schema_requirement],
                        },
                    ],
                    identities: schema_identities,
                    methods: vec![ServiceMethod {
                        name: "read".into(),
                        requirement_owner: "Parent".into(),
                        requirement_identity: schema_requirement,
                        completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
                        ..ServiceMethod::default()
                    }],
                    ..effects::provider_plan::ServiceSurface::default()
                },
            )),
            rows: vec![ProviderPlanRow {
                method: "read".into(),
                requirement_identity: requirement,
                requirement_lifetime_partition: Vec::new(),
                admitted_same_stack_bytes: 0,
                binding: ProviderBinding::CheckedAdapter {
                    machine_identity: machine,
                    machine_package_identity: None,
                },
            }],
            ..ProviderPlan::default()
        },
    };
    let mut identities = TerminalIdentityTableBuilder::default();
    let mut module = TerminalModule::for_entry(semantic_vocabulary::MachineId::new(1).unwrap());
    // Both choosing owners share bodies. The second closed coordinate is
    // uncalled; no operation roster is needed to project its exact association.
    for (ordinal, (owner, tuple)) in [
        ("FirstMap", "4"),
        ("FirstMap", "8"),
        ("SecondMap", "4"),
        ("SecondMap", "8"),
    ]
    .into_iter()
    .enumerate()
    {
        module
            .provider_candidates
            .push(ProviderCandidateConformance {
                boundary: semantic_vocabulary::BoundaryMachineId::new((ordinal % 2 + 1) as u64)
                    .unwrap(),
                requirement_identity: identities.intern(&format!("Parent::read<{tuple}>")),
                source_requirement_identity: identities.intern("Parent::read"),
                declaration_application: identities.intern("Parent<u64>"),
                receiving_application: identities.intern("Parent<u64>"),
                receiving_region_arguments: Vec::new(),
                provider_identity: identities.intern("First"),
                provider_declaration: NominalDeclarationIdentity::new(identities.intern(owner)),
                candidate_identity: identities.intern(&format!("Shared::read<{tuple}>")),
                source_candidate_identity: identities.intern("Shared::read"),
                source_candidate_package_identity: None,
                candidate: semantic_vocabulary::MachineId::new((ordinal % 2 + 1) as u64).unwrap(),
                signature: terminal_psi::ProviderSignature::empty(),
                refinement: terminal_psi::ProviderRefinement::empty(),
            });
    }
    module.identities = identities.build();
    (selected, module)
}

fn invocation_fixture() -> (TerminalTargetProviderPlan, TerminalModule) {
    let (selected, mut module) = fixture();
    let candidates = module.provider_candidates[..2].to_vec();
    module
        .provider_instance_requirements
        .push(terminal_psi::ProviderInstanceRequirements {
            template: candidates[0].source_requirement_identity,
            package: None,
            selecting_declaration: Some(candidates[0].provider_declaration.clone()),
            source_candidate_identity: candidates[0].source_candidate_identity,
            required_instances: candidates
                .iter()
                .map(|candidate| terminal_psi::RequiredProviderInstance {
                    requirement_identity: candidate.requirement_identity,
                    candidate_identity: candidate.candidate_identity,
                    declaration_application: candidate.declaration_application,
                    receiving_application: candidate.receiving_application,
                    receiving_region_arguments: candidate.receiving_region_arguments.clone(),
                })
                .collect(),
        });
    let scope = &mut module.scalar_applications.proof_scope;
    for (index, candidate) in candidates.iter().enumerate() {
        let declaration = terminal_psi::ProofDeclarationId(index as u32 + 1);
        scope
            .callables
            .insert(terminal_psi::ProofCallableAssociation::Callable {
                declaration,
                callable: terminal_psi::TargetMachineCallable::Machine(candidate.candidate),
            });
        scope
            .callable_headers
            .insert(terminal_psi::ProofCallableHeader {
                declaration,
                callable: Ok(candidate.candidate_identity),
                package: None,
                source_target: Some("macos_arm64".into()),
                product_eligible: true,
                supplies_checked_body: true,
                type_parameter_count: 0,
                synchronous_invocations: Ok(Vec::new()),
                compiler_evaluation: Err("not an evaluation fixture".into()),
            });
    }
    // The source template is independently retained, but its formal telescope
    // cannot describe the invocation of either closed application.
    scope
        .callable_headers
        .insert(terminal_psi::ProofCallableHeader {
            declaration: terminal_psi::ProofDeclarationId(3),
            callable: Ok(candidates[0].source_candidate_identity),
            package: None,
            source_target: None,
            product_eligible: true,
            supplies_checked_body: true,
            type_parameter_count: 1,
            synchronous_invocations: Err("open template invocation".into()),
            compiler_evaluation: Err("not an evaluation fixture".into()),
        });
    (selected, module)
}

#[test]
fn invocation_rows_consume_all_closed_headers_instead_of_the_open_template() {
    let (selected, mut module) = invocation_fixture();
    let plans = [selected];
    let rows = selected_checked_invocation_rows(&plans, &module).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].headers.len(), 2);
    assert_eq!(
        rows[0].headers[1].declaration,
        terminal_psi::ProofDeclarationId(2)
    );
    crate::synchronous_invocations::validate_declared_synchronous_invocation_cycles(
        &plans, &module,
    )
    .unwrap();
    // Instance8 is not the entry, but its missing summary still refuses.
    let handle = module
        .scalar_applications
        .proof_scope
        .callable_headers
        .iter()
        .find_map(|(handle, header)| {
            (header.declaration == terminal_psi::ProofDeclarationId(2)).then_some(handle)
        })
        .unwrap();
    module
        .scalar_applications
        .proof_scope
        .callable_headers
        .get_mut(handle)
        .synchronous_invocations = Err("missing uncalled summary".into());
    let errors = crate::synchronous_invocations::validate_declared_synchronous_invocation_cycles(
        &plans, &module,
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message == "missing uncalled summary")
    );
}

#[test]
fn an_uncalled_closed_invocation_still_forms_a_selected_cycle() {
    let (selected, mut module) = invocation_fixture();
    let mut identities = module.identities.clone().into_builder();
    let declaration = identities.intern("Parent");
    let application = identities.intern("Parent<u64>");
    module.identities = identities.build();
    let headers = &mut module.scalar_applications.proof_scope.callable_headers;
    let handle = headers
        .iter()
        .find_map(|(handle, header)| {
            (header.declaration == terminal_psi::ProofDeclarationId(2)).then_some(handle)
        })
        .unwrap();
    headers.get_mut(handle).synchronous_invocations =
        Ok(vec![terminal_psi::DeclaredSynchronousBoundaryTarget {
            declaration,
            package: None,
            application: Some(application),
        }]);
    let errors = crate::synchronous_invocations::validate_declared_synchronous_invocation_cycles(
        &[selected],
        &module,
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("cyclic synchronous"))
    );
}

#[test]
fn closed_invocation_headers_retain_selected_eligibility_and_instance_obligations() {
    let (selected, module) = invocation_fixture();
    for mismatch in [
        "origin",
        "package",
        "target",
        "callable",
        "product",
        "body",
        "header",
        "association",
        "instance",
        "duplicate",
    ] {
        let mut selected = selected.clone();
        let mut changed = module.clone();
        let header = changed
            .scalar_applications
            .proof_scope
            .callable_headers
            .iter()
            .next()
            .unwrap()
            .0;
        match mismatch {
            "origin" => {
                selected.plan.origin_package_identity =
                    semantic_vocabulary::PackageKeyIdentity::from_digest([9; 32])
            }
            "package" => {
                changed
                    .scalar_applications
                    .proof_scope
                    .callable_headers
                    .get_mut(header)
                    .package = semantic_vocabulary::PackageKeyIdentity::from_digest([9; 32])
            }
            "target" => {
                changed
                    .scalar_applications
                    .proof_scope
                    .callable_headers
                    .get_mut(header)
                    .source_target = Some("windows_x86_64".into())
            }
            "callable" => {
                changed
                    .scalar_applications
                    .proof_scope
                    .callable_headers
                    .get_mut(header)
                    .callable = Ok(TerminalIdentityIndex::ABSENT)
            }
            "product" => {
                changed
                    .scalar_applications
                    .proof_scope
                    .callable_headers
                    .get_mut(header)
                    .product_eligible = false
            }
            "body" => {
                changed
                    .scalar_applications
                    .proof_scope
                    .callable_headers
                    .get_mut(header)
                    .supplies_checked_body = false
            }
            "header" => {
                changed.scalar_applications.proof_scope.callable_headers = Default::default()
            }
            "association" => changed.scalar_applications.proof_scope.callables = Default::default(),
            "instance" => {
                // Keep the independent obligation and declaration view intact.
                // A body for another actual application is not its instance.
                let mut identities = changed.identities.clone().into_builder();
                let other_application = identities.intern("Parent<bool>");
                for candidate in &mut changed.provider_candidates {
                    candidate.receiving_application = other_application;
                }
                changed.identities = identities.build();
            }
            _ => {
                let duplicate = changed
                    .scalar_applications
                    .proof_scope
                    .callable_headers
                    .get(header)
                    .clone();
                changed
                    .scalar_applications
                    .proof_scope
                    .callable_headers
                    .insert(duplicate);
            }
        }
        assert!(
            selected_checked_invocation_rows(&[selected], &changed).is_err(),
            "{mismatch}"
        );
    }
}

#[test]
fn canonical_instances_replace_source_rows_and_keep_uncalled_inherited_coordinates() {
    let (declaration, module) = fixture();
    let selected =
        effects::SelectedProviderPlanFacts::from_selected_plans(vec![declaration.plan.clone()])
            .unwrap();
    let mut identities = module.identities.clone().into_builder();
    let stale = effects::SelectedRequirementSpecialization {
        provider_plan_report_identity: declaration.plan.report_fingerprint(),
        selected_requirement_identity: identities.intern("Parent::read"),
        selected_machine_identity: identities.intern("Shared::read"),
        requirement_identity: identities.intern("Parent::read<999>"),
        machine_identity: identities.intern("StaleSourceBody"),
    };
    let selected = selected
        .with_specialized_requirement_rows(vec![stale], &identities.build())
        .unwrap();
    let closed = close_checked_provider_specializations(selected, &[declaration], &module).unwrap();
    assert!(
        closed
            .specialized_requirement_row("Parent::read<999>")
            .is_none()
    );
    assert_eq!(closed.specialized_requirement_rows().len(), 2);
    for tuple in ["4", "8"] {
        let row = closed
            .specialized_requirement_row(&format!("Parent::read<{tuple}>"))
            .unwrap();
        assert_eq!(
            closed.identities().text(row.machine_identity),
            Some(format!("Shared::read<{tuple}>").as_str())
        );
    }
    let stripped = closed.clone().without_checked_specializations();
    assert!(stripped.specialized_requirement_rows().is_empty());
    assert_eq!(stripped.plans(), closed.plans());
    assert!(stripped.recovery_candidate().validate().is_ok());
}

#[test]
fn canonical_projection_preserves_partial_toolchain_plan_custody() {
    let (mut declaration, module) = fixture();
    let mut missing = declaration.plan.schema.declaration.methods[0].clone();
    missing.name = "unavailable".into();
    let surface = std::sync::Arc::make_mut(&mut declaration.plan.schema.declaration);
    missing.requirement_identity = surface.identities.intern("Parent::unavailable");
    for application in &mut surface.receiver_applications {
        application
            .requirement_identities
            .push(missing.requirement_identity);
    }
    surface.methods.push(missing);
    assert!(
        effects::SelectedProviderPlanFacts::from_selected_plans(vec![declaration.plan.clone()])
            .is_err()
    );
    let selected = effects::SelectedProviderPlanFacts::default()
        .with_toolchain_settled_plan(declaration.plan.clone())
        .unwrap();
    let closed = close_checked_provider_specializations(
        selected,
        std::slice::from_ref(&declaration),
        &module,
    )
    .unwrap();
    assert!(closed.is_toolchain_settled(&declaration.plan));
    assert_eq!(closed.specialized_requirement_rows().len(), 2);
}

#[test]
fn canonical_projection_joins_exact_target_before_rejecting_duplicate_coordinates() {
    let (mut declaration, mut module) = fixture();
    let first = module.provider_candidates[0].clone();
    let mut alternate = first.clone();
    alternate.candidate = semantic_vocabulary::MachineId::new(5).unwrap();
    module.provider_candidates = vec![first.clone(), alternate.clone()];
    let callable = |candidate| terminal_psi::TargetMachineCallable::Machine(candidate);
    module.target_machine_families = vec![terminal_psi::TargetMachineFamily {
        canonical: terminal_psi::TargetMachineCallable::Boundary(first.boundary),
        head_contract: [0; 32],
        members: vec![
            terminal_psi::TargetMachineFamilyMember {
                target: "macos_arm64".into(),
                callable: callable(first.candidate),
                supply: terminal_psi::TargetMachineSupply::CheckedBody,
            },
            terminal_psi::TargetMachineFamilyMember {
                target: "windows_x86_64".into(),
                callable: callable(alternate.candidate),
                supply: terminal_psi::TargetMachineSupply::CheckedBody,
            },
        ],
    }];
    for (target, expected) in [
        ("macos_arm64", first.candidate),
        ("windows_x86_64", alternate.candidate),
    ] {
        declaration.plan.target = target.into();
        let selected = checked_candidate_roster(std::slice::from_ref(&declaration), &module)
            .expect("distinct target members are not duplicates");
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].candidate, expected);
    }
    // Two copies of the selected member still compete in the same target.
    module.provider_candidates.push(alternate);
    assert!(
        checked_candidate_roster(std::slice::from_ref(&declaration), &module)
            .unwrap_err()
            .contains("duplicate canonical realizations")
    );
    declaration.plan.target = "linux_arm64".into();
    assert!(
        checked_candidate_roster(std::slice::from_ref(&declaration), &module)
            .unwrap_err()
            .contains("no emitted exact declaration candidate")
    );
}

#[test]
fn canonical_join_rejects_missing_owner_template_and_duplicate_coordinate() {
    let (declaration, module) = fixture();
    for mismatch in [
        "owner",
        "requirement",
        "body",
        "receiver",
        "package",
        "duplicate",
    ] {
        let mut changed = module.clone();
        match mismatch {
            "owner" => changed.provider_candidates.retain(|row| {
                module.identities.text(row.provider_declaration.identity()) != Some("FirstMap")
            }),
            "requirement" => changed
                .provider_candidates
                .iter_mut()
                .for_each(|row| row.source_requirement_identity = TerminalIdentityIndex::ABSENT),
            "body" => changed
                .provider_candidates
                .iter_mut()
                .for_each(|row| row.source_candidate_identity = TerminalIdentityIndex::ABSENT),
            "receiver" => {
                let mut identities = changed.identities.clone().into_builder();
                let substituted = identities.intern("Parent<bool>");
                for row in &mut changed.provider_candidates {
                    row.declaration_application = substituted;
                    row.receiving_application = substituted;
                }
                changed.identities = identities.build();
            }
            "package" => {
                // The plan's None is an exact package coordinate, not a wildcard.
                for row in &mut changed.provider_candidates {
                    row.source_candidate_package_identity =
                        semantic_vocabulary::PackageKeyIdentity::from_digest([9; 32]);
                }
            }
            _ => changed
                .provider_candidates
                .push(changed.provider_candidates[0].clone()),
        }
        let error = checked_candidate_roster(std::slice::from_ref(&declaration), &changed)
            .expect_err(mismatch);
        assert!(
            error.contains(if mismatch == "duplicate" {
                "duplicate canonical realizations"
            } else {
                "no emitted exact declaration candidate"
            }),
            "{mismatch}: {error}"
        );
    }
}

#[test]
fn component_and_native_projection_retain_the_same_complete_closed_roster() {
    let (selected, module) = fixture();
    let projected = selected_provider_candidate_rows(&selected.plan, "FirstMap", &module).unwrap();
    assert_eq!(projected.len(), 1);
    assert_eq!(projected[0].candidate_positions, vec![0, 1]);
    let installations = checked_candidate_roster(std::slice::from_ref(&selected), &module).unwrap();
    assert_eq!(installations.len(), projected[0].candidate_positions.len());
    for (installation, position) in installations.iter().zip(&projected[0].candidate_positions) {
        assert_eq!(
            installation.boundary,
            module.provider_candidates[*position].boundary
        );
        assert_eq!(
            installation.candidate,
            module.provider_candidates[*position].candidate
        );
        assert_eq!(installation.provider_declaration, "FirstMap");
    }
    assert!(selected_provider_candidate_rows(&selected.plan, "", &module).is_err());
}

#[test]
fn inherited_requirement_uses_its_declaring_receiver_not_the_child_view() {
    let (mut selected, mut module) = fixture();
    let surface = std::sync::Arc::make_mut(&mut selected.plan.schema.declaration);
    let child_application = surface.identities.intern("Child<u8>");
    let argument = surface.identities.intern("u8");
    surface.receiver_applications[0].application = child_application;
    surface.receiver_applications[0].argument_identities = vec![argument];
    surface.receiver_applications[1].application = surface.receiver_applications[1].trait_name;
    surface.receiver_applications[1].argument_identities.clear();
    for candidate in &mut module.provider_candidates {
        candidate.declaration_application = TerminalIdentityIndex::ABSENT;
        candidate.receiving_application = TerminalIdentityIndex::ABSENT;
    }
    assert_eq!(
        checked_candidate_roster(std::slice::from_ref(&selected), &module)
            .unwrap()
            .len(),
        2
    );

    let mut identities = module.identities.clone().into_builder();
    let substituted_child = identities.intern("Child<u8>");
    module.identities = identities.build();
    for candidate in &mut module.provider_candidates {
        candidate.declaration_application = substituted_child;
        candidate.receiving_application = substituted_child;
    }
    assert!(checked_candidate_roster(std::slice::from_ref(&selected), &module).is_err());
}
