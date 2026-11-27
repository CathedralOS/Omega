//! The registrar takes a native-only callback pointer. Its callback executes a
//! checked boundary body, so both namespaces have a nonempty physical child.
//! This is native admission input, not a claim about source/Build checking.

use std::sync::Arc;

use arena::{Arena, HandleSpan};
use build_providers::plans::{
    BoundaryCallingPlanCommitment, BoundaryCallingPlanPromise, EvaluatedBindingEvaluationDigest,
    EvaluatedBindingMaterializationDigest, EvaluatedBindingProducerClosureDigest,
    EvaluatedBindingReceipt, EvaluatedBindingUsage, EvaluatedForeignImport, ProviderBinding,
    ProviderPlanRow, ServiceCallingApplication, ServiceCallingPlan, ServiceMethod,
};
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
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{
    BoundaryMachineResult, CallingAbiDeclaration, CallingAbiStorage, CallingCallbackBinder,
    CallingDirectCallback, CallingSignatureDeclaration, CallingSignatureTelescope,
    CheckedMachineDeclaration, NominalDeclarationIdentity, OperationKind, OperationResult,
    TargetMachineCallable, TerminalCallbackApplication, TerminalMachineResult, Terminator,
};

const REGISTRAR: &str = "PhysicalFixture::register";
const ENTRY: &str = "PhysicalFixture::callback_entry";
const OWNER: &str = "PhysicalCalling";

struct Fixture {
    artifact: Arc<CanonicalTerminalArtifact>,
    selected: build_providers::selections::SelectedProviderPlanFacts,
    external: Vec<ExternalBindingRow>,
    calling: Vec<BoundaryCallingApplication>,
    callbacks: Vec<provider_admission::NativeCallbackArgumentProposal>,
    occurrences: Vec<provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal>,
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
        dispatch_only_formal: None,
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
    let mut identities = module.identities.clone().into_builder();
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
    let mut value_boundary = module.interfaces.boundary_machines[0].clone();
    value_boundary.id = BoundaryMachineId::new(2).unwrap();
    module.interfaces.boundary_machines[0].identity = registrar_name;
    module.interfaces.boundary_machines[0].owner = NominalDeclarationIdentity::new(owner);
    module.interfaces.boundary_machines[0].result = BoundaryMachineResult::Unit;
    let mut entry_boundary = module.interfaces.boundary_machines[0].clone();
    entry_boundary.id = BoundaryMachineId::new(3).unwrap();
    entry_boundary.identity = entry_name;
    module
        .interfaces
        .boundary_machines
        .extend([value_boundary, entry_boundary]);
    module.interfaces.provider_candidates[0].boundary = BoundaryMachineId::new(2).unwrap();
    module.interfaces.provider_candidates[0].candidate = MachineId::new(3).unwrap();
    module
        .interfaces
        .callback_applications
        .push(TerminalCallbackApplication {
            application_index: 0,
            registrar_machine: MachineId::new(1).unwrap(),
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
        module
            .interfaces
            .calling_signatures
            .push(CallingSignatureDeclaration {
                boundary: BoundaryMachineId::new(boundary).unwrap(),
                application: Ok(application.abi.id()),
            });
        module
            .interfaces
            .calling_abi_signatures
            .push(CallingAbiDeclaration {
                id: application.abi.id(),
                requirement_identity: identities.intern(&application.requirement_identity),
                declaring_application: owner,
                operands: Vec::new(),
                formals: Vec::new(),
                default_eligible: true,
                physical_owner: Ok(
                    semantic_vocabulary::CallingDeclarationOwner::TopLevelRequirement,
                ),
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
    // Both operations belong to one slot, so its selected record contains the
    // registrar import beside the checked value implementation.
    let mut plan = super::checked_value_plan();
    plan.name = "physical-registrar-and-value".into();
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
            requirement: plan
                .schema
                .declaration
                .methods
                .last()
                .unwrap()
                .requirement_identity,
            abi: registrar.abi.clone(),
            plan: ServiceCallingPlan {
                report_fingerprint: registrar.report_fingerprint,
                commitment: BoundaryCallingPlanCommitment::from_digest(registrar.commitment),
                promise: BoundaryCallingPlanPromise::from_digest(
                    registrar.plan.contract_commitment_digest(),
                ),
            },
        });
    let provider_type = plan.provider_type.clone();
    let selected =
        build_providers::selections::SelectedProviderPlanFacts::from_selected_plans(vec![plan])
            .unwrap();
    let identity =
        function_identity::MachineFunctionIdentity::callback_thunk(artifact.view().identity(), 0)
            .unwrap();
    let occurrence = provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal::new(
        0,
        0,
        OperationId::new(1).unwrap(),
        identity,
        provider_correspondence::callbacks::TerminalCallbackThunkArtifact::for_application(
            Arc::clone(&artifact),
            0,
        )
        .unwrap(),
        inbound.plan.plan().clone(),
        Vec::new(),
    );
    let callback = provider_admission::NativeCallbackArgumentProposal {
        registrar_machine: MachineId::new(1).unwrap(),
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
            provider_type,
            calling_applications: vec![registrar.clone()],
            binding: ExternalBindingKind::Import { locator },
        }],
        calling: vec![registrar, inbound],
        callbacks: vec![callback],
        occurrences: vec![occurrence],
    }
}

