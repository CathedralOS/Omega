// Exercise publication from actual staged native code, rather than fabricating
// a physical-evidence record. Source admission is tested separately: this starts
// from a canonical Psi artifact and does not establish standard-library support.

use std::sync::Arc;

use boundary_applications::{
    TerminalBoundaryApplicationCoverage, TerminalBoundaryApplicationRealizations,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, StructuralTypeId, ValueId,
};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::TerminalBoundaryApplicationDemands;
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
};

mod callback;

mod code_layout;

fn check_missing_framed_reference(
    physical: &crate::driver::compile_body::PreparedPhysicalBody,
    callback: function_identity::MachineFunctionIdentity,
) {
    let admitted = physical_program::NativeBodyFacts::retain_selected_body_facts(
        physical_program::BodyPublication::new(physical.stage.facts().semantic.clone(), None)
            .unwrap(),
        vec![callback],
        physical.stage.installations().as_ref().clone(),
    );
    assert_eq!(
        admitted.into_framed_body_facts(&[]).unwrap_err(),
        "physical selection changed the closed native private-body references",
    );
}

fn authorize_fixture(
    artifact: &CanonicalTerminalArtifact,
) -> psi_authorization::AuthorizedTerminalArtifact {
    let producer = psi_authorization::HostProducerTrust::accept("controlled publication fixture");
    psi_authorization::authorize_terminal_artifact(Arc::new(artifact.clone()), Some(&producer))
        .unwrap()
}

fn emit_body<'execution>(
    admitted: artifact_lowering::AdmittedNativeRealization<'execution>,
    retain_native_stage_inputs: bool,
    inspect_target: impl FnOnce(&target_lowering::NativeBodyPlan),
) -> (
    crate::driver::compile_body::BodyObject,
    provider_admission::AdmittedNativeProviders<'execution>,
) {
    emit_body_inspecting_physical(admitted, retain_native_stage_inputs, inspect_target, |_| {})
}

