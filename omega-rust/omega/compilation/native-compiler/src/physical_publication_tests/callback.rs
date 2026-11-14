//! The registrar takes a native-only callback pointer. Its callback executes a
//! checked boundary body, so both namespaces have a nonempty physical child.
//! This is native admission input, not a claim about source/Build checking.

use std::sync::Arc;

use arena::{Arena, HandleSpan};
use calling_conventions::{
    BoundaryCallbackBinding, BoundaryCallingApplication, BoundaryNativeParameterGeometry,
    BoundaryNativeParameterOrigin, BoundaryNativeParameterShape, BoundarySignatureGeometry,
    CallSignature, CallbackBinderRequirement, CallbackMaterialization,
    CallbackMaterializationContext, CallingPolicy, ExternalBindingKind, ExternalBindingRow,
    NativeCallbackDemand, NativeParameterApplication, NativeParameterId, NativePlace,
    StaticMachineBinderId, ValueShape, callback_requirement_id,
    evaluate_ordinary_boundary_entry_plan,
    validate_boundary_entry_plan_with_callback_materializations,
};
use effects::provider_plan::{
    BoundaryCallingPlanCommitment, BoundaryCallingPlanPromise, EvaluatedBindingEvaluationDigest,
    EvaluatedBindingMaterializationDigest, EvaluatedBindingProducerClosureDigest,
    EvaluatedBindingReceipt, EvaluatedBindingUsage, EvaluatedForeignImport, ProviderBinding,
    ProviderPlan, ProviderPlanRow, ServiceCallingApplication, ServiceCallingPlan, ServiceMethod,
};
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{
    BoundaryMachineResult, CallingAbiDeclaration, CallingAbiStorage, CallingCallbackBinder,
    CallingDirectCallback, CallingSignatureDeclaration, CallingSignatureTelescope,
    CheckedMachineDeclaration, NominalDeclarationIdentity, OperationKind, OperationResult,
    TargetMachineCallable, TerminalCallbackApplication, TerminalIdentityTableBuilder,
    TerminalMachineResult, Terminator,
};

const REGISTRAR: &str = "PhysicalFixture::register";
const ENTRY: &str = "PhysicalFixture::callback_entry";
const OWNER: &str = "PhysicalCalling";

struct Fixture {
    artifact: Arc<CanonicalTerminalArtifact>,
    selected: effects::SelectedProviderPlanFacts,
    external: Vec<ExternalBindingRow>,
    calling: Vec<BoundaryCallingApplication>,
    callbacks: Vec<terminal_psi_to_abstract_operations::NativeCallbackArgumentProposal>,
    occurrences: Vec<backend_plan::TerminalCallbackOccurrenceProposal>,
}

fn calling_application(requirement_identity: &str, callback: bool) -> BoundaryCallingApplication {
    let target = target::TargetProfile::WindowsX64.native_target();
    let binder = StaticMachineBinderId::new(1).unwrap();
    let requirement = callback_requirement_id(ENTRY);
    let parameter = NativeParameterId::new(3).unwrap();
    let destination = NativePlace::Parameter(parameter);
    let context = if callback {
        CallbackMaterializationContext {
            binders: vec![CallbackBinderRequirement {
                binder,
                requirement,
            }],
            demands: vec![NativeCallbackDemand {
                destination: destination.clone(),
                requirement,
            }],
        }
    } else {
        CallbackMaterializationContext::default()
    };
    let signature = CallSignature {
        parameters: if callback {
            vec![ValueShape::integer(8, 8)]
        } else {
            Vec::new()
        },
        result: None,
    };
    let mut plan = evaluate_ordinary_boundary_entry_plan(CallingPolicy::MicrosoftX64, &signature)
        .unwrap()
        .plan()
        .clone();
    if callback {
        plan.call
            .callback_materializations
            .push(CallbackMaterialization {
                binder,
                destination,
            });
    }
    let plan =
        validate_boundary_entry_plan_with_callback_materializations(plan, &signature, &context)
            .unwrap();
    let mut native_parameters = Arena::new();
    let native_parameter_rows = native_parameters.insert_many(if callback {
        vec![BoundaryNativeParameterGeometry {
            identity: parameter,
            native_ordinal: 0,
            shape: BoundaryNativeParameterShape::function_pointer(target).unwrap(),
            origin: BoundaryNativeParameterOrigin::PrivateCallback {
                binder,
                requirement,
            },
        }]
    } else {
        Vec::new()
    });
    let mut callback_bindings = Arena::new();
    let callback_binding_rows = callback_bindings.insert_many(if callback {
        vec![BoundaryCallbackBinding {
            binder,
            requirement,
            static_machine_ordinal: 0,
            requirement_identity: ENTRY.into(),
            invokes_synchronously: true,
        }]
    } else {
        Vec::new()
    });
    let mut application = BoundaryCallingApplication {
        abi: semantic_vocabulary::CallingAbiIdentity {
            requirement_identity: requirement_identity.into(),
            declaring_application: "PhysicalFixture".into(),
            operands: Vec::new(),
        },
        calling_owner_application: Some(OWNER.into()),
        requirement_identity: requirement_identity.into(),
        target,
        report_fingerprint: plan.contract_report_fingerprint(),
        commitment: [0; 32],
        plan,
        dispatch_only_parameter_count: 0,
        signature: BoundarySignatureGeometry {
            shapes: Vec::new(),
            fields: Vec::new(),
            parameters: Vec::new(),
            result: None,
            native_parameters,
            native_parameter_rows,
            callback_layouts: Arena::new(),
            callback_layout_rows: HandleSpan::empty(),
            callback_bindings,
            callback_binding_rows,
        },
        opaque_applications: Vec::new(),
        callback_context: context,
        callback_context_closed: true,
    };
    application.commitment = application.recomputed_commitment().unwrap();
    application
}

