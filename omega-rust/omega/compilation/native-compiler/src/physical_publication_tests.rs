// Exercise publication from actual staged native code, rather than fabricating
// a physical-evidence record. Source admission is tested separately: this starts
// from a canonical Psi artifact and does not establish standard-library support.

use std::sync::Arc;

use boundary_applications::{
    TerminalBoundaryApplicationCoverage, TerminalBoundaryApplicationDemands,
    TerminalBoundaryApplicationRealizations,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
};

#[path = "physical_publication_tests_callback.rs"]
mod callback;

fn authorize_fixture(
    artifact: &CanonicalTerminalArtifact,
) -> terminal_installation::AuthorizedTerminalArtifact {
    let producer =
        terminal_installation::HostProducerTrust::accept("controlled publication fixture");
    terminal_installation::authorize_terminal_artifact(Arc::new(artifact.clone()), Some(&producer))
        .unwrap()
}

fn emit_body<'execution>(
    mut admitted: terminal_psi_to_abstract_operations::AdmittedNativeRealization<'execution>,
    inspect_target: impl FnOnce(
        &abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
    ),
) -> (
    native_artifact::FragmentObjectProjection,
    terminal_psi_to_abstract_operations::provider_admission::AdmittedNativeProviders<'execution>,
) {
    let profile = target::TargetProfile::WindowsX64;
    let observation = crate::observations::NativePipelineObservation::new(
        profile,
        crate::observations::NativePipelineUnit::Application,
    );
    let target = crate::pipeline::lower_to_target(
        admitted.input,
        abstract_operations_to_target_operations::OptimizedTargetLoweringRequest {
            target: profile.native_target(),
            settlements: &admitted.providers.settlements,
            installation: admitted.providers.installation.take(),
        },
        &optimization_core::PostTerminalOptimizationSelections::default(),
        optimization_core::OptimizationWorkBudget::new(10000, 10000, 10000, 10000, 10000).unwrap(),
        &observation,
    )
    .unwrap_or_else(|error| match error {
        crate::pipeline::TargetLoweringError::AbstractOptimization(error) => {
            panic!("constant optimization: {error:?}")
        }
        crate::pipeline::TargetLoweringError::Target(error) => {
            panic!("constant target lowering: {error:?}")
        }
    });
    inspect_target(&target);
    let projection =
        crate::pipeline::emit_target(target, &observation).unwrap_or_else(|error| match error {
            crate::pipeline::BodyEmissionError::Physical(error) => {
                panic!("constant emission: {error}")
            }
            crate::pipeline::BodyEmissionError::Object(errors) => {
                panic!("constant object: {errors:?}")
            }
        });
    (projection, admitted.providers)
}

fn reject_changed_entry_literal(
    target: &abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
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
    let error = native_preparation::verify_target_operation_plan_with_closed_module(
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
            native_preparation::TargetOperationAuditError::SourceCustodyMismatch {
                machine: Some(rejected_machine), ..
            } if rejected_machine == entry),
        "{error:?}"
    );
}