fn emit_body_inspecting_physical<'execution>(
    mut admitted: artifact_lowering::AdmittedNativeRealization<'execution>,
    retain_native_stage_inputs: bool,
    inspect_target: impl FnOnce(&target_lowering::NativeBodyPlan),
    inspect_physical: impl FnOnce(&crate::driver::compile_body::PreparedPhysicalBody),
) -> (
    crate::driver::compile_body::BodyObject,
    provider_admission::AdmittedNativeProviders<'execution>,
) {
    let profile = target::TargetProfile::WindowsX64;
    let observation = crate::driver::observations::NativePipelineObservation::new(
        profile,
        crate::driver::observations::NativePipelineUnit::Application,
    );
    let mut target = crate::driver::compile_body::plan_native_body(
        admitted.input,
        target_lowering::OptimizedTargetLoweringRequest {
            target: profile.native_target(),
            settlements: &admitted.providers.settlements,
            installation: admitted.providers.installation.take(),
        },
        &optimization_core::PostTerminalOptimizationSelections::default(),
        optimization_core::OptimizationWorkBudget::new(10000, 10000, 10000, 10000, 10000).unwrap(),
        &observation,
        true, // This fixture independently inspects the optimized input.
    )
    .unwrap_or_else(|error| match error {
        crate::driver::compile_body::NativeBodyPlanningError::AbstractInput(error) => {
            panic!("constant abstract input: {error:?}")
        }
        crate::driver::compile_body::NativeBodyPlanningError::Target(error) => {
            panic!("constant target lowering: {error:?}")
        }
    });
    inspect_target(&target);
    if !retain_native_stage_inputs {
        target.release_original_for_audit();
    }
    let physical = crate::driver::compile_body::prepare_physical_body(
        target,
        None,
        Vec::new(),
        &observation,
    )
    .unwrap_or_else(|error| match error {
        crate::driver::compile_body::BodyEmissionError::Physical(error) => {
            panic!("constant emission: {error}")
        }
        crate::driver::compile_body::BodyEmissionError::Installation(error) => {
            panic!("constant installation: {error}")
        }
        crate::driver::compile_body::BodyEmissionError::Object(errors) => {
            panic!("constant object: {errors:?}")
        }
    });
    inspect_physical(&physical);
    let mut other_physical = physical.stage.program().frame.instructions.clone();
    let mut machine_identity = other_physical.machine.identity.bytes();
    machine_identity[0] ^= 1;
    Arc::make_mut(&mut other_physical.machine).identity =
        post_allocation_machine::PostAllocationMachineIdentity::from_bytes(machine_identity);
    assert_eq!(
        installation_admission::BoundInstallationAuthority::verify(
            &other_physical,
            &physical.stage.program().frame.frame,
            Arc::clone(physical.stage.installations()),
            Vec::new(),
            None,
        ),
        Err("installation resources differ from the final physical program, target or entry"),
    );
    let planned_cpu = entry_planning::body_cpu::plan(
        &physical.stage.program().frame,
        physical.stage.program().frame.instructions.entry,
        calling_conventions::Preemption::NotApplicable,
    );
    let bound = physical
        .bind(
            &entry_planning::installation_binding::RetainedHandlerMeasurements::default(),
            None,
        )
        .unwrap();
    let mut other_program = bound.stage.program().frame.instructions.identity;
    other_program[0] ^= 1;
    let frame_plan = bound.stage.program().frame.frame.shared_plan();
    assert!(bound.stage.installations().matches_frame(
        bound.stage.program().frame.instructions.identity,
        &frame_plan
    ));
    assert!(
        !bound
            .stage
            .installations()
            .matches_frame(other_program, &frame_plan)
    );
    // A separately built copy of the same plan still matches; a changed plan does not.
    assert!(bound.stage.installations().matches_frame(
        bound.stage.program().frame.instructions.identity,
        &Arc::new((*frame_plan).clone())
    ));
    assert!(!bound.stage.installations().matches_frame(
        bound.stage.program().frame.instructions.identity,
        &other_frame_plan(&frame_plan),
    ));
    let audit_frame = retain_native_stage_inputs.then(|| bound.stage.program().frame.frame.clone());
    let (original, projection) = crate::driver::compile_body::encode_physical_body(
        bound,
        &observation,
    )
    .and_then(|encoded| {
        let original = encoded.original_encoding_weak();
        let body = crate::driver::compile_body::layout_encoded_body(
            encoded,
            &observation,
        )?;
        assert_eq!(
            original.upgrade().is_some(),
            retain_native_stage_inputs,
            "ordinary placement releases the encoded instruction program before building an object"
        );
        let projection = crate::driver::compile_body::construct_body_object(body)?;
        Ok((original, projection))
    })
    .unwrap_or_else(|error| match error {
        crate::driver::compile_body::BodyEmissionError::Physical(error) => {
            panic!("constant encoding: {error}")
        }
        crate::driver::compile_body::BodyEmissionError::Installation(error) => {
            panic!("constant installation: {error}")
        }
        crate::driver::compile_body::BodyEmissionError::Object(errors) => {
            panic!("constant publication: {errors:?}")
        }
    });
    assert_eq!(
        original.upgrade().is_some(),
        retain_native_stage_inputs,
        "ordinary object publication releases the encoded instruction program"
    );
    if let Some(frame) = audit_frame {
        let encoding = &projection.originals.as_ref().unwrap().encoding;
        let check = |protocol: &machine_code::TargetFrameProtocolEncodingPlan| {
            encoding_admission::frame_protocol::validate_target_frame_protocol_encoding(
                &frame,
                &encoding.context.environment,
                protocol,
            )
        };
        assert_eq!(check(&encoding.protocol), Ok(encoding.protocol_identity));
        let mut changed = (*encoding.protocol).clone();
        if let Some(byte) = changed.bytes.first_mut() {
            *byte ^= 1;
        } else {
            changed.bytes.push(0);
        }
        assert!(check(&changed).is_err());
    }
    let decoded_cpu = image_emission::observe_native_body_cpu_state(
        projection.projection.object(),
        calling_conventions::Preemption::NotApplicable,
    );
    match (planned_cpu, decoded_cpu) {
        (Ok(planned), Ok(decoded)) => assert_eq!(
            planned.observations(),
            decoded.observations(),
            "physical CPU facts must agree with independent encoded readback"
        ),
        (Err(planned), Err(decoded)) => assert_eq!(
            planned.to_string(),
            decoded.to_string(),
            "physical and encoded refusal must agree"
        ),
        (planned, decoded) => {
            panic!("CPU planning changed availability: planned={planned:?}, decoded={decoded:?}")
        }
    }
    (projection, admitted.providers)
}

fn reject_changed_entry_literal(
    target: &verified_optimization_input::publication::target_replay::TargetOperationsForReplay,
    entry: MachineId,
) {
    // Keep the compiled operation and result paired with their optimized
    // source, and change just the value in a copy of the target plan.
    let mut mismatched = target.working_program_for_validation().clone();
    let function = mismatched
        .functions
        .iter_mut()
        .find(|function| function.machine == entry)
        .expect("compiled entry has a target function");
    let operation = OperationId::new(1).unwrap();
    let value = function
        .graph
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find_map(|instruction| match &mut instruction.kind {
            target_operations::TargetUnitOperation::IntegerConstant {
                psi_operation,
                value,
                ..
            } if *psi_operation == operation => Some(value),
            _ => None,
        })
        .expect("compiled entry retained its original literal");
    assert_eq!(*value, IntegerValue::Unsigned(7));
    *value = IntegerValue::Unsigned(8);

    let optimized = target.optimized();
    let error = target_lowering::replay::verify_target_operation_plan_with_closed_module(
        &mismatched,
        optimized.plan(),
        optimized.unit(),
        target.boundary_origins(),
        target.lowering_facts().installed_interrupts(),
        target.lowering_facts().closed_module(),
    )
    .expect_err("a retained target literal must agree with the compiled source");
    assert!(
        matches!(error,
            target_lowering::replay::TargetOperationAuditError::SourceCustodyMismatch {
                machine: Some(rejected_machine), ..
            } if rejected_machine == entry),
        "{error:?}"
    );
}