fn build_fixture() -> Fixture {
    let mut module = super::constant_artifact(true).semantic_module().clone();
    let mut identities = TerminalIdentityTableBuilder::default();
    identities.intern("PhysicalFixture::value");
    let registrar_name = identities.intern(REGISTRAR);
    let callback_name = identities.intern("PhysicalFixture::callback");
    let entry_name = identities.intern(ENTRY);
    let unit_name = identities.intern("()");
    let owner = identities.intern("PhysicalFixture");
    let declaration = CheckedMachineDeclaration {
        machine: NominalDeclarationIdentity::new(callback_name),
        state: NominalDeclarationIdentity::new(callback_name),
        semantic_declaration: callback_name,
        parameter_types: Vec::new(),
        callable: callback_name,
        result_type: unit_name,
        contract: [9; 32],
        published_may_suspend: false,
        published_may_block: false,
        specialization: None,
    };
    let mut value = module.machines[1].clone();
    value.id = MachineId::new(3).unwrap();
    value.contract.id = semantic_vocabulary::ContractId::new(3).unwrap();
    value.blocks[0].operations[0].id = OperationId::new(3).unwrap();
    module.machines[1].blocks[0].operations[0].kind = OperationKind::BoundaryCall {
        boundary: BoundaryMachineId::new(2).unwrap(),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        completion_receipts: Vec::new(),
        requirement_obligations: Vec::new(),
    };
    module.machines[1].blocks[0].terminator = Terminator::ReturnUnit {
        edge: semantic_vocabulary::EdgeId::new(2).unwrap(),
        cleanup_actions: Vec::new(),
    };
    module.machines[1].contract.result = TerminalMachineResult::Unit;
    module.machines[1].contract.declaration = Some(declaration.clone());
    module.machines[0].blocks[0].operations[0].result = OperationResult::Unit;
    let mut root_value = value.blocks[0].operations[0].clone();
    root_value.id = OperationId::new(4).unwrap();
    module.machines[0].blocks[0].operations.push(root_value);
    module.machines.push(value);
    let mut value_boundary = module.boundary_machines[0].clone();
    value_boundary.id = BoundaryMachineId::new(2).unwrap();
    module.boundary_machines[0].identity = registrar_name;
    module.boundary_machines[0].owner = NominalDeclarationIdentity::new(owner);
    module.boundary_machines[0].result = BoundaryMachineResult::Unit;
    let mut entry_boundary = module.boundary_machines[0].clone();
    entry_boundary.id = BoundaryMachineId::new(3).unwrap();
    entry_boundary.identity = entry_name;
    module
        .boundary_machines
        .extend([value_boundary, entry_boundary]);
    module.target_machine_families[0].canonical =
        TargetMachineCallable::Boundary(BoundaryMachineId::new(2).unwrap());
    module.target_machine_families[0].members[0].callable =
        TargetMachineCallable::Machine(MachineId::new(3).unwrap());
    module
        .callback_applications
        .push(TerminalCallbackApplication {
            application_index: 0,
            registrar: OperationId::new(1).unwrap(),
            static_machine_ordinal: 0,
            requirement: ENTRY.into(),
            calling_owner_application: OWNER.into(),
            boundary: BoundaryMachineId::new(3),
            published_contract: [9; 32],
            entry_admitted: true,
            body: declaration,
        });
    let registrar = calling_application(REGISTRAR, true);
    let inbound = calling_application(ENTRY, false);
    for (boundary, application) in [(1, &registrar), (3, &inbound)] {
        let callback = boundary == 1;
        module.calling_signatures.push(CallingSignatureDeclaration {
            boundary: BoundaryMachineId::new(boundary).unwrap(),
            application: Ok(application.abi.id()),
        });
        module.calling_abi_signatures.push(CallingAbiDeclaration {
            id: application.abi.id(),
            requirement_identity: identities.intern(&application.requirement_identity),
            declaring_application: owner,
            operands: Vec::new(),
            formals: Vec::new(),
            default_eligible: true,
            physical_owner: Ok(semantic_vocabulary::CallingDeclarationOwner::TopLevelRequirement),
            storage: Ok(CallingAbiStorage {
                parameters: Vec::new(),
                result: None,
                telescope: CallingSignatureTelescope {
                    parameters: Vec::new(),
                    binders: if callback {
                        vec![CallingCallbackBinder {
                            binder: StaticMachineBinderId::new(1).unwrap(),
                            requirement: callback_requirement_id(ENTRY),
                            requirement_identity: entry_name,
                            static_machine_ordinal: 0,
                            invokes_synchronously: true,
                        }]
                    } else {
                        Vec::new()
                    },
                    direct_callbacks: if callback {
                        vec![CallingDirectCallback {
                            identity: NativeParameterId::new(3).unwrap(),
                            native_ordinal: 0,
                            binder: StaticMachineBinderId::new(1).unwrap(),
                            requirement: callback_requirement_id(ENTRY),
                        }]
                    } else {
                        Vec::new()
                    },
                },
            }),
        });
    }
    let locator = target::normalize_foreign_locator(
        target::ForeignLocatorCandidate::PeByName {
            library: b"fixture.dll".to_vec(),
            export: b"register_callback".to_vec(),
        },
        target::TargetProfile::WindowsX64,
    )
    .unwrap();
    let receipt = EvaluatedBindingReceipt::from_evaluation(
        None,
        "fixture::binding",
        &mut identities,
        EvaluatedBindingProducerClosureDigest::from_bytes([11; 32]).unwrap(),
        1,
        EvaluatedBindingUsage::from_evaluator(1, 1, 1, 100, 0, 0, 1, 1, 1, 1).unwrap(),
        EvaluatedBindingEvaluationDigest::from_bytes([12; 32]).unwrap(),
        1,
        EvaluatedBindingMaterializationDigest::from_bytes([13; 32]).unwrap(),
        locator.identity_digest(),
    )
    .unwrap();
    module.identities = identities.build();
    let optimization =
        terminal_codec::build_identity_optimization_execution_record(&module).unwrap();
    let artifact =
        Arc::new(CanonicalTerminalArtifact::from_parts(module, optimization, None).unwrap());
    let mut plan = ProviderPlan {
        name: "physical-registrar".into(),
        provider_type: "PhysicalProvider".into(),
        target: "windows_x86_64".into(),
        ..ProviderPlan::default()
    };
    plan.provider_application = plan.identities.intern("PhysicalProvider");
    let surface = Arc::make_mut(&mut plan.schema.declaration);
    surface.trait_name = surface.identities.intern("PhysicalFixture");
    surface.methods.push(ServiceMethod {
        name: "register".into(),
        requirement_owner: "PhysicalFixture".into(),
        requirement_identity: surface.identities.intern(REGISTRAR),
        terminates_guarantee: true,
        ..ServiceMethod::default()
    });
    let requirement_identity = plan.identities.intern(REGISTRAR);
    plan.rows.push(ProviderPlanRow {
        method: "register".into(),
        requirement_identity,
        declared_requirement_identity: requirement_identity,
        calling_signature: Some(registrar.abi.id()),
        requirement_lifetime_partition: Vec::new(),
        admitted_same_stack_bytes: 1024,
        binding: ProviderBinding::Import {
            evaluated: EvaluatedForeignImport::from_retained_evidence(locator.clone(), receipt)
                .unwrap(),
        },
    });
    plan.schema
        .calling_applications
        .push(ServiceCallingApplication {
            requirement: plan.schema.declaration.methods[0].requirement_identity,
            abi: registrar.abi.clone(),
            plan: ServiceCallingPlan {
                report_fingerprint: registrar.report_fingerprint,
                commitment: BoundaryCallingPlanCommitment::from_digest(registrar.commitment),
                promise: BoundaryCallingPlanPromise::from_digest(
                    registrar.plan.contract_commitment_digest(),
                ),
            },
        });
    let selected = effects::SelectedProviderPlanFacts::from_selected_plans(vec![plan]).unwrap();
    let identity =
        function_identity::MachineFunctionIdentity::callback_thunk(artifact.view().identity(), 0)
            .unwrap();
    let occurrence = backend_plan::TerminalCallbackOccurrenceProposal::new(
        0,
        0,
        OperationId::new(1).unwrap(),
        identity,
        backend_plan::TerminalCallbackThunkArtifact::for_application(Arc::clone(&artifact), 0)
            .unwrap(),
        inbound.plan.plan().clone(),
        Vec::new(),
    );
    let callback = terminal_psi_to_abstract_operations::NativeCallbackArgumentProposal {
        registrar_invokes_callback: true,
        terminal_operation: OperationId::new(1).unwrap(),
        placement_index: 0,
        callback_function: identity,
        destination: NativePlace::Parameter(NativeParameterId::new(3).unwrap()),
        field_placements: None,
        application: NativeParameterApplication {
            parameter: NativeParameterId::new(3).unwrap(),
            native_ordinal: 0,
            shape: ValueShape::integer(8, 8),
            placement: registrar.plan.plan().call.parameters[0].clone(),
        },
        registrar_boundary_entry_plan: registrar.plan.plan().clone(),
        registrar_context: registrar.callback_context.clone(),
        registrar_application_commitment: registrar.commitment,
        registrar_application: registrar.clone(),
        binder: StaticMachineBinderId::new(1).unwrap(),
    };
    Fixture {
        artifact,
        selected,
        external: vec![ExternalBindingRow {
            target_name: "windows_x86_64".into(),
            trait_name: "PhysicalFixture".into(),
            method: "register".into(),
            requirement_identity: REGISTRAR.into(),
            provider_type: "PhysicalProvider".into(),
            calling_applications: vec![registrar.clone()],
            binding: ExternalBindingKind::Import { locator },
        }],
        calling: vec![registrar, inbound],
        callbacks: vec![callback],
        occurrences: vec![occurrence],
    }
}

