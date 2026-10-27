use super::TerminalIntegerComparisonOccurrenceProposal as IntegerOccurrence;
use effects::CompilerPrimitiveIntegerComparisonOperation as Authored;
use effects::provider_plan::ProviderPlan;
use effects::{CompilerIntrinsicExecutionIdentity, CompilerNumericType};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, ScalarType,
    ValueId,
};
use terminal_psi::{
    Block, Operation, OperationKind, OperationResult, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, ValueDeclaration,
};

fn comparison_module(operand_type: ScalarType, kind: OperationKind) -> TerminalModule {
    let machine = MachineId::new(1).unwrap();
    let block = BlockId::new(1).unwrap();
    TerminalModule {
        machines: vec![TerminalMachine {
            assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
            id: machine,
            attachment: None,
            declaration: None,
            nominal_cleanup_subject: None,
            parameters: vec![value(1, operand_type), value(2, operand_type)],
            parameter_order: vec![terminal_psi::BoundaryParameterKind::Scalar; 2],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            declared_service_reach: Vec::new(),
            closed_reach_application: None,
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![Block {
                id: block,
                parameters: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: OperationId::new(1).unwrap(),
                    result: OperationResult::Scalar(value(3, ScalarType::Boolean)),
                    kind,
                }],
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(1).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            }],
            contract: terminal_psi::MachineContract::empty(ContractId::new(1).unwrap()),
        }],
        ..TerminalModule::for_entry(machine)
    }
}

fn value(identity: u64, scalar_type: ScalarType) -> ValueDeclaration {
    ValueDeclaration {
        id: ValueId::new(identity).unwrap(),
        scalar_type,
        qualifications: Default::default(),
    }
}

#[test]
fn integer_rows_preserve_builtin_omission_and_exact_selected_negation() {
    let plans = [ProviderPlan::default()];
    let integer_type = IntegerType::new(IntegerSign::Signed, 64).unwrap();
    let mut module = comparison_module(
        ScalarType::Integer(integer_type),
        OperationKind::IntegerEqual {
            left: ValueId::new(1).unwrap(),
            right: ValueId::new(2).unwrap(),
        },
    );
    let occurrence = IntegerOccurrence {
        terminal_machine: match module.entry {
            terminal_psi::TargetMachineCallable::Machine(machine) => machine,
            _ => unreachable!("comparison fixture enters a machine"),
        },
        terminal_operation: OperationId::new(1).unwrap(),
        provider_plan_index: 0,
        provider_plan_commitment: plans[0].identity_digest(),
        comparison: Authored::NotEqual,
        integer_type: CompilerNumericType::I64,
    };
    // A bare integer operation may be builtin; the canonical demand table
    // establishes whether it requires a selected-provider occurrence.
    assert_eq!(
        IntegerOccurrence::validate_roster(&module, &plans, &[]),
        Ok(())
    );
    assert_eq!(
        IntegerOccurrence::validate_roster(&module, &plans, &[occurrence]),
        Err("Terminal integer comparison changed its authored negation")
    );
    module.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(2).unwrap(),
        result: OperationResult::Scalar(value(4, ScalarType::Boolean)),
        kind: OperationKind::BooleanNot {
            operand: ValueId::new(3).unwrap(),
        },
    });
    assert_eq!(
        IntegerOccurrence::validate_roster(&module, &plans, &[occurrence]),
        Ok(())
    );
    assert_eq!(
        IntegerOccurrence::validate_roster(&module, &plans, &[occurrence, occurrence]),
        Err("Terminal proposal repeats an integer comparison occurrence")
    );
    let mut duplicate_declaration = module.clone();
    duplicate_declaration.machines[0].blocks[0]
        .parameters
        .push(value(1, ScalarType::Integer(integer_type)));
    assert_eq!(
        IntegerOccurrence::validate_roster(&duplicate_declaration, &plans, &[occurrence]),
        Err("Terminal integer comparison changed operand type or identity")
    );
    let wrong_type = IntegerOccurrence {
        integer_type: CompilerNumericType::U64,
        ..occurrence
    };
    assert_eq!(
        IntegerOccurrence::validate_roster(&module, &plans, &[wrong_type]),
        Err("Terminal integer comparison changed operand type or identity")
    );
}

#[test]
fn integer_execution_identity_preserves_selected_meaning() {
    for comparison in Authored::ALL {
        let occurrence = IntegerOccurrence {
            terminal_machine: MachineId::new(1).unwrap(),
            terminal_operation: OperationId::new(1).unwrap(),
            provider_plan_index: 0,
            provider_plan_commitment: ProviderPlan::default().identity_digest(),
            comparison,
            integer_type: CompilerNumericType::I64,
        };
        assert_eq!(
            occurrence.execution_identity(),
            Some(
                CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
                    operation: comparison,
                    integer_type: CompilerNumericType::I64,
                }
            )
        );
        for integer_type in [CompilerNumericType::F32, CompilerNumericType::F64] {
            assert_eq!(
                IntegerOccurrence {
                    integer_type,
                    ..occurrence
                }
                .execution_identity(),
                None
            );
        }
    }
}
