//! Source-free graphs test selection before lowering, including dormant work.
use super::*;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarApplicationId, ScalarType, ValueId,
};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
};

fn machine_id(value: u64) -> MachineId {
    MachineId::new(value).unwrap()
}

fn scalar_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

fn machine(identity: u64, kind: OperationKind) -> TerminalMachine {
    let result = ValueDeclaration {
        id: ValueId::new(1).unwrap(),
        scalar_type: scalar_type(),
        qualifications: Default::default(),
    };
    TerminalMachine {
        id: machine_id(identity),
        attachment: None,
        declaration: None,
        nominal_cleanup_subject: None,
        parameters: Vec::new(),
        parameter_order: Vec::new(),
        structural_parameters: Vec::new(),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_entry_claims: Vec::new(),
        result: TerminalMachineResult::Scalar(result.clone()),
        entry: BlockId::new(1).unwrap(),
        blocks: vec![Block {
            id: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![Operation::unsuspended(
                OperationId::new(1).unwrap(),
                OperationResult::Scalar(result.clone()),
                kind,
            )],
            terminator: Terminator::Return {
                edge: EdgeId::new(1).unwrap(),
                value: result.id,
                cleanup_actions: Vec::new(),
            },
        }],
        ranked_scc: None,
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        contract: MachineContract::empty(ContractId::new(identity).unwrap()),
        assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
    }
}

fn call(callee: u64) -> OperationKind {
    OperationKind::Call {
        callee: machine_id(callee),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    }
}

fn graph() -> TerminalModule {
    let mut module = TerminalModule::for_entry(machine_id(1));
    let application = module
        .scalar_applications
        .insert(terminal_psi::ScalarApplication::Evaluate {
            entry: terminal_psi::TargetMachineCallable::Machine(machine_id(1)),
            scalar_type: scalar_type(),
        })
        .unwrap();
    module.machines = vec![
        machine(
            1,
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(7),
            },
        ),
        machine(2, call(3)),
        machine(3, OperationKind::ReadScalarApplication { application }),
    ];
    module
}

fn prepare(module: TerminalModule) -> Result<VerifiedNativeArtifactInput, ArtifactLoweringError> {
    prepare_at(module, target::TargetProfile::MacosArm64)
}

fn prepare_at(
    module: TerminalModule,
    profile: target::TargetProfile,
) -> Result<VerifiedNativeArtifactInput, ArtifactLoweringError> {
    let record = terminal_codec::build_identity_optimization_execution_record(&module).unwrap();
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_parts(module, record, None).unwrap();
    let reloaded =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    VerifiedNativeArtifactInput::prepare(
        &reloaded,
        profile,
        reloaded
            .semantic_module()
            .entry
            .expect("fixture declares an entry"),
        &[],
        &NativeExecutionSelection {
            scalar_applications: None,
            providers: &[],
            plans: &effects::SelectedProviderPlanFacts::default(),
            callbacks: &[],
            entry_call: None,
        },
    )
}

#[test]
fn native_scope_preserves_target_member_exclusion() {
    let mut module = graph();
    let callable = |identity| terminal_psi::TargetMachineCallable::Machine(machine_id(identity));
    module
        .target_machine_families
        .push(terminal_psi::TargetMachineFamily {
            canonical: callable(1),
            head_contract: [1; 32],
            members: vec![
                terminal_psi::TargetMachineFamilyMember {
                    target: "macos_arm64".into(),
                    callable: callable(1),
                    supply: terminal_psi::TargetMachineSupply::CheckedBody,
                },
                terminal_psi::TargetMachineFamilyMember {
                    target: "windows_x86_64".into(),
                    callable: callable(2),
                    supply: terminal_psi::TargetMachineSupply::CheckedBody,
                },
            ],
        });
    let input = prepare(module.clone()).unwrap();
    assert_eq!(input.plan().functions.len(), 1);
    assert_eq!(input.plan().functions[0].machine, machine_id(1));
    module.entry = Some(callable(2));
    assert!(matches!(
        prepare(module.clone()),
        Err(ArtifactLoweringError::TargetFamily(_))
    ));
    assert!(matches!(
        prepare_at(module, target::TargetProfile::WindowsX64),
        Err(ArtifactLoweringError::Lowering(
            crate::LoweringError::UnclosedScalarApplication(_)
        ))
    ));
}

#[test]
fn native_scope_omits_dormant_scalar_work_before_lowering() {
    let input = prepare(graph()).unwrap();
    assert_eq!(input.plan().functions.len(), 1);
    assert_eq!(input.plan().functions[0].machine, machine_id(1));
    // Selection does not erase the canonical catalog used by other targets.
    assert_eq!(input.context().module().machines.len(), 3);
    assert!(matches!(
        input.plan().functions[0].operations[0],
        abstract_operations::AbstractOperation::IntegerConstant {
            value: IntegerValue::Unsigned(7),
            ..
        }
    ));
}

#[test]
fn native_scope_retains_called_scalar_work_and_its_refusal() {
    let mut module = graph();
    module.machines[0].blocks[0].operations[0].kind = call(2);
    assert!(matches!(
        prepare(module),
        Err(ArtifactLoweringError::Lowering(crate::LoweringError::UnclosedScalarApplication(application)))
            if application == ScalarApplicationId::new(1).unwrap()
    ));
}

#[test]
fn native_scope_preserves_all_block_call_demand() {
    let mut module = graph();
    let mut block = module.machines[0].blocks[0].clone();
    block.id = BlockId::new(2).unwrap();
    block.operations[0].id = OperationId::new(2).unwrap();
    block.operations[0].result = OperationResult::Scalar(ValueDeclaration {
        id: ValueId::new(2).unwrap(),
        scalar_type: scalar_type(),
        qualifications: Default::default(),
    });
    block.operations[0].kind = call(2);
    block.terminator = Terminator::Return {
        edge: EdgeId::new(2).unwrap(),
        value: ValueId::new(2).unwrap(),
        cleanup_actions: Vec::new(),
    };
    module.machines[0].blocks.push(block);
    assert!(matches!(
        prepare(module),
        Err(ArtifactLoweringError::Lowering(
            crate::LoweringError::UnclosedScalarApplication(_)
        ))
    ));
}