fn check_layout_fixup_identity(program: &machine_code::ResolvedMachineProgram) {
    let original = program.layout.identity();
    assert_eq!(original, program.layout.recomputed_identity());
    encoding_admission::layout::validate_baseline_resolved_layout(program).unwrap();

    let check_edit = |edit: fn(&mut machine_code::SelectedFormNormalizedForeignCallFixup)| {
        let mut changed = program.clone();
        let layout = Arc::make_mut(&mut changed.layout);
        let registrar = layout
            .functions
            .iter_mut()
            .flat_map(|function| &mut function.blocks)
            .flat_map(|block| &mut block.instructions)
            .filter_map(|row| row.normalized_foreign_call_fixup.as_deref_mut())
            .find(|fixup| !fixup.callbacks.is_empty())
            .expect("the registrar calls an import and materializes the callback");
        edit(registrar);
        layout.identity = layout.recomputed_identity();
        assert_ne!(original, layout.identity);
        assert_eq!(
            encoding_admission::layout::validate_baseline_resolved_layout(&changed),
            Err(encoding_admission::layout::OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch),
        );
    };
    check_edit(|fixup| fixup.ordinal += 1);
    check_edit(|fixup| fixup.callbacks.clear());
}

#[test]
fn physical_publication_retains_linked_callback_children() {
    check_callback_publication(false);
    check_callback_publication(true);
}