/// The decisive checks on the plan stage 02 published for a compiled program.
///
/// First the plan is selected with nothing but itself, its bound installations
/// and the register model selected by the plan’s target. The entry takes no source module,
/// abstract program or optimization-unit table, so none can be consulted.
/// Then separate corruptions are each refused by the check that owns them: a
/// changed value type, a width the module does not retain and a dropped
/// proof record by stage 02's validator, and installations admitted for another program by the binding
/// check in front of selection.
fn check_published_plan(body: &target_lowering::NativeBodyPlan, entry: MachineId) {
    use instruction_selection::OptimizedSelectionPipelineError;
    use instruction_selection::test_support::{
        BoundInstallations, InstallationBindingError,
        select_instructions_for_plan,
    };
    use legalized_operations::PreparedExecutionPlan;
    use semantic_vocabulary::{ClosedIntegerType, ClosedScalarType};
    use target_lowering::replay::TargetOperationAuditError::PublishedPlanMismatch;

    let target = body
        .original_for_audit()
        .unwrap()
        .target_for_correspondence();
    let verify = |prepared: &PreparedExecutionPlan| {
        target_lowering::replay::verify_execution_plan(
            prepared,
            target.working_program_for_validation(),
            target.optimized(),
            target.lowering_facts(),
        )
    };
    let published = target.execution_plan();
    verify(published).expect("the published plan equals its independent derivation");

    let authority = instruction_selection::test_support::InstallationRequirements {
        interrupts: body.semantic_facts().admission.installed_interrupts(),
        callbacks: body.semantic_facts().admission.native_callbacks(),
        entry_execution: None,
        stack: Vec::new(),
    }
    .bind(body.execution_plan())
    .unwrap();
    let installations = authority.facts().as_ref().clone();
    let selected = select_instructions_for_plan(published, &installations)
        .expect("selection needs only the plan, its installations and the registers");
    assert_eq!(
        selected.plan().contracts.borrowed_storage_layouts,
        published.plan().borrowed_storage_layouts
    );
    assert!(
        std::ptr::eq(
            selected
                .plan()
                .contracts
                .execution()
                .unwrap()
                .callable_invocations(),
            published.callable_invocations(),
        ),
        "selection shares preparation's actual query rows"
    );
    assert!(
        selected
            .plan()
            .functions
            .iter()
            .any(|function| function.machine == entry),
        "the entry was selected"
    );

    let reseal = |plan| {
        PreparedExecutionPlan::seal(plan, published.receipt().validator())
            .expect("the proposed plan still carries its prepared execution facts")
    };
    let mut missing_context = published.plan().clone();
    missing_context.execution = None;
    assert_eq!(
        PreparedExecutionPlan::seal(missing_context, published.receipt().validator()).unwrap_err(),
        "execution plan has no prepared callable facts"
    );
    let original_facts = published.execution_facts();
    let mut changed_reach = original_facts.callable_invocations().clone();
    let foreign_machine = MachineId::new(4096).unwrap();
    assert!(
        changed_reach
            .reached_machines
            .iter()
            .all(|machine| *machine < foreign_machine)
    );
    changed_reach.reached_machines.push(foreign_machine);
    let mut foreign_roots = original_facts.roots();
    foreign_roots.pre_physical =
        optimization_core::PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(
            b"foreign prephysical root",
        );
    for facts in [
        target_operations::NativeExecutionFacts::seal(
            original_facts.roots(),
            Arc::new(changed_reach),
            original_facts.entry_callable_invocations().to_vec(),
        )
        .unwrap(),
        target_operations::NativeExecutionFacts::seal(
            foreign_roots,
            Arc::new(original_facts.callable_invocations().clone()),
            original_facts.entry_callable_invocations().to_vec(),
        )
        .unwrap(),
    ] {
        let mut changed = published.plan().clone();
        changed.execution = Some(facts);
        let changed = reseal(changed);
        assert!(
            matches!(
                verify(&changed),
                Err(PublishedPlanMismatch {
                    machine: None,
                    part: "prepared callable invocation facts differ from their source",
                })
            ),
            "resealed query output and preparation roots need independent correspondence"
        );
    }
    let mut changed_abi = published.plan().clone();
    std::sync::Arc::make_mut(&mut changed_abi.function_abis)
        .first_mut()
        .expect("the prepared program has function ABI rows")
        .machine = MachineId::new(4096).unwrap();
    assert!(
        matches!(
            verify(&reseal(changed_abi)),
            Err(PublishedPlanMismatch {
                machine: None,
                part: "function ABI rows differ from the target's",
            })
        ),
        "a coherently resealed foreign function ABI is refused by correspondence"
    );
    let mut changed_geometry = published.plan().clone();
    changed_geometry.borrowed_storage_layouts =
        runtime_abi::NativeBorrowedStorageLayouts::seal(vec![
            runtime_abi::NativeBorrowedStorageLayout::new(
                StructuralTypeId::new(4096).unwrap(),
                8,
                8,
                [],
                [],
                [],
            ),
        ])
        .unwrap();
    assert!(
        matches!(
            verify(&reseal(changed_geometry)),
            Err(PublishedPlanMismatch {
                machine: None,
                part: "borrowed storage geometry differs from the target's",
            })
        ),
        "a coherently resealed foreign geometry roster is refused"
    );
    let entry_function = |plan: &mut legalized_operations::LegalizedOperationPlan| {
        plan.scalar_functions
            .iter()
            .position(|function| function.machine == entry)
            .expect("the plan holds the entry")
    };

    // A value's concrete type is part of the plan: one changed to another
    // type the module closes (every module closes Boolean) differs from the
    // derivation.
    let mut narrowed = published.plan().clone();
    let index = entry_function(&mut narrowed);
    let result = narrowed.scalar_functions[index]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| instruction.result.as_mut())
        .expect("the entry defines a value");
    let wide =
        ClosedScalarType::Integer(ClosedIntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    assert_eq!(result.scalar_type, wide);
    result.scalar_type = ClosedScalarType::Boolean;
    assert!(
        matches!(
            verify(&reseal(narrowed)),
            Err(PublishedPlanMismatch { machine: Some(machine), part: "a block, instruction or terminator" })
                if machine == entry
        ),
        "a changed value type is refused"
    );

    // Preparation closed every type against the accepted module, so a
    // resolved width that module never retained is refused before the plan is
    // compared with its derivation.
    let mut foreign = published.plan().clone();
    let index = entry_function(&mut foreign);
    let result = foreign.scalar_functions[index]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| instruction.result.as_mut())
        .expect("the entry defines a value");
    result.scalar_type =
        ClosedScalarType::Integer(ClosedIntegerType::new(IntegerSign::Unsigned, 24).unwrap());
    assert!(
        matches!(
            verify(&reseal(foreign)),
            Err(PublishedPlanMismatch { machine: Some(machine), part: "a closed type the module does not retain" })
                if machine == entry
        ),
        "a resolved width the module does not retain is refused"
    );

    // The plan lists the installations that survived lowering; one the target
    // program does not contain is refused.
    let mut installed = published.plan().clone();
    installed
        .installation_operations
        .push((entry, OperationId::new(4096).unwrap()));
    assert!(matches!(
        verify(&reseal(installed)),
        Err(PublishedPlanMismatch {
            machine: None,
            part: "the surviving installations differ from the target's"
        })
    ));

    // Installations admitted for another program cannot be bound to this plan.
    let mut other_program = published.plan().psi;
    other_program.program_fingerprint = terminal_psi::SemanticFingerprint::from_bytes([9; 32]);
    let foreign = BoundInstallations::empty(other_program);
    let error = select_instructions_for_plan(published, &foreign)
        .expect_err("a foreign interrupt cohort is refused");
    assert!(matches!(
        error,
        OptimizedSelectionPipelineError::Binding(InstallationBindingError::InterruptCohortProgram)
    ));

    // An installed entry admitted for another machine cannot be bound either.
    let other_entry = BoundInstallations {
        installed_entry: Some(bound_installations::BoundInstalledEntry {
            psi: published.plan().psi,
            target: published.plan().target,
            machine: MachineId::new(entry.get() + 1).unwrap(),
        }),
        ..installations.clone()
    };
    let error = select_instructions_for_plan(published, &other_entry)
        .expect_err("an installed entry for another machine is refused");
    assert!(matches!(
        error,
        OptimizedSelectionPipelineError::Binding(InstallationBindingError::EntryExecution)
    ));
}

