//! Float calls, arrivals and comparisons share the independently replayed graph.
use super::*;
use abstract_operations::{AbstractBlockEntry, AbstractParameter, AbstractResult, ValueBinding};
use semantic_vocabulary::{
    EdgeId, FuelScheduleIdentity, IeeeFloatComparisonOperation, IeeeFloatFormat, OperationId,
};

fn value(ordinal: u64) -> ValueId {
    ValueId::new(ordinal).unwrap()
}
fn block(ordinal: u64) -> BlockId {
    BlockId::new(ordinal).unwrap()
}
fn operation(ordinal: u64) -> OperationId {
    OperationId::new(ordinal).unwrap()
}
fn edge(ordinal: u64) -> EdgeId {
    EdgeId::new(ordinal).unwrap()
}

fn fixture(
    format: IeeeFloatFormat,
    comparison: IeeeFloatComparisonOperation,
) -> AbstractOperationPlan {
    let scalar_type = ScalarType::IeeeFloat(format);
    let parameter = |ordinal| AbstractParameter {
        value: value(ordinal),
        scalar_type,
    };
    let entry = |ordinal, operation_offset, parameters| AbstractBlockEntry {
        block: block(ordinal),
        operation_offset,
        parameters,
        structural_parameters: Vec::new(),
    };
    let callee = MachineId::new(2).unwrap();
    let caller = AbstractFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        entry: block(1),
        parameters: vec![parameter(1), parameter(2)],
        structural_parameters: Vec::new(),
        result: AbstractFunctionResult::Scalar(AbstractResult {
            value: value(9),
            scalar_type: ScalarType::Boolean,
        }),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![
            entry(1, 0, Vec::new()),
            entry(2, 1, vec![parameter(3), parameter(4)]),
        ],
        operations: vec![
            AbstractOperation::Jump {
                psi_edge: edge(1),
                target: block(2),
                bindings: vec![
                    ValueBinding {
                        parameter: value(3),
                        argument: value(1),
                        scalar_type,
                    },
                    ValueBinding {
                        parameter: value(4),
                        argument: value(2),
                        scalar_type,
                    },
                ],
                structural_bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: Vec::new(),
            },
            AbstractOperation::Call {
                psi_operation: operation(1),
                result: value(5),
                scalar_type,
                callee,
                arguments: vec![value(3)],
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
            AbstractOperation::IeeeFloatCompare {
                psi_operation: operation(2),
                result: value(6),
                comparison,
                format,
                left: value(5),
                right: value(4),
            },
            AbstractOperation::Return {
                psi_edge: edge(2),
                result: value(9),
                value: value(6),
                scalar_type: ScalarType::Boolean,
                cleanup_actions: Vec::new(),
            },
        ],
    };
    let identity = AbstractFunction {
        machine: callee,
        attachment: None,
        entry: block(3),
        parameters: vec![parameter(10)],
        structural_parameters: Vec::new(),
        result: AbstractFunctionResult::Scalar(AbstractResult {
            value: value(11),
            scalar_type,
        }),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![entry(3, 0, Vec::new())],
        operations: vec![AbstractOperation::Return {
            psi_edge: edge(3),
            result: value(11),
            value: value(10),
            scalar_type,
            cleanup_actions: Vec::new(),
        }],
    };
    AbstractOperationPlan {
        psi: terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([73; 32]),
        },
        entry: caller.machine,
        structural_types: Vec::new().into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![caller, identity],
    }
}

#[test]
fn ieee_comparisons_preserve_float_calls_and_block_arrivals() {
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        for comparison in [
            IeeeFloatComparisonOperation::Equal,
            IeeeFloatComparisonOperation::NotEqual,
            IeeeFloatComparisonOperation::Less,
            IeeeFloatComparisonOperation::LessOrEqual,
            IeeeFloatComparisonOperation::Greater,
            IeeeFloatComparisonOperation::GreaterOrEqual,
        ] {
            for native in [
                ::target::NativeTarget::linux_x64(),
                ::target::NativeTarget::linux_arm64(),
                ::target::NativeTarget::windows_x64(),
                ::target::NativeTarget::macos_arm64(),
            ] {
                let source = fixture(format, comparison);
                let target = abstract_operations_to_target_operations::lower_to_target_operations(
                    &source, native,
                )
                .unwrap();
                let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                    &source,
                    FuelScheduleIdentity::new(1).unwrap(),
                )
                .unwrap();
                let legalized = crate::legalize_target_operations(&target, &source, &unit).unwrap();
                crate::validate_legalized_operations(
                    &target,
                    &source,
                    &unit,
                    legalized.plan().clone(),
                )
                .unwrap();
            }
        }
    }
}