fn check_callback_publication(retain_body_source: bool) {
    use crate::driver::image::plan_callables::inspect_private_body_demand;
    use artifact_lowering::test_support::NativeExecutionSelection;
    use artifact_lowering::{
        NativeAdmissionRequest, VerifiedNativeArtifactInput, admit_prepared_native_realization,
    };
    use provider_admission::{
        ProviderAdmissionRequest, current_terminal_authority_policy,
        prepare_source_evaluated_bindings,
    };
    let fixture = build_fixture();
    let adapters =
        provider_admission::retain_selected_provider_adapters(&fixture.selected).unwrap();
    let artifact = &fixture.artifact;
    let demands = terminal_psi::TerminalBoundaryApplicationDemands::new(
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
    let exclusions = native_behavior_exclusions::BehaviorExclusions::default();
    let request = NativeAdmissionRequest {
        entry: TargetMachineCallable::Machine(MachineId::new(1).unwrap()),
        private_entry_ram: &[],
        boundary_application_coverage: Some(&coverage),
        behavior_exclusions: &exclusions,
        isolated_execution_grant: None,
        providers: ProviderAdmissionRequest {
            native_bindings: &Default::default(),
            profile: target::TargetProfile::WindowsX64,
            bootstrap_entry: MachineId::new(1).unwrap(),
            program_provider_context: None,
            installation_assumptions: &[],
            selected_provider_plans: &fixture.selected,
            checked_adapters: &adapters,
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
    let admit = || {
        let input = VerifiedNativeArtifactInput::prepare_with_private_entry_ram(
            &super::authorize_fixture(artifact),
            target::TargetProfile::WindowsX64,
            request.entry,
            &[],
            &selection,
        )
        .expect("prepare real callback occurrence closure");
        admit_prepared_native_realization(input, artifact, &request)
            .expect("admit callback registrar with exact selected foreign contract")
    };
    if !retain_body_source {
        let substituted = build_providers::selections::SelectedProviderPlanFacts::default();
        let Err(errors) =
            native_artifact::NativeImageProviders::from_admission(admit().providers, &substituted)
        else {
            panic!("accepted unrelated provider selections for the callback program")
        };
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("another selected-plan closure")),
            "{errors:?}"
        );
    }
    let admitted = admit();
    let (root, providers) = super::emit_body(admitted, retain_body_source, |target| {
        assert!(
            inspect_private_body_demand(&fixture.occurrences, &[])
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            inspect_private_body_demand(&fixture.occurrences, &[target, target]).unwrap(),
            [0],
            "repeated demand schedules the original placement once"
        );
        let missing = inspect_private_body_demand(&[], &[target]).unwrap_err();
        assert!(missing.iter().any(|error| {
            error
                .message
                .contains("referenced private function has no original placement")
        }));
        let occurrence = &fixture.occurrences[0];
        let substituted =
            provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal::new(
                occurrence.placement_index(),
                occurrence.application_index(),
                OperationId::new(99).unwrap(),
                occurrence.callback_thunk_identity(),
                occurrence.callback_thunk_artifact().clone(),
                occurrence.boundary_entry_plan().clone(),
                occurrence.entry_parameters().to_vec(),
            );
        let wrong = inspect_private_body_demand(&[substituted], &[target]).unwrap_err();
        assert!(wrong.iter().any(|error| {
            error
                .message
                .contains("differs from its admitted occurrence")
        }));

        assert_eq!(
            target.private_references(),
            &[fixture.occurrences[0].callback_thunk_identity()]
        );
        let callback = MachineId::new(2).unwrap();
        let facts = target.execution_plan().execution_facts();
        assert!(facts.callable_invocations().roots.contains(&callback));
        assert!(
            facts
                .callable_invocations_from(callback)
                .expect("the activated callback has its independently prepared context")
                .roots
                .contains(&callback)
        );
    });
    let root_originals = root.originals;
    let root = root.projection;
    let object = root.object();
    let section = object.publication().text();
    let mut edited = section.clone();
    let registrar = edited
        .unresolved_normalized_foreign_calls
        .iter_mut()
        .find(|resolution| !resolution.callback_materializations.is_empty())
        .expect("the placed registrar carries its callback address");
    registrar.callback_materializations[0].code_section_offset += 1;
    assert_ne!(*section, edited);

    let bindings = object.publication().bindings();
    let mut missing_import = bindings.clone();
    let import = Arc::make_mut(&mut missing_import.imports)
        .imports
        .pop()
        .expect("the registrar calls a Windows import");
    assert_eq!(
        object.object().check_bindings(&missing_import),
        Err("encoded import bodies differ from the object's import symbols")
    );
    let mut changed_import = bindings.clone();
    let mut exchanged = import;
    exchanged.identity = machine_code::import_code::ImportCodeIdentity::Normalized(
        target::normalize_foreign_locator(
            target::ForeignLocatorCandidate::PeByName {
                library: b"other.dll".to_vec(),
                export: b"same_registrar".to_vec(),
            },
            target::TargetProfile::WindowsX64,
        )
        .unwrap()
        .into(),
    );
    *Arc::make_mut(&mut changed_import.imports)
        .imports
        .last_mut()
        .unwrap() = exchanged;
    assert_eq!(
        object.object().check_bindings(&changed_import),
        Err("encoded import bodies differ from the object's import symbols")
    );
    let mut missing_private = bindings.clone();
    assert_eq!(
        bindings.private_references.as_ref(),
        &[fixture.occurrences[0].callback_thunk_identity()]
    );
    missing_private.private_references = Arc::from([]);
    assert_eq!(
        object.object().check_bindings(&missing_private),
        Err("object publication changed the closed physical program's private references")
    );
    if let Some(retained) = object.retained_fragment_source() {
        let issued = retained.published_object();
        let sections = issued.sections();
        let callback = fixture.occurrences[0].callback_thunk_identity();
        let (symbol, declaration) =
            object_file::object_function_reference(&sections.object, callback)
                .expect("the body object already declares the callback before composition");
        assert_eq!(declaration.section, object_file::SymbolSection::None);
        let source = retained.publication();
        let inspect = |relocations: &object_file::RelocationPlan| {
            object_file::body_object::imports::readback(
                &source,
                &sections.object,
                relocations,
                &sections.functions,
            )
        };
        assert_eq!(
            inspect(&sections.relocations).unwrap(),
            sections.foreign_calls
        );
        let (field, _) = sections
            .relocations
            .records()
            .find(|(_, row)| row.symbol_handle == symbol)
            .expect("the body materializes its callback address");
        let mut redirected = sections.relocations.clone();
        let registrar = sections
            .object
            .layout
            .normalized_imports
            .first()
            .unwrap()
            .symbol;
        redirected.records.get_mut(field).symbol_handle = registrar;
        assert!(matches!(
            inspect(&redirected),
            Err(object_file::body_object::Error::Mismatch(
                "fragment import callback relocation differs"
            ))
        ));
        assert_eq!(sections.relocations, *object.relocations());
        assert_eq!(sections.text_bytes, object.text_bytes());
        assert_eq!(sections.foreign_calls, object.foreign_calls());
        assert!(sections.object.check_bindings(&missing_private).is_err());
        assert!(sections.object.check_bindings(&changed_import).is_err());
    }
    let calls = &object.foreign_call_plans().calls;
    assert!(
        !calls.is_empty(),
        "fixture must exercise real foreign transport"
    );
    let reconstructed = calls
        .iter()
        .map(|call| image_emission::place_foreign_call(call, object.publication().text()).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(reconstructed, object.foreign_calls());
    let mut changed = calls[0].clone();
    changed.spans.push(
        machine_code::foreign_call_plans::InstructionSpanObligation::NonEmpty(
            selected_instructions::SelectedInstructionId(u32::MAX),
        ),
    );
    assert!(
        image_emission::place_foreign_call(&changed, object.publication().text()).is_err(),
        "foreign evidence cannot omit a demanded instruction span"
    );
    let mut changed = calls[0].clone();
    changed.record.instruction = selected_instructions::SelectedInstructionId(u32::MAX);
    assert!(
        image_emission::place_foreign_call(&changed, object.publication().text()).is_err(),
        "a foreign contract cannot be attached to another instruction"
    );
    let program_context = Arc::clone(providers.program_provider_context());
    let body_request = NativeAdmissionRequest {
        entry: TargetMachineCallable::Machine(MachineId::new(2).unwrap()),
        providers: ProviderAdmissionRequest {
            native_bindings: &Default::default(),
            program_provider_context: Some(&program_context),
            ..request.providers
        },
        ..request
    };
    let input = VerifiedNativeArtifactInput::prepare_with_private_entry_ram(
        &super::authorize_fixture(artifact),
        target::TargetProfile::WindowsX64,
        body_request.entry,
        &[],
        &selection,
    )
    .expect("prepare callback body under the same retained program");
    let admitted = admit_prepared_native_realization(input, artifact, &body_request)
        .expect("admit actual callback body");
    let mut planned_entry = None;
    let (body, _) = super::emit_body_inspecting_physical(
        admitted,
        retain_body_source,
        |target| {
            assert!(
                target
                    .execution_plan()
                    .plan()
                    .scalar_functions
                    .iter()
                    .any(|function| function.machine == MachineId::new(1).unwrap())
            );
            assert_eq!(
                inspect_private_body_demand(&fixture.occurrences, &[target, target]).unwrap(),
                [0],
                "this full-module fixture retains the registrar even when the callback is the entry"
            );
        },
        |physical| {
            let callback = fixture.occurrences[0].callback_thunk_identity();
            assert_eq!(
                physical.stage.program().private_references.as_slice(),
                &[callback]
            );
            super::check_missing_framed_reference(physical, callback);
            planned_entry = Some(
                entry_planning::callable_entry::plan_private_function_entry(
                    fixture.occurrences[0].callback_thunk_identity(),
                    &physical.stage.program().frame,
                    physical.stage.installations().for_entry_planning(),
                    fixture.occurrences[0].boundary_entry_plan(),
                    &[],
                    None,
                )
                .expect("plan callback entry before producing any bytes"),
            );
        },
    );
    let callback_originals = body.originals;
    let body = body.projection;
    let originals = if retain_body_source {
        vec![(
            fixture.occurrences[0].callback_thunk_identity(),
            native_artifact::PrivatePhysicalOriginal::new(
                body.object().psi(),
                artifact.retain_code(),
                coverage.clone(),
            )
            .unwrap(),
        )]
    } else {
        Vec::new()
    };
    let physical_originals = native_artifact::NativePhysicalOriginals::new(originals).unwrap();
    let entry = native_encoding::callable_entry::encode_private_function_entry(
        installation_binding::entries::bind_private_entry(
            planned_entry.expect("complete callback entry plan"),
        ),
    )
    .expect("encode the closed callback entry plan");
    let entry = object_file::PrivateEntryObject::from_layout(
        placement::entry_layout::resolve_private_entry(entry).unwrap(),
    );
    let private = image_emission::bind_private_function_entry(entry, Arc::new(body.into_object()))
        .expect("bind the encoded entry to its body");
    let published = native_artifact::publish_fragment_object(
        root,
        native_artifact::FragmentObjectPublicationRequest {
            runtime: None,
            physical_entry: None,
            hosted_entry: None,
            program_entry: None,
            private_functions: &[private],
        },
    )
    .expect("link demanded callback object, not an injected unused body");
    if retain_body_source {
        let callback_originals = callback_originals.expect("requested callback audit");
        let mismatch = published_object_verification::verify_body_selected_optimization(
            published.object(),
            &callback_originals,
        )
        .unwrap_err();
        assert!(
            mismatch.to_string().contains("another object"),
            "{mismatch}"
        );
        let archive = published_object_verification::NativeStageReplay::from_bodies(
            root_originals,
            vec![(
                fixture.occurrences[0].callback_thunk_identity(),
                callback_originals,
            )],
        )
        .unwrap();
        check_layout_fixup_identity(&archive.root().unwrap().baseline_layout);
        published_object_verification::verify_retained_selected_optimization(
            published.object(),
            &archive,
        )
        .unwrap();
    }
    let image_providers =
        native_artifact::NativeImageProviders::from_admission(providers, &fixture.selected)
            .expect("retain provider evidence for the admitted root");
    let requested = native_artifact::ProgramImageInput {
        components: Vec::new(),
        terminal: Arc::clone(artifact),
        published,
        image: image_emission::ExecutableImageEmissionRequest::direct(3),
        providers: image_providers,
        coverage: Some(coverage),
        physical_originals,
    }
    .emit()
    .expect("publish physical evidence over root and relocated private body");
    let (requested, runtime) = requested.into_parts();
    assert!(
        runtime.is_none(),
        "this fixture issued an object without a runtime entry"
    );
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