fn constant_artifact(through_boundary: bool) -> Arc<CanonicalTerminalArtifact> {
    let entry = MachineId::new(1).unwrap();
    let result = ValueDeclaration {
        id: ValueId::new(1).unwrap(),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        qualifications: Default::default(),
    };
    let block = BlockId::new(1).unwrap();
    let mut module = TerminalModule::for_entry(entry);
    module.machines.push(TerminalMachine {
        id: entry,
        attachment: None,
        structural_places: Vec::new(),
        call_formations: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        entry: block,
        blocks: vec![Block {
            id: block,
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![Operation::unsuspended(
                OperationId::new(1).unwrap(),
                OperationResult::Scalar(result),
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(7),
                },
            )],
            terminator: Terminator::Return {
                edge: EdgeId::new(1).unwrap(),
                value: result.id,
                cleanup_actions: Vec::new(),
            },
        }],
        ranked_scc: None,
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        contract: MachineContract {
            result: TerminalMachineResult::Scalar(result),
            ..MachineContract::empty(ContractId::new(1).unwrap())
        },
        assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
    });
    if through_boundary {
        let boundary = semantic_vocabulary::BoundaryMachineId::new(1).unwrap();
        let mut callee = module.machines[0].clone();
        callee.id = MachineId::new(2).unwrap();
        callee.contract.id = ContractId::new(2).unwrap();
        callee.blocks[0].operations[0].id = OperationId::new(2).unwrap();
        module.machines[0].blocks[0].operations[0].kind = OperationKind::BoundaryCall {
            boundary,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
            requirement_obligations: Vec::new(),
        };
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let name = identities.intern("PhysicalFixture::value");
        module.identities = identities.build();
        module
            .interfaces.boundary_machines
            .push(terminal_psi::BoundaryMachineDeclaration {
                id: boundary,
                identity: name,
                owner: terminal_psi::NominalDeclarationIdentity::new(name),
                role: terminal_psi::BoundaryDeclarationRole::Callable,
                kind: terminal_psi::BoundaryRequirementKind::Machine,
                completion: terminal_psi::BoundaryCompletion::NormalReturn,
                unconditional_termination: true,
                declaration: None,
                numeric_intrinsic: None,
                attachment: None,
                membership_ensures: Vec::new(),
                normal_qualifications: Vec::new(),
                scalar_domain_requirements: Vec::new(),
                parameter_order: Vec::new(),
                scalar_parameters: Vec::new(),
                crash_routes: Vec::new(),
                structural_parameters: Vec::new(),
                result: terminal_psi::BoundaryMachineResult::Scalar(result.scalar_type.into()),
                requires: Vec::new(),
                scalar_requires: Vec::new(),
                scalar_ensures: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                result_reference_regions: Vec::new(),
                result_lifetimes: None,
                reference_outputs: Vec::new(),
                fixed_service_reach: Vec::new(),
                service_reach_is_installation_bound: false,
                owning_service: None,
                published_synchronous_entries: Vec::new(),
                published_service_ceiling: Vec::new(),
            });
        let mut identities = module.identities.clone().into_builder();
        let provider = identities.intern("PhysicalValueProvider");
        let declaration = identities.intern("PhysicalValueMap");
        let candidate = identities.intern("PhysicalValueProvider::value");
        module
            .interfaces.provider_candidates
            .push(terminal_psi::ProviderCandidateConformance {
                boundary,
                requirement_identity: name,
                source_requirement_identity: name,
                declaration_application: terminal_psi::TerminalIdentityIndex::ABSENT,
                receiving_application: terminal_psi::TerminalIdentityIndex::ABSENT,
                receiving_region_arguments: Vec::new(),
                provider_identity: provider,
                provider_declaration: terminal_psi::NominalDeclarationIdentity::new(declaration),
                candidate_identity: candidate,
                source_candidate_identity: candidate,
                source_candidate_package_identity: None,
                candidate: callee.id,
                signature: terminal_psi::ProviderSignature::empty(),
                refinement: terminal_psi::ProviderRefinement::empty(),
            });
        module.identities = identities.build();
        module.machines.push(callee);
    }
    let optimization = terminal_codec::build_identity_optimization_execution_record(&module)
        .expect("constant module optimization identity");
    Arc::new(CanonicalTerminalArtifact::from_parts(module, optimization, None).unwrap())
}

