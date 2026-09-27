use super::{
    BoundaryApplication, BoundaryApplicationRealization, BoundaryApplicationRealizationCompanion,
    BoundaryNominalIdentity, BoundaryOperatorRequirement, TerminalBoundaryApplicationDemand,
    TerminalBoundaryApplicationDemands, TerminalBoundaryApplicationRealizations,
    TerminalIeeeFloatComparisonOccurrenceProposal as FloatOccurrence,
    TerminalIntegerComparisonOccurrenceProposal as IntegerOccurrence,
};
use effects::provider_plan::ProviderPlan;
use effects::{CompilerIntrinsicExecutionIdentity, CompilerNumericType};
use lowered_psi::{
    LoweredSelectedIntegerComparisonOperandOrder as Order,
    LoweredSelectedIntegerComparisonOperation as Emitted,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IeeeFloatComparisonOperation as Comparison, IeeeFloatFormat,
    IntegerSign, IntegerType, MachineId, OperationId, ScalarType, ValueId,
};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
};

fn comparison_module(operand_type: ScalarType, kind: OperationKind) -> TerminalModule {
    let machine = MachineId::new(1).unwrap();
    let block = BlockId::new(1).unwrap();
    TerminalModule {
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            parameters: vec![value(1, operand_type), value(2, operand_type)],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
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
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
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

fn float_realizations(
    occurrence: FloatOccurrence,
    execution: CompilerIntrinsicExecutionIdentity,
) -> TerminalBoundaryApplicationRealizations {
    let demands = TerminalBoundaryApplicationDemands::new(
        terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([1; 32]),
        },
        vec![TerminalBoundaryApplicationDemand::new(
            occurrence.terminal_operation,
            BoundaryOperatorRequirement::new(
                BoundaryNominalIdentity::new("comparison".to_owned()).unwrap(),
                "float,float->bool".to_owned(),
            )
            .unwrap(),
            BoundaryApplication::Empty,
        )],
    )
    .unwrap();
    TerminalBoundaryApplicationRealizations::new(
        &demands,
        vec![
            BoundaryApplicationRealizationCompanion::new(
                occurrence.terminal_operation,
                *occurrence.provider_plan_commitment.as_bytes(),
                BoundaryApplicationRealization::ExactCompilerIntrinsic { execution },
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn float_coverage_replays_exact_operation_format_plan_and_execution() {
    use effects::CompilerPrimitiveFloatBinaryOperation as Execution;
    use numerics::literals::FloatFormat;
    let plans = [ProviderPlan::default()];
    for (format, execution_format) in [
        (IeeeFloatFormat::Binary32, FloatFormat::F32),
        (IeeeFloatFormat::Binary64, FloatFormat::F64),
    ] {
        for (comparison, operation) in [
            (Comparison::Equal, Execution::Equal),
            (Comparison::NotEqual, Execution::NotEqual),
            (Comparison::Less, Execution::Less),
            (Comparison::LessOrEqual, Execution::LessOrEqual),
            (Comparison::Greater, Execution::Greater),
            (Comparison::GreaterOrEqual, Execution::GreaterOrEqual),
        ] {
            let module = comparison_module(
                ScalarType::IeeeFloat(format),
                OperationKind::IeeeFloatCompare {
                    comparison,
                    left: ValueId::new(1).unwrap(),
                    right: ValueId::new(2).unwrap(),
                },
            );
            let occurrence = FloatOccurrence {
                terminal_machine: module.entry,
                terminal_operation: OperationId::new(1).unwrap(),
                provider_plan_index: 0,
                provider_plan_commitment: plans[0].identity_digest(),
                comparison,
                format,
            };
            let execution = CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary {
                operation,
                format: execution_format,
            };
            assert_eq!(occurrence.execution_identity(), execution);
            let realizations = float_realizations(occurrence, execution);
            assert_eq!(
                FloatOccurrence::validate_coverage(&module, &plans, &[occurrence], &realizations,),
                Ok(())
            );
            assert_eq!(
                FloatOccurrence::validate_roster(&module, &plans, &[]),
                Err("Terminal proposal must retain every IEEE comparison occurrence exactly once")
            );
            let wrong_operation = float_realizations(
                FloatOccurrence {
                    terminal_operation: OperationId::new(2).unwrap(),
                    ..occurrence
                },
                execution,
            );
            assert_eq!(
                FloatOccurrence::validate_coverage(
                    &module,
                    &plans,
                    &[occurrence],
                    &wrong_operation
                ),
                Err("Terminal IEEE comparison requires one exact boundary realization companion")
            );

            let altered = FloatOccurrence {
                format: match format {
                    IeeeFloatFormat::Binary32 => IeeeFloatFormat::Binary64,
                    IeeeFloatFormat::Binary64 => IeeeFloatFormat::Binary32,
                },
                ..occurrence
            };
            assert_eq!(
                FloatOccurrence::validate_roster(&module, &plans, &[altered]),
                Err("Terminal IEEE comparison changed operand format or identity")
            );
            let wrong_execution = float_realizations(occurrence, altered.execution_identity());
            assert_eq!(
                FloatOccurrence::validate_coverage(
                    &module,
                    &plans,
                    &[occurrence],
                    &wrong_execution,
                ),
                Err("Terminal IEEE comparison changed its selected boundary realization")
            );
            let wrong_plan = [ProviderPlan {
                name: "other".to_owned(),
                ..plans[0].clone()
            }];
            assert_eq!(
                FloatOccurrence::validate_roster(&module, &wrong_plan, &[occurrence]),
                Err("Terminal IEEE comparison changed its exact selected plan")
            );
        }
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
        terminal_machine: module.entry,
        terminal_operation: OperationId::new(1).unwrap(),
        provider_plan_index: 0,
        provider_plan_commitment: plans[0].identity_digest(),
        comparison: Emitted::Equal,
        operand_order: Order::Authored,
        negated: true,
        integer_type,
    };
    // A bare integer operation may be builtin; only the sealed checked scope
    // can establish whether it requires a selected-provider occurrence.
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
        integer_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        ..occurrence
    };
    assert_eq!(
        IntegerOccurrence::validate_roster(&module, &plans, &[wrong_type]),
        Err("Terminal integer comparison changed operand type or identity")
    );
}

#[test]
fn integer_execution_identity_recovers_only_admitted_authored_meanings() {
    use effects::CompilerPrimitiveIntegerComparisonOperation as Authored;
    let occurrence = IntegerOccurrence {
        terminal_machine: MachineId::new(1).unwrap(),
        terminal_operation: OperationId::new(1).unwrap(),
        provider_plan_index: 0,
        provider_plan_commitment: ProviderPlan::default().identity_digest(),
        comparison: Emitted::Equal,
        operand_order: Order::Authored,
        negated: false,
        integer_type: IntegerType::new(IntegerSign::Signed, 64).unwrap(),
    };
    for (comparison, operand_order, negated, expected) in [
        (
            Emitted::Equal,
            Order::Authored,
            false,
            Some(Authored::Equal),
        ),
        (
            Emitted::Equal,
            Order::Authored,
            true,
            Some(Authored::NotEqual),
        ),
        (
            Emitted::LessThan,
            Order::Authored,
            false,
            Some(Authored::Less),
        ),
        (
            Emitted::LessOrEqual,
            Order::Authored,
            false,
            Some(Authored::LessOrEqual),
        ),
        (
            Emitted::LessThan,
            Order::Swapped,
            false,
            Some(Authored::Greater),
        ),
        (
            Emitted::LessOrEqual,
            Order::Swapped,
            false,
            Some(Authored::GreaterOrEqual),
        ),
        (Emitted::Equal, Order::Swapped, false, None),
        (Emitted::LessThan, Order::Authored, true, None),
    ] {
        let proposal = IntegerOccurrence {
            comparison,
            operand_order,
            negated,
            ..occurrence
        };
        assert_eq!(proposal.authored_operation(), expected);
        assert_eq!(
            proposal.execution_identity(),
            expected.map(|operation| {
                CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
                    operation,
                    integer_type: CompilerNumericType::I64,
                }
            })
        );
    }
    for integer_type in [
        IntegerType::address(64).unwrap(),
        IntegerType::new(IntegerSign::Signed, 128).unwrap(),
    ] {
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