#[test]
fn ieee_comparison_replay_rejects_changed_operands_format_relation_and_arrival() {
    let source = fixture(
        IeeeFloatFormat::Binary32,
        IeeeFloatComparisonOperation::Less,
    );
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        ::target::NativeTarget::macos_arm64(),
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    let legalized = crate::legalize_target_operations(&target, &source, &unit).unwrap();
    let mut changed_legalized = legalized.plan().clone();
    let legalized_operations::LegalizedScalarInstructionKind::IeeeFloatCompare {
        left, right, ..
    } = &mut changed_legalized.scalar_functions[0].blocks[1].instructions[1].kind
    else {
        panic!("legalized comparison")
    };
    std::mem::swap(left, right);
    assert!(
        crate::validate_legalized_operations(&target, &source, &unit, changed_legalized).is_err()
    );
    for mutation in 0..5 {
        let mut changed = target.clone();
        let graph = &mut changed.functions[0].graph;
        if mutation == 4 {
            graph.blocks[1].parameters[0].scalar_type =
                ScalarType::IeeeFloat(IeeeFloatFormat::Binary64);
        } else {
            let TargetUnitOperation::IeeeFloatCompare {
                comparison,
                format,
                left,
                right,
                result_home,
            } = &mut graph.blocks[1].operations[1]
            else {
                panic!("comparison")
            };
            match mutation {
                0 => std::mem::swap(left, right),
                1 => *format = IeeeFloatFormat::Binary64,
                2 => *comparison = IeeeFloatComparisonOperation::Equal,
                3 => result_home.scalar_type = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
                _ => unreachable!(),
            }
        }
        assert!(
            crate::legalize_target_operations(&changed, &source, &unit).is_err(),
            "mutation {mutation}"
        );
    }
    let mut changed_source = source.clone();
    let AbstractOperation::IeeeFloatCompare { format, .. } =
        &mut changed_source.functions[0].operations[2]
    else {
        panic!("comparison")
    };
    *format = IeeeFloatFormat::Binary64;
    assert!(
        abstract_operations_to_target_operations::lower_to_target_operations(
            &changed_source,
            ::target::NativeTarget::macos_arm64()
        )
        .is_err()
    );
}

#[test]
fn ten_float_arguments_retain_incoming_outgoing_stack_and_result_abi() {
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        let scalar_type = ScalarType::IeeeFloat(format);
        let mut source = fixture(format, IeeeFloatComparisonOperation::Equal);
        let callee = source.functions[1].machine;
        for (position, function) in source.functions.iter_mut().enumerate() {
            let first_value = if position == 0 { 1 } else { 20 };
            function.parameters = (first_value..first_value + 10)
                .map(|ordinal| AbstractParameter {
                    value: value(ordinal),
                    scalar_type,
                })
                .collect();
            let result = value(first_value + 11);
            function.result = AbstractFunctionResult::Scalar(AbstractResult {
                value: result,
                scalar_type,
            });
            function.block_entries.truncate(1);
            function.operations.clear();
            let returned = if position == 0 {
                let returned = value(11);
                function.operations.push(AbstractOperation::Call {
                    psi_operation: operation(1),
                    result: returned,
                    scalar_type,
                    callee,
                    arguments: function
                        .parameters
                        .iter()
                        .map(|parameter| parameter.value)
                        .collect(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                });
                returned
            } else {
                value(first_value + 9)
            };
            function.operations.push(AbstractOperation::Return {
                psi_edge: edge(position as u64 + 1),
                result,
                value: returned,
                scalar_type,
                cleanup_actions: Vec::new(),
            });
        }
        for native in [
            ::target::NativeTarget::linux_x64(),
            ::target::NativeTarget::linux_arm64(),
            ::target::NativeTarget::windows_x64(),
            ::target::NativeTarget::macos_arm64(),
        ] {
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source, native,
            )
            .unwrap();
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            let legalized = crate::legalize_target_operations(&target, &source, &unit).unwrap();
            crate::validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
                .unwrap();
            for function in &target.functions {
                let abi = function.scalar_abi.as_ref().unwrap();
                assert_eq!(abi.result.scalar_type, scalar_type);
                assert!(
                    abi.parameters[9]
                        .placement
                        .locations
                        .iter()
                        .any(|location| matches!(location, ValueLocation::Stack { .. }))
                );
            }
            let mut changed = target.clone();
            let placement =
                &mut changed.functions[0].scalar_abi.as_mut().unwrap().parameters[9].placement;
            let ValueLocation::Stack {
                stack_byte_offset, ..
            } = &mut placement.locations[0]
            else {
                panic!("tenth float argument is stack-passed")
            };
            *stack_byte_offset += 1;
            assert!(
                crate::legalize_target_operations(&changed, &source, &unit).is_err(),
                "changed incoming stack placement {native:?}"
            );
        }
    }
}