// A checked boundary body uses the same explicit provider plan as ordinary
// native compilation. The fixture grants no alternate target-body selection.
fn checked_value_plan() -> build_providers::plans::ProviderPlan {
    use build_providers::plans::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod};
    let mut plan = ProviderPlan {
        name: "physical-value".into(),
        provider_type: "PhysicalValueProvider".into(),
        provider_declaration: Some("PhysicalValueMap".into()),
        target: "windows_x86_64".into(),
        ..ProviderPlan::default()
    };
    plan.provider_application = plan.identities.intern("PhysicalValueProvider");
    let surface = std::sync::Arc::make_mut(&mut plan.schema.declaration);
    surface.trait_name = surface.identities.intern("PhysicalFixture");
    surface.methods.push(ServiceMethod {
        name: "value".into(),
        requirement_owner: "PhysicalFixture".into(),
        requirement_identity: surface.identities.intern("PhysicalFixture::value"),
        has_result: true,
        result_type_identity: surface.identities.intern("u64"),
        terminates_guarantee: true,
        ..ServiceMethod::default()
    });
    let requirement = plan.identities.intern("PhysicalFixture::value");
    let machine = plan.identities.intern("PhysicalValueProvider::value");
    plan.rows.push(ProviderPlanRow {
        method: "value".into(),
        requirement_identity: requirement,
        declared_requirement_identity: requirement,
        calling_signature: None,
        requirement_lifetime_partition: Vec::new(),
        admitted_same_stack_bytes: 1024,
        binding: ProviderBinding::CheckedAdapter {
            machine_identity: machine,
            machine_package_identity: None,
        },
    });
    plan
}

