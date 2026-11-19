use super::*;
use psi_provider_selections::admit_provider_installation;
use semantic_vocabulary::compiler_evaluation::{
    CompilerEvaluationDeclaration, CompilerEvaluationEffects, DeclarationCustody,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarApplicationId, ScalarType, ValueId,
};
use terminal_codec::{CanonicalTerminalArtifact, build_identity_optimization_execution_record};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, ProofCallableAssociation,
    ProofCallableHeader, ProofScopeDeclaration, ScalarApplication, ScalarConditionSubject,
    TargetMachineCallable, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration,
};
use terminal_semantics::entry_values::TerminalScalarValue;

fn scalar() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}
fn application(index: u64) -> ScalarApplicationId {
    ScalarApplicationId::new(index).unwrap()
}
fn machine_id(index: u64) -> MachineId {
    MachineId::new(index).unwrap()
}
fn value_id(index: u64) -> ValueId {
    ValueId::new(index).unwrap()
}
fn machine(identity: u64, operations: Vec<OperationKind>) -> TerminalMachine {
    let result = ValueDeclaration {
        id: value_id(operations.len() as u64),
        scalar_type: scalar(),
        qualifications: Default::default(),
    };
    TerminalMachine {
        id: machine_id(identity),
        attachment: None,
        structural_places: Vec::new(),
        call_formations: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        entry: BlockId::new(1).unwrap(),
        blocks: vec![Block {
            id: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: operations
                .into_iter()
                .enumerate()
                .map(|(position, kind)| {
                    Operation::unsuspended(
                        OperationId::new(position as u64 + 1).unwrap(),
                        OperationResult::Scalar(ValueDeclaration {
                            id: value_id(position as u64 + 1),
                            scalar_type: scalar(),
                            qualifications: Default::default(),
                        }),
                        kind,
                    )
                })
                .collect(),
            terminator: Terminator::Return {
                edge: EdgeId::new(1).unwrap(),
                value: result.id,
                cleanup_actions: Vec::new(),
            },
        }],
        ranked_scc: None,
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        contract: terminal_psi::MachineContract {
            selected_application: None,
            declaration: None,
            parameters: Vec::new(),
            parameter_order: Vec::new(),
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            content_entry_claims: Vec::new(),
            result: TerminalMachineResult::Scalar(result),
            declared_service_reach: None,
            closed_reach_application: None,
            published_synchronous_entries: Vec::new(),
            published_service_ceiling: Vec::new(),
            ..MachineContract::empty(ContractId::new(identity).unwrap())
        },
        assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
    }
}

fn program() -> std::sync::Arc<CanonicalTerminalArtifact> {
    let mut module = TerminalModule::for_entry(machine_id(1));
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    for (index, value) in [(1, 41), (2, 42)] {
        module.machines.push(machine(
            index,
            vec![OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(value),
            }],
        ));
        let callable = TargetMachineCallable::Machine(machine_id(index));
        module
            .scalars.scalar_applications
            .insert(ScalarApplication::Evaluate {
                entry: callable,
                scalar_type: scalar(),
            })
            .unwrap();
        let scope = &mut module.scalars.scalar_applications.proof_scope;
        let name = format!("scalar-evaluation::constant{index}");
        let id = identities.intern(&name);
        let declaration = terminal_psi::ProofDeclarationId(index as u32);
        scope
            .declarations
            .insert(ProofScopeDeclaration::Nominal(id));
        scope.callables.insert(ProofCallableAssociation::Callable {
            declaration,
            callable,
        });
        scope
            .plan
            .machines
            .insert(proof_applications::ProofApplicationMachine {
                machine: declaration,
                product_selected: true,
                ..Default::default()
            });
        scope
            .calls
            .insert_machine(proof_applications::ProofCallMachine {
                machine: declaration,
                ..Default::default()
            });
        scope.callable_headers.insert(ProofCallableHeader {
            declaration,
            compiler_evaluation: Ok(CompilerEvaluationDeclaration {
                name,
                custody: DeclarationCustody::Toolchain,
                effects: CompilerEvaluationEffects::default(),
                local_terminates: true,
                entry_requires: false,
                linear_carrier: None,
                invocation_refusal: None,
                selections: Ok(Vec::new()),
            }),
            ..Default::default()
        });
    }
    module.identities = identities.build();
    module
        .scalars.scalar_applications
        .conditions
        .retain_completion(ScalarConditionSubject::Module, application(1));
    let optimized = build_identity_optimization_execution_record(&module).unwrap();
    let artifact = CanonicalTerminalArtifact::from_parts(module, optimized, None).unwrap();
    std::sync::Arc::new(CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap())
}
fn unscoped(artifact: &CanonicalTerminalArtifact) -> ScalarScopeEvaluation {
    ScalarScopeEvaluation {
        application_roots: None,
        targets: None,
        providers: admit_provider_installation(artifact.view(), &[], &[]).unwrap(),
        address_model: Ok(None),
        carrier_layout: None,
        layout_target: None,
        import_realizations: None,
    }
}

#[test]
fn extension_executes_and_charges_each_demanded_entry_once() {
    let artifact = program();
    let mut records = Vec::new();
    let first = prepare_scalar_applications(&artifact, None, None, &[], None, &mut records, |_| {
        Ok(unscoped(&artifact))
    })
    .unwrap();
    assert_eq!(records.len(), 1);
    let extended = first
        .extend(None, &[], &[application(2)], &mut records)
        .unwrap();
    assert_eq!(records.len(), 2);
    for (index, value) in [(1, 41), (2, 42)] {
        let receipt = &records[(index - 1) as usize];
        assert_eq!(receipt.application(), application(index));
        assert_eq!(receipt.target(), None);
        assert_eq!(
            receipt.artifact().view().identity(),
            artifact.view().identity()
        );
        assert!(receipt.returned());
        assert!(receipt.usage().total_units() > 0);
        receipt.validate_charge().unwrap();
        assert!(
            matches!(extended.values().value(application(index)),Some(TerminalScalarValue::Integer{value:IntegerValue::Unsigned(actual),..}) if actual==value)
        );
    }
}

#[test]
fn an_unknown_requested_application_cannot_generate_work_receipts() {
    let artifact = program();
    let mut records = Vec::new();
    assert!(
        prepare_scalar_applications(
            &artifact,
            None,
            None,
            &[application(3)],
            None,
            &mut records,
            |_| Ok(unscoped(&artifact))
        )
        .is_err()
    );
    assert!(records.is_empty());
}