/// The decisive checks on the plan stage 02 published for a compiled program.
///
/// First the plan is selected with nothing but itself, its bound installations
/// and the register environment: the selection entry takes no source module,
/// abstract program or optimization-unit table, so none can be consulted.
/// Then three separate corruptions are each refused by the check that owns
/// them: a narrowed value type and a dropped proof record by stage 02's
/// validator, and installations admitted for another program by the binding
/// check in front of selection.
fn check_published_plan(
    target: &abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
    entry: MachineId,
) {
    use legalized_operations::{PreparedExecutionPlan, RetainedIntegerTypes};
    use native_preparation::TargetOperationAuditError::PublishedPlanMismatch;
    use semantic_vocabulary::{ClosedIntegerType, ClosedScalarType};
    use target_operations_to_selected_instructions::{
        BoundInstallations, InstallationBindingError, OptimizedSelectionPipelineError,
        select_instructions_for_plan,
    };

    let optimized = target.optimized();
    let verify = |prepared: &PreparedExecutionPlan| {
        native_preparation::verify_execution_plan(
            prepared,
            target.working_program_for_validation(),
            optimized.plan(),
            optimized.unit(),
            target.boundary_origins(),
            target.lowering_facts().installed_interrupts(),
            target.lowering_facts().closed_module(),
        )
    };
    let published = target.execution_plan();
    verify(published).expect("the published plan equals its independent derivation");

    let environment =
        register_environment::baseline_target_register_environment(target.target()).unwrap();
    let installations = BoundInstallations {
        interrupts: target.lowering_facts().installed_interrupts(),
        entry_execution: target.installed_entry_execution(),
        stack_requirements: target.installed_stack_requirements(),
    };
    let selected = select_instructions_for_plan(published, installations, &environment)
        .expect("selection needs only the plan, its installations and the registers");
    assert!(
        selected
            .plan()
            .functions
            .iter()
            .any(|function| function.machine == entry),
        "the entry was selected"
    );

    let reseal = |plan| PreparedExecutionPlan::seal(plan, published.receipt().validator());
    let entry_function = |plan: &mut legalized_operations::LegalizedOperationPlan| {
        plan.scalar_functions
            .iter()
            .position(|function| function.machine == entry)
            .expect("the plan holds the entry")
    };

    // A value's concrete type is part of the plan.
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
    result.scalar_type =
        ClosedScalarType::Integer(ClosedIntegerType::new(IntegerSign::Unsigned, 32).unwrap());
    assert!(
        matches!(
            verify(&reseal(narrowed)),
            Err(PublishedPlanMismatch { machine: Some(machine), part: "a block, instruction or terminator" })
                if machine == entry
        ),
        "a narrowed value type is refused"
    );

    // So is the record of which integer types the module retains.
    let mut unsealed = published.plan().clone();
    unsealed.retained_integers = RetainedIntegerTypes::Unsealed;
    assert!(matches!(
        verify(&reseal(unsealed)),
        Err(PublishedPlanMismatch { machine: None, .. })
    ));

    // Proof evidence stays with the function it was accepted for. This small
    // program records none, so the corruption is a record the sources lack;
    // a program that has some loses one instead. Either way the plan then
    // claims evidence its sources do not.
    let mut forged = published.plan().clone();
    let index = entry_function(&mut forged);
    let facts = &mut forged.scalar_functions[index].ownership_frontier_facts;
    if facts.pop().is_none() {
        facts.push(optimization_unit::OwnershipFrontierFact::new(
            published.plan().psi,
            entry,
            optimization_unit::OwnershipFrontierSite::BlockEntry(BlockId::new(1).unwrap()),
            optimization_unit::OwnershipFrontierSnapshot {
                claims: Vec::new(),
                owned_places: Vec::new(),
                partial_custody: Vec::new(),
            },
        ));
    }
    assert!(
        matches!(
            verify(&reseal(forged)),
            Err(PublishedPlanMismatch { machine: Some(machine), part: "ownership-frontier facts" })
                if machine == entry
        ),
        "a changed proof record is refused"
    );

    // The plan lists the installations that survived lowering; one the target
    // program does not contain is refused.
    let mut installed = published.plan().clone();
    installed
        .installation_operations
        .push(OperationId::new(4096).unwrap());
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
    let foreign = terminal_psi_to_abstract_operations::provider_admission::AdmittedInstalledInterruptCohort::empty(other_program);
    let error = select_instructions_for_plan(
        published,
        BoundInstallations {
            interrupts: &foreign,
            ..installations
        },
        &environment,
    )
    .expect_err("a foreign interrupt cohort is refused");
    assert!(matches!(
        error,
        OptimizedSelectionPipelineError::Binding(InstallationBindingError::InterruptCohortProgram)
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
        nominal_cleanup_subject: None,
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
            .boundary_machines
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
            .provider_candidates
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
fn checked_value_plan() -> effects::provider_plan::ProviderPlan {
    use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod};
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

#[test]
fn physical_publication_rejoins_emitted_object_and_receiver() {
    check_publication(false);
    check_publication(true);
}

fn check_publication(through_boundary: bool) {
    use terminal_psi_to_abstract_operations::provider_admission::{
        ProviderAdmissionRequest, current_terminal_authority_policy,
    };
    let artifact = constant_artifact(through_boundary);
    let entry = artifact.semantic_module().entry.unwrap();
    let terminal_psi::TargetMachineCallable::Machine(bootstrap_entry) = entry else {
        panic!("fixture has a checked root");
    };
    let demands =
        TerminalBoundaryApplicationDemands::new(artifact.manifest().semantic(), Vec::new())
            .unwrap();
    let realizations = TerminalBoundaryApplicationRealizations::new(&demands, Vec::new()).unwrap();
    let coverage = TerminalBoundaryApplicationCoverage::new(demands, realizations).unwrap();
    let selected = effects::SelectedProviderPlanFacts::from_selected_plans(if through_boundary {
        vec![checked_value_plan()]
    } else {
        Vec::new()
    })
    .unwrap();
    let adapters =
        terminal_psi_to_abstract_operations::provider_admission::retain_selected_provider_adapters(
            &selected,
        )
        .unwrap();
    let policy = current_terminal_authority_policy();
    let exclusions = effects::BehaviorExclusions::default();
    let profile = target::TargetProfile::WindowsX64;
    let admission = terminal_psi_to_abstract_operations::NativeAdmissionRequest {
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
    };
    let admitted = terminal_psi_to_abstract_operations::admit_native_realization(
        &authorize_fixture(&artifact),
        &admission,
    )
    .expect("canonical constant admits without provider authority");
    let (projection, providers) = emit_body(admitted, |target| {
        check_published_plan(target, bootstrap_entry);
        if !through_boundary {
            reject_changed_entry_literal(target, bootstrap_entry);
        }
    });
    let unrelated = constant_artifact(!through_boundary);
    assert!(
        projection
            .object()
            .clone()
            .with_semantic_source(unrelated.retain_code(), coverage.clone(),)
            .is_err(),
        "an emitted object cannot adopt another semantic owner"
    );
    let projection = projection
        .with_semantic_source(artifact.retain_code(), coverage.clone())
        .expect("retain the emitted body's own semantic owner");
    let published = native_artifact::publish_fragment_object(
        projection,
        native_artifact::FragmentObjectPublicationRequest {
            program_entry: None,
            private_functions: &[],
        },
    )
    .expect("publish actual emitted object");
    image_emission::verify_retained_target_operations(&published.object)
        .expect("independent target audit accepts the emitted object's retained source");
    let requested = native_artifact::RequestedNativeArtifact::emit_admitted(
        profile.native_target(),
        artifact,
        published,
        image_emission::ExecutableImageEmissionRequest::direct(3),
        &selected,
        providers,
        Some(coverage),
    )
    .expect("emit native image with retained physical scope");
    let native_artifact::RequestedNativeArtifact::Direct(native) = requested else {
        panic!("Windows image is direct");
    };
    assert!(
        native.physical_evidence().is_some(),
        "retained staged source establishes complete physical scope"
    );
    assert_eq!(
        native.physical_evidence().unwrap().children().len(),
        usize::from(through_boundary)
    );
    native
        .validate()
        .expect("independent receiver replays emitted code");
    let mut parts = native.into_parts();
    parts.physical_evidence = None;
    assert!(
        native_artifact::NativeArtifact::from_replayed_parts(parts).is_err(),
        "receiver must reject stripped complete physical evidence"
    );
}