/// The same frame plan with one root changed: a different frame for every
/// check that compares plans.
fn other_frame_plan(
    frame: &machine_code::TargetFrameLayoutPlan,
) -> Arc<machine_code::TargetFrameLayoutPlan> {
    let mut other = frame.clone();
    let mut storage = other.callee_save_storage.bytes();
    storage[0] ^= 1;
    other.callee_save_storage =
        machine_code::NonAuthoritativeCalleeSaveStorageIdentity::from_bytes(storage);
    Arc::new(other)
}

#[test]
fn object_publication_rejects_placed_code_from_a_different_program() {
    let (_, scalar, scalar_archive) = check_publication(false, true);
    let (_, boundary, boundary_archive) = check_publication(true, true);
    let scalar = scalar.unwrap();
    let boundary = boundary.unwrap();
    let object = scalar.fragment_source();
    object.verify_placement(object.source()).unwrap();
    let encoding = &scalar_archive.root().unwrap().encoding;
    let observed = object.source().encoding_subject();
    let identify = machine_code::program_context::BoundEncodingSubject::from_encoded;
    assert_eq!(
        identify(&encoding.context, Arc::clone(&encoding.frame)),
        Ok(observed.clone())
    );
    let mut other = (*encoding.context).clone();
    other.identity[0] ^= 1;
    assert_ne!(
        identify(&other, Arc::clone(&encoding.frame)).unwrap(),
        observed
    );
    let mut other = (*encoding.context).clone();
    Arc::make_mut(&mut other.machine).target = target::TargetProfile::LinuxArm64.native_target();
    assert_ne!(
        identify(&other, Arc::clone(&encoding.frame)).unwrap(),
        observed
    );
    assert_ne!(
        identify(&encoding.context, other_frame_plan(&encoding.frame)).unwrap(),
        observed
    );
    let mut other = (*encoding.context).clone();
    other.work_schedule =
        semantic_vocabulary::WorkScheduleIdentity::new(if other.work_schedule.marker() == 1 {
            2
        } else {
            1
        })
        .unwrap();
    assert!(identify(&other, Arc::clone(&encoding.frame)).is_err());
    assert!(
        function_layout::validate_optimized_fixed_frame_text_section(
            object.source(),
            boundary.fragment_source().frame_application_for_audit(),
        )
        .is_err()
    );
    let publication = object.publication();
    assert!(!publication.body().has_installed_branch_checks());
    assert!(
        publication
            .body()
            .check_installed_branches(publication.text().semantic_entry, &[])
            .unwrap_err()
            .contains("not requested")
    );
    let layout = object
        .frame_application_for_audit()
        .source()
        .source()
        .relaxed_layout();
    let mut emitted =
        image_emission::prepare_object_for_image(object.published_object()).unwrap();
    assert!(emitted.fragment_replay().is_none());
    let wrong = boundary.clone().attach_to(&mut emitted).unwrap_err();
    assert!(
        wrong
            .to_string()
            .contains("retained object belongs to another publication")
    );
    assert!(emitted.fragment_replay().is_none());
    scalar.clone().attach_to(&mut emitted).unwrap();
    assert!(emitted.fragment_replay().is_some());
    assert!(scalar_archive.root().unwrap().belongs_to(layout.current()));
    assert!(
        !boundary_archive
            .root()
            .unwrap()
            .belongs_to(layout.current())
    );
    assert!(matches!(
        object.verify_placement(boundary.fragment_source().source()),
        Err(object_file::RelocationFreeObjectContainerError::PublicationMismatch)
    ));
}

#[test]
fn physical_publication_rejoins_emitted_object_and_receiver() {
    for through_boundary in [false, true] {
        let (ordinary, _, _) = check_publication(through_boundary, false);
        let (audited, originals, archive) = check_publication(through_boundary, true);
        assert_eq!(
            ordinary, audited,
            "retaining originals changes no artifact identity"
        );
        if !through_boundary {
            let first = originals.expect("explicitly retained target");
            let (again, second, second_archive) = check_publication(through_boundary, true);
            let second = second.expect("separately retained target");
            assert_eq!(again, audited);
            let mut emitted = image_emission::prepare_object_for_image(
                first.fragment_source().published_object(),
            )
            .unwrap();
            assert!(second.clone().attach_to(&mut emitted).is_err());
            first.clone().attach_to(&mut emitted).unwrap();
            let error = published_object_verification::verify_retained_fixed_frame(
                &emitted,
                &second_archive,
            )
            .unwrap_err();
            assert!(error.to_string().contains("another object"), "{error}");
            let mut layout = archive.root().unwrap().baseline_layout.clone();
            assert!(!layout.control_alternatives.is_empty());
            layout.control_alternatives = Arc::from([]);
            assert_eq!(
                encoding_admission::layout::validate_current_resolved_layout(&layout),
                Err(encoding_admission::layout::OptimizedResolvedSelectedFormLayoutError::RootMismatch)
            );
            let original = first.target_source().unwrap();
            assert!(original.belongs_to(&first.fragment_source().emission_facts().semantic));
            assert!(!original.belongs_to(&second.fragment_source().emission_facts().semantic));
            let mut changed = first.fragment_source().emission_facts().semantic.clone();
            changed.program.functions.clear();
            assert!(!original.belongs_to(&changed));
        }
    }
}

