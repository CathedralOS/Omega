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

mod callback;

fn emit_body<'execution>(
    mut admitted: terminal_psi_to_abstract_operations::AdmittedNativeRealization<'execution>,
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
                reference_outputs: Vec::new(),
                fixed_service_reach: Vec::new(),
                owning_service: None,
                published_synchronous_entries: Vec::new(),
                published_service_ceiling: Vec::new(),
            });
        module
            .target_machine_families
            .push(terminal_psi::TargetMachineFamily {
                canonical: terminal_psi::TargetMachineCallable::Boundary(boundary),
                head_contract: [1; 32],
                members: vec![terminal_psi::TargetMachineFamilyMember {
                    target: "windows_x86_64".into(),
                    callable: terminal_psi::TargetMachineCallable::Machine(callee.id),
                    supply: terminal_psi::TargetMachineSupply::CheckedBody,
                }],
            });
        module.machines.push(callee);
    }
    let optimization = terminal_codec::build_identity_optimization_execution_record(&module)
        .expect("constant module optimization identity");
    Arc::new(CanonicalTerminalArtifact::from_parts(module, optimization, None).unwrap())
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
    let selected = effects::SelectedProviderPlanFacts::default();
    let policy = current_terminal_authority_policy();
    let exclusions = effects::BehaviorExclusions::default();
    let profile = target::TargetProfile::WindowsX64;
    let admission = terminal_psi_to_abstract_operations::NativeAdmissionRequest {
        entry,
        private_entry_ram: &[],
        providers: ProviderAdmissionRequest {
            profile,
            bootstrap_entry,
            program_provider_context: None,
            installation_assumptions: &[],
            selected_provider_plans: &selected,
            checked_adapters: &[],
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
    let admitted =
        terminal_psi_to_abstract_operations::admit_native_realization(&artifact, &admission)
            .expect("canonical constant admits without provider authority");
    let (projection, providers) = emit_body(admitted);
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