#[test]
fn physical_publication_retains_linked_callback_children() {
    check_callback_publication(false);
    check_callback_publication(true);
}

fn check_callback_publication(retain_body_source: bool) {
    use terminal_psi_to_abstract_operations::provider_admission::{
        ProviderAdmissionRequest, current_terminal_authority_policy,
        prepare_source_evaluated_bindings,
    };
    use terminal_psi_to_abstract_operations::{
        NativeAdmissionRequest, NativeExecutionSelection, VerifiedNativeArtifactInput,
        admit_prepared_native_realization,
    };
    let fixture = build_fixture();
    let artifact = &fixture.artifact;
    let demands = boundary_applications::TerminalBoundaryApplicationDemands::new(
        artifact.manifest().semantic(),
        Vec::new(),
    )
    .unwrap();
    let realizations =
        boundary_applications::TerminalBoundaryApplicationRealizations::new(&demands, Vec::new())
            .unwrap();
    let coverage =
        boundary_applications::TerminalBoundaryApplicationCoverage::new(demands, realizations)
            .unwrap();
    let prepared = prepare_source_evaluated_bindings(
        fixture.selected.plans(),
        &std::collections::BTreeSet::from([REGISTRAR.to_owned()]),
        &[],
        fixture.selected.identities(),
    )
    .expect("derive selected foreign execution and stack custody");
    let settlements = prepared.settlements();
    let policy = current_terminal_authority_policy();
    let exclusions = effects::BehaviorExclusions::default();
    let request = NativeAdmissionRequest {
        entry: TargetMachineCallable::Machine(MachineId::new(1).unwrap()),
        private_entry_ram: &[],
        boundary_application_coverage: Some(&coverage),
        behavior_exclusions: &exclusions,
        providers: ProviderAdmissionRequest {
            profile: target::TargetProfile::WindowsX64,
            bootstrap_entry: MachineId::new(1).unwrap(),
            program_provider_context: None,
            installation_assumptions: &[],
            selected_provider_plans: &fixture.selected,
            checked_adapters: &[],
            external_binding_rows: &fixture.external,
            settlements: &settlements,
            compiler_builtins: &[],
            native_callbacks: &fixture.callbacks,
            installed_interrupts: &[],
            callback_occurrences: &fixture.occurrences,
            installed_calling: &fixture.calling,
            terminal_authority_policy: &policy,
            terminal_authority_permission_policy: &None,
            x86_scalar_fma_provider: None,
        },
    };
    let selection = NativeExecutionSelection {
        scope: None,
        scalar_applications: None,
        providers: &[],
        plans: &fixture.selected,
        callbacks: &fixture.occurrences,
        entry_call: None,
    };
    let input = VerifiedNativeArtifactInput::prepare_with_private_entry_ram(
        artifact,
        target::TargetProfile::WindowsX64,
        request.entry,
        &[],
        &selection,
    )
    .expect("prepare real callback occurrence closure");
    let admitted = admit_prepared_native_realization(input, artifact, &request)
        .expect("admit callback registrar with exact selected foreign contract");
    let (root, providers) = super::emit_body(admitted);
    let program_context = Arc::clone(providers.program_provider_context());
    let body_request = NativeAdmissionRequest {
        entry: TargetMachineCallable::Machine(MachineId::new(2).unwrap()),
        providers: ProviderAdmissionRequest {
            program_provider_context: Some(&program_context),
            ..request.providers
        },
        ..request
    };
    let input = VerifiedNativeArtifactInput::prepare_with_private_entry_ram(
        artifact,
        target::TargetProfile::WindowsX64,
        body_request.entry,
        &[],
        &selection,
    )
    .expect("prepare callback body under the same retained program");
    let admitted = admit_prepared_native_realization(input, artifact, &body_request)
        .expect("admit actual callback body");
    let (body, _) = super::emit_body(admitted);
    let body = if retain_body_source {
        body.with_semantic_source(artifact.retain_code(), coverage.clone())
            .unwrap()
    } else {
        body
    };
    let private = image_emission::create_private_function_object(
        fixture.occurrences[0].callback_thunk_identity(),
        Arc::new(body.into_object()),
        fixture.occurrences[0].boundary_entry_plan(),
        &[],
    )
    .expect("bridge the actual callback body to its admitted inbound ABI");
    let published = native_artifact::publish_fragment_object(
        root,
        native_artifact::FragmentObjectPublicationRequest {
            program_entry: None,
            private_functions: &[private],
        },
    )
    .expect("link demanded callback object, not an injected unused body");
    let requested = native_artifact::RequestedNativeArtifact::emit_admitted(
        target::TargetProfile::WindowsX64.native_target(),
        Arc::clone(artifact),
        published,
        image_emission::ExecutableImageEmissionRequest::direct(3),
        &fixture.selected,
        providers,
        Some(coverage),
    )
    .expect("publish physical evidence over root and relocated private body");
    let native_artifact::RequestedNativeArtifact::Direct(native) = requested else {
        panic!("PE image")
    };
    if !retain_body_source {
        assert!(native.physical_evidence().is_none());
        assert!(
            native.physical_evidence_gap().is_some(),
            "uncovered private body must remain explicit"
        );
        native
            .validate()
            .expect("receiver retains the same missing-body gap");
        return;
    }
    let physical = native
        .physical_evidence()
        .expect("complete composed physical evidence");
    // This raw admission input requests full-module emission. Each namespace
    // therefore retains both the registrar and the callback's checked call.
    assert_eq!(physical.children().len(), 2);
    assert_eq!(physical.private_functions().len(), 1);
    let private = &physical.private_functions()[0];
    assert_eq!(
        private.function(),
        fixture.occurrences[0].callback_thunk_identity()
    );
    assert_eq!(private.evidence().children().len(), 2);
    assert!(private.text_offset() > 0);
    let child = private.evidence().children()[0].clone().into_parts();
    assert_eq!(
        child.final_image_spans[0].offset(),
        child.object_spans[0].offset() + private.text_offset()
    );
    native
        .validate()
        .expect("receiver independently rejoins both namespaces and image spans");
    let mut parts = native.into_parts();
    parts.physical_evidence = None;
    assert!(
        native_artifact::NativeArtifact::from_replayed_parts(parts).is_err(),
        "cannot strip composed custody"
    );
}