fn check_publication(
    through_boundary: bool,
    retain_native_stage_inputs: bool,
) -> (
    native_artifact::NativeArtifactIdentity,
    Option<image_emission::FragmentReplay>,
    published_object_verification::NativeStageReplay,
) {
    check_canonical_publication(
        constant_artifact(through_boundary),
        through_boundary,
        retain_native_stage_inputs,
        |target, entry| {
            check_published_plan(target, entry);
            if !through_boundary {
                reject_changed_entry_literal(
                    target
                        .original_for_audit()
                        .unwrap()
                        .target_for_correspondence(),
                    entry,
                );
            }
        },
        |_| {},
    )
}

fn check_canonical_publication(
    artifact: Arc<CanonicalTerminalArtifact>,
    through_boundary: bool,
    retain_native_stage_inputs: bool,
    inspect_target: impl FnOnce(&target_lowering::NativeBodyPlan, MachineId),
    inspect_image: impl FnOnce(&native_artifact::RetainedNativeArtifact),
) -> (
    native_artifact::NativeArtifactIdentity,
    Option<image_emission::FragmentReplay>,
    published_object_verification::NativeStageReplay,
) {
    use provider_admission::{
        ProviderAdmissionRequest, current_terminal_authority_policy,
    };
    let entry = artifact.semantic_module().entry.unwrap();
    let terminal_psi::TargetMachineCallable::Machine(bootstrap_entry) = entry else {
        panic!("fixture has a checked root");
    };
    let demands =
        TerminalBoundaryApplicationDemands::new(artifact.manifest().semantic(), Vec::new())
            .unwrap();
    let realizations = TerminalBoundaryApplicationRealizations::new(&demands, Vec::new()).unwrap();
    let coverage = TerminalBoundaryApplicationCoverage::new(demands, realizations).unwrap();
    let selected = build_providers::selections::SelectedProviderPlanFacts::from_selected_plans(
        if through_boundary {
            vec![checked_value_plan()]
        } else {
            Vec::new()
        },
    )
    .unwrap();
    let adapters =
        provider_admission::retain_selected_provider_adapters(&selected)
            .unwrap();
    let policy = current_terminal_authority_policy();
    let exclusions = native_behavior_exclusions::BehaviorExclusions::default();
    let profile = target::TargetProfile::WindowsX64;
    let admission = artifact_lowering::NativeAdmissionRequest {
        entry,
        private_entry_ram: &[],
        providers: ProviderAdmissionRequest {
            native_bindings: &Default::default(),
            profile,
            bootstrap_entry,
            program_provider_context: None,
            installation_assumptions: &[],
            selected_provider_plans: &selected,
            checked_adapters: &adapters,
            external_binding_rows: &[],
            settlements: &[],
            compiler_builtins: &[],
            native_callbacks: &[],
            installed_interrupts: &[],
            callback_occurrences: &[],
            installed_calling: &[],
            terminal_authority_policy: &policy,
            terminal_authority_permission_policy: &None,
            x86_scalar_fma_provider: None,
        },
        boundary_application_coverage: Some(&coverage),
        behavior_exclusions: &exclusions,
        isolated_execution_grant: None,
    };
    let admitted =
        artifact_lowering::test_support::admit_native_realization(&authorize_fixture(&artifact), &admission)
            .expect("canonical constant admits without provider authority");
    let runtime_input = admitted.input.optimization_input().runtime_input();
    assert!(std::ptr::eq(
        runtime_input.module(),
        artifact.semantic_module()
    ));
    assert!(std::ptr::eq(
        runtime_input.boundary_machines(),
        admitted.input.plan().boundary_machines.as_slice(),
    ));
    let runtime_host = crate::driver::runtime::plan_host_provisions(
        admitted
            .providers
            .installation
            .as_ref()
            .map(|installation| installation.terminal_selection()),
        Vec::new(),
        runtime_input,
        &selected,
        &[],
        crate::driver::runtime::NativeBindingRuntimeInputs {
            bindings: &Default::default(),
            calling: &[],
        },
    )
    .expect("runtime capture borrows the same admitted boundary roster");
    let capture_runtime = |artifact: &CanonicalTerminalArtifact, profile| {
        native_artifact::TerminalCallableRuntimeProvisions::admit(
            artifact.view(),
            runtime_input,
            bootstrap_entry,
            profile,
            runtime_host.clone(),
        )
    };
    let runtime = capture_runtime(&artifact, profile).expect("exact runtime input is admitted");
    assert_eq!(runtime.identity(), artifact.view().identity());
    assert_eq!(runtime.entry(), bootstrap_entry);
    assert_eq!(runtime.target(), profile);
    assert_eq!(
        capture_runtime(&artifact, target::TargetProfile::LinuxX64).unwrap_err(),
        "runtime capture names another native artifact or target",
    );
    assert_eq!(
        capture_runtime(&constant_artifact(!through_boundary), profile).unwrap_err(),
        if runtime_host.installation().is_some() {
            "callable runtime provisions name another artifact or absent entry"
        } else {
            "runtime capture names another native artifact or target"
        },
    );
    let (projection, providers) = emit_body(admitted, retain_native_stage_inputs, |target| {
        assert!(target.private_references().is_empty());
        inspect_target(target, bootstrap_entry);
    });
    let crate::driver::compile_body::BodyObject {
        projection,
        originals,
    } = projection;
    let archive =
        published_object_verification::NativeStageReplay::from_bodies(originals, Vec::new())
            .unwrap();
    let unrelated = constant_artifact(!through_boundary);
    assert!(
        native_artifact::PrivatePhysicalOriginal::new(
            projection.object().psi(),
            unrelated.retain_code(),
            coverage.clone(),
        )
        .is_err(),
        "a physical proof cannot adopt another semantic owner"
    );
    let published = native_artifact::publish_fragment_object(
        projection,
        native_artifact::FragmentObjectPublicationRequest {
            runtime: None,
            physical_entry: None,
            hosted_entry: None,
            program_entry: None,
            private_functions: &[],
        },
    )
    .expect("publish actual emitted object");
    assert_eq!(
        published.object().retained_fragment_source().is_some(),
        retain_native_stage_inputs,
        "only an explicit audit may retain compiler stages behind an emitted object"
    );
    assert_eq!(
        published
            .object()
            .native_instruction_source_rows()
            .is_some(),
        retain_native_stage_inputs,
        "ordinary image publication has no instruction graph lookup capability"
    );
    let target_audit =
        published_object_verification::verify_retained_target_operations(published.object());
    let audit =
        published_object_verification::verify_retained_fixed_frame(published.object(), &archive);
    if retain_native_stage_inputs {
        target_audit.expect("independent target audit accepts the retained stage 02 output");
        audit.expect("explicitly retained originals admit the fixed-frame correspondence audit");
    } else {
        let error = target_audit.expect_err("default publication retains no stage 02 output");
        assert!(
            error
                .to_string()
                .contains("requires retained target operations"),
            "{error}"
        );
        let error = audit.expect_err("default publication retains no original allocation graph");
        assert!(error.to_string().contains("ReplayUnavailable"), "{error}");
    }
    let originals = published.object().fragment_replay().cloned();
    let image_providers =
        native_artifact::NativeImageProviders::from_admission(providers, &selected)
            .expect("retain provider evidence for the admitted root");
    let requested = native_artifact::ProgramImageInput {
        components: Vec::new(),
        terminal: artifact,
        published,
        image: image_emission::ExecutableImageEmissionRequest::direct(3),
        providers: image_providers,
        coverage: Some(coverage),
        physical_originals: Default::default(),
    }
    .emit()
    .expect("emit native image with retained physical scope");
    let (requested, runtime) = requested.into_parts();
    assert!(
        runtime.is_none(),
        "this fixture issued an object without a runtime entry"
    );
    let native_artifact::RequestedNativeArtifact::Direct(native) = requested else {
        panic!("Windows image is direct");
    };
    let derivation = native
        .physical_evidence()
        .expect("an audit derives physical evidence from the emitted bytes");
    let native_artifact::NativePhysicalEvidenceDerivation::Complete(physical) = &derivation else {
        panic!("retained staged source establishes complete physical scope: {derivation:?}");
    };
    assert_eq!(physical.children().len(), usize::from(through_boundary));
    let receipt = native_artifact::PreparedExecutablePublication::new(&native)
        .expect("fresh emission can be published without replay reports")
        .into_receipt(std::path::PathBuf::from("published-program"))
        .expect("publication keeps consistent installation inputs");
    assert!(receipt.has_consistent_installation_identity());
    native
        .validate()
        .expect("independent receiver replays emitted code");
    inspect_image(&native);
    let mut changed = native.image().output().clone();
    changed.final_text_bytes[0] ^= 1;
    assert!(
        image_emission::validate_direct_executable_image_output(
            native.object(),
            native.image(),
            &changed,
        )
        .is_err(),
        "absent replay reports do not exempt changed bytes from receiver replay"
    );
    let identity = native.identity();
    // Emission published the bytes without deriving evidence. A receiver that
    // re-derives it from the same parts must reach the same identity.
    let replayed = native_artifact::NativeArtifact::from_replayed_parts(native.into_parts())
        .expect("receiver replays the published parts");
    assert_eq!(
        replayed.identity(),
        identity,
        "artifact identity is independent of the receiver's evidence pass"
    );
    (identity, originals, archive)
}
