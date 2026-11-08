// Plan identity and value-reference tests for scalar control flow, structural edge bindings,
// access lifecycles, and arithmetic/comparison instructions.

use super::super::{
    LegalizedExactIntegerOperator, LegalizedScalarBlock, LegalizedScalarComparison,
    LegalizedScalarInstruction, LegalizedScalarInstructionKind, LegalizedScalarSuccessor,
    LegalizedScalarTerminator, LegalizedValueDefinition, legalized_operation_plan_identity,
};
use super::identity::assert_identity_drift;
use super::{
    EffectLink, IntegerSign, IntegerType, IntegerValue, LegalizedOperationPlan, OwnershipEvent,
    PsiProvenance, ScalarType, WorkSettlement, id, scalar_call_unit_plan,
};
use abstract_operations::ValueBinding;
use optimization_core::AcceptedObligationFactIdentity;
use optimization_unit::{ValueDefinition, ValueDefinitionSite};

#[test]
fn shared_descriptor_return_identity_binds_source_and_transport_kind() {
    use crate::{LegalizedScalarReturn, LegalizedScalarReturnValue};
    let mut plan = scalar_call_unit_plan();
    plan.scalar_functions[0].blocks[0].terminator =
        LegalizedScalarTerminator::Return(LegalizedScalarReturn {
            edge: id(500),
            value: LegalizedScalarReturnValue::SharedDescriptor { place: id(501) },
            work: Vec::new(),
            effect: EffectLink {
                input: 0,
                output: 1,
            },
            ownership: Vec::new(),
        });
    let original = legalized_operation_plan_identity(&plan);
    for value in [
        LegalizedScalarReturnValue::SharedDescriptor { place: id(502) },
        LegalizedScalarReturnValue::StructuralParameter { place: id(501) },
    ] {
        let mut changed = plan.clone();
        let LegalizedScalarTerminator::Return(returned) =
            &mut changed.scalar_functions[0].blocks[0].terminator
        else {
            panic!("return fixture");
        };
        returned.value = value;
        assert_identity_drift(original, &changed);
    }
}

#[test]
fn structural_control_identity_binds_descriptor_telescope_and_actual_source() {
    let mut plan = scalar_call_unit_plan();
    let parameter = terminal_psi::StructuralParameterDeclaration {
        place: id(500),
        position: 0,
        structural_type: id(501),
        access: terminal_psi::StructuralAccess::SharedBorrow,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
    };
    plan.scalar_functions[0].blocks[0]
        .structural_parameters
        .push(parameter);
    plan.scalar_functions[0].blocks[0].terminator = LegalizedScalarTerminator::Jump {
        successor: LegalizedScalarSuccessor {
            cleanup_calls: Vec::new(),
            edge: id(502),
            target: id(503),
            bindings: Vec::new(),
            work: Vec::new(),
            structural_bindings: vec![crate::LegalizedStructuralBinding {
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter: id(500),
                    argument: terminal_psi::StructuralArgument {
                        place: id(504),
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                    },
                },
                indices: Vec::new(),
            }],
        },
        effect: EffectLink {
            input: 0,
            output: 1,
        },
        ownership: Vec::new(),
    };
    let original = legalized_operation_plan_identity(&plan);
    for mutation in 0..5 {
        let mut changed = plan.clone();
        let block = &mut changed.scalar_functions[0].blocks[0];
        let LegalizedScalarTerminator::Jump { successor, .. } = &mut block.terminator else {
            panic!("jump");
        };
        match mutation {
            0 => block.structural_parameters[0].position = 1,
            1 => block.structural_parameters[0].structural_type = id(505),
            2 => successor.structural_bindings[0].semantic.parameter = id(506),
            3 => successor.structural_bindings[0].semantic.argument.place = id(507),
            _ => {
                successor.structural_bindings[0].semantic.argument.access =
                    terminal_psi::StructuralAccess::MutableBorrow
            }
        }
        assert_ne!(original, legalized_operation_plan_identity(&changed));
    }
}

#[test]
fn scalar_control_identity_binds_parameters_edges_and_comparisons() {
    let mut plan = scalar_call_unit_plan();
    let function = &mut plan.scalar_functions[0];
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let end = function.blocks[0].terminator.clone();
    function.blocks[0]
        .instructions
        .push(LegalizedScalarInstruction {
            operation: id(120),
            result: Some(LegalizedValueDefinition {
                value: id(121),
                scalar_type: ScalarType::Boolean,
                definition_site: ValueDefinitionSite::Node {
                    block: function.entry_block,
                    node: 5,
                },
            }),
            kind: LegalizedScalarInstructionKind::Compare {
                predicate: LegalizedScalarComparison::Equal,
                operand_type: ScalarType::Integer(integer),
                right_operand_type: ScalarType::Integer(integer),
                left: id(112),
                right: id(114),
            },
            work: vec![WorkSettlement {
                site: PsiProvenance::Operation(id(120)),
                units: 1,
            }],
            effect: EffectLink {
                input: 5,
                output: 6,
            },
            ownership: vec![],
        });
    let successor = |edge, target, argument| LegalizedScalarSuccessor {
        cleanup_calls: Vec::new(),
        structural_bindings: Vec::new(),
        edge: id(edge),
        target: id(target),
        bindings: vec![ValueBinding {
            parameter: id(130),
            argument: id(argument),
            scalar_type,
        }],
        work: vec![WorkSettlement {
            site: PsiProvenance::Edge(id(edge)),
            units: 1,
        }],
    };
    function.blocks[0].terminator = LegalizedScalarTerminator::Conditional {
        condition: id(121),
        when_true: successor(122, 123, 112),
        when_false: successor(124, 123, 114),
        effect: EffectLink {
            input: 6,
            output: 7,
        },
        ownership: vec![],
    };
    function.blocks.push(LegalizedScalarBlock {
        id: id(123),
        structural_parameters: Vec::new(),
        parameters: vec![ValueDefinition {
            value: id(130),
            scalar_type,
            site: ValueDefinitionSite::BlockParameter {
                block: id(123),
                position: 0,
            },
        }],
        instructions: vec![],
        terminator: LegalizedScalarTerminator::Jump {
            successor: LegalizedScalarSuccessor {
                cleanup_calls: Vec::new(),
                structural_bindings: Vec::new(),
                edge: id(125),
                target: id(126),
                bindings: vec![],
                work: vec![WorkSettlement {
                    site: PsiProvenance::Edge(id(125)),
                    units: 1,
                }],
            },
            effect: EffectLink {
                input: 0,
                output: 1,
            },
            ownership: vec![],
        },
    });
    function.blocks.push(LegalizedScalarBlock {
        id: id(126),
        structural_parameters: Vec::new(),
        parameters: vec![],
        instructions: vec![],
        terminator: end,
    });
    let identity = legalized_operation_plan_identity(&plan);
    for mutation in 0..25 {
        let mut changed = plan.clone();
        let function = &mut changed.scalar_functions[0];
        let LegalizedScalarTerminator::Conditional {
            condition,
            when_true,
            when_false,
            effect,
            ownership,
        } = &mut function.blocks[0].terminator
        else {
            panic!("conditional fixture")
        };
        match mutation {
            0 => *condition = id(999),
            1 => when_true.edge = id(999),
            2 => when_true.target = id(999),
            3 => when_true.bindings[0].argument = id(999),
            4 => when_true.bindings[0].parameter = id(999),
            5 => when_true.bindings[0].scalar_type = ScalarType::Boolean,
            6 => when_true.work[0].units += 1,
            7 => std::mem::swap(when_true, when_false),
            8 => effect.output += 1,
            9 => ownership.push(OwnershipEvent::Cleanup(vec![])),
            10 => function.blocks[1].parameters[0].value = id(999),
            11 => function.blocks[1].parameters[0].scalar_type = ScalarType::Boolean,
            12 => function.blocks[1].parameters[0].site = ValueDefinitionSite::FunctionParameter(0),
            13..=17 => {
                let LegalizedScalarInstructionKind::Compare {
                    predicate,
                    operand_type,
                    left,
                    right,
                    ..
                } = &mut function.blocks[0].instructions[5].kind
                else {
                    panic!("comparison fixture")
                };
                match mutation {
                    13 => *predicate = LegalizedScalarComparison::LessThan,
                    14 => *predicate = LegalizedScalarComparison::LessOrEqual,
                    15 => {
                        *operand_type =
                            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap())
                    }
                    16 => *left = id(999),
                    _ => *right = id(999),
                }
            }
            18..=22 => {
                let LegalizedScalarTerminator::Jump {
                    successor,
                    effect,
                    ownership,
                } = &mut function.blocks[1].terminator
                else {
                    panic!("jump fixture")
                };
                match mutation {
                    18 => successor.edge = id(999),
                    19 => successor.target = id(999),
                    20 => successor.work[0].units += 1,
                    21 => effect.output += 1,
                    _ => ownership.push(OwnershipEvent::Cleanup(vec![])),
                }
            }
            23 => function.blocks.swap(1, 2),
            _ => {
                function.blocks[0].instructions[5]
                    .result
                    .as_mut()
                    .unwrap()
                    .scalar_type = scalar_type
            }
        }
        assert_identity_drift(identity, &changed);
    }
}

#[test]
fn structural_edge_identity_and_liveness_retain_selector_evidence() {
    let mut plan = scalar_call_unit_plan();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    plan.scalar_functions[0].blocks[0].terminator = LegalizedScalarTerminator::Jump {
        successor: LegalizedScalarSuccessor {
            edge: id(502),
            target: id(503),
            bindings: Vec::new(),
            work: Vec::new(),
            cleanup_calls: Vec::new(),
            structural_bindings: vec![crate::LegalizedStructuralBinding {
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter: id(500),
                    argument: terminal_psi::StructuralArgument {
                        place: id(504),
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                    },
                },
                indices: vec![crate::LegalizedRuntimeIndexOperand {
                    operand: abstract_operations::AbstractResult {
                        value: id(510),
                        scalar_type,
                    },
                    stride: 16,
                    extent: Some(4),
                    obligation: id(511),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [3; 32],
                    ),
                }],
            }],
        },
        effect: EffectLink {
            input: 0,
            output: 1,
        },
        ownership: Vec::new(),
    };
    assert!(plan.scalar_functions[0].references_value(id(510)));
    assert!(!plan.scalar_functions[0].references_value(id(512)));
    let original = legalized_operation_plan_identity(&plan);
    for mutation in 0..6 {
        let mut changed = plan.clone();
        let LegalizedScalarTerminator::Jump { successor, .. } =
            &mut changed.scalar_functions[0].blocks[0].terminator
        else {
            panic!("jump fixture");
        };
        let index = &mut successor.structural_bindings[0].indices[0];
        match mutation {
            0 => index.operand.value = id(512),
            1 => index.stride = 8,
            2 => index.extent = Some(5),
            3 => index.obligation = id(513),
            4 => {
                index.accepted_fact =
                    optimization_core::AcceptedObligationFactIdentity::from_bytes([4; 32])
            }
            _ => successor.structural_bindings[0].indices.clear(),
        }
        assert_identity_drift(original, &changed);
    }
}

#[test]
fn lifecycle_identity_and_liveness_retain_original_owner_and_projection_evidence() {
    let mut plan = scalar_call_unit_plan();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let instruction = LegalizedScalarInstruction {
        operation: id(400),
        result: None,
        kind: LegalizedScalarInstructionKind::AccessLifecycle {
            boundary: id(401),
            operands: abstract_operations::AccessLifecycleOperands::MoveOut {
                storage: terminal_psi::StructuralArgument {
                    place: id(402),
                    path: vec![terminal_psi::StructuralPathSegment::RuntimeIndex {
                        index: id(403),
                        obligation: id(404),
                    }],
                    access: terminal_psi::StructuralAccess::Owned,
                },
                profile: terminal_psi::StructuralArgument {
                    place: id(412),
                    path: vec![],
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                },
            },
            indices: vec![crate::LegalizedRuntimeIndexOperand {
                operand: abstract_operations::AbstractResult {
                    value: id(403),
                    scalar_type,
                },
                stride: 16,
                extent: Some(4),
                obligation: id(404),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
            }],
            result: terminal_psi::StructuralOperationResult {
                place: id(405),
                structural_type: id(406),
                multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                qualifications: vec![],
                projected_qualifications: vec![],
                additional_input_qualifications: Vec::new(),
                qualification_establishments: vec![],
                claims: vec![],
            },
        },
        work: vec![],
        effect: EffectLink {
            input: 0,
            output: 1,
        },
        ownership: vec![],
    };
    assert!(instruction.references_value(id(403)));
    assert!(!instruction.references_value(id(402)));
    assert!(instruction.result.is_none());
    plan.scalar_functions[0].blocks[0].instructions = vec![instruction];
    let original = legalized_operation_plan_identity(&plan);
    for mutation in 0..7 {
        let mut changed = plan.clone();
        let LegalizedScalarInstructionKind::AccessLifecycle {
            boundary,
            operands,
            indices,
            result,
        } = &mut changed.scalar_functions[0].blocks[0].instructions[0].kind
        else {
            panic!("lifecycle fixture");
        };
        match mutation {
            0 => *boundary = id(407),
            1 => {
                let abstract_operations::AccessLifecycleOperands::MoveOut { storage, .. } =
                    operands
                else {
                    panic!("extraction fixture");
                };
                storage.place = id(408);
            }
            2 => indices[0].operand.value = id(409),
            3 => indices[0].stride = 8,
            4 => indices[0].accepted_fact = AcceptedObligationFactIdentity::from_bytes([4; 32]),
            5 => result.qualifications.push(id(410)),
            _ => result.place = id(411),
        }
        assert_identity_drift(original, &changed);
    }
}

fn operation_plan() -> LegalizedOperationPlan {
    let mut plan = scalar_call_unit_plan();
    let function = &mut plan.scalar_functions[0];
    let narrow = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let wide = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let rows = [
        (
            ScalarType::Integer(narrow),
            LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(7)),
        ),
        (
            ScalarType::Integer(narrow),
            LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(9)),
        ),
        (
            ScalarType::Integer(narrow),
            LegalizedScalarInstructionKind::ExactBinary {
                operator: LegalizedExactIntegerOperator::Add,
                left: id(200),
                right: id(201),
                obligation: id(300),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
            },
        ),
        (
            ScalarType::Integer(wide),
            LegalizedScalarInstructionKind::IntegerWiden {
                operand: id(202),
                source_type: narrow,
            },
        ),
        (
            ScalarType::Boolean,
            LegalizedScalarInstructionKind::Compare {
                predicate: LegalizedScalarComparison::Equal,
                operand_type: ScalarType::Integer(wide),
                right_operand_type: ScalarType::Integer(wide),
                left: id(203),
                right: id(203),
            },
        ),
        (
            ScalarType::Boolean,
            LegalizedScalarInstructionKind::BooleanNot { operand: id(204) },
        ),
    ];
    function.blocks[0].instructions = rows
        .into_iter()
        .enumerate()
        .map(|(position, (scalar_type, kind))| {
            let operation = id(400 + position as u64);
            LegalizedScalarInstruction {
                operation,
                result: Some(LegalizedValueDefinition {
                    value: id(200 + position as u64),
                    scalar_type,
                    definition_site: ValueDefinitionSite::Node {
                        block: function.entry_block,
                        node: position as u32,
                    },
                }),
                kind,
                work: vec![WorkSettlement {
                    site: PsiProvenance::Operation(operation),
                    units: 1,
                }],
                effect: EffectLink {
                    input: position as u64,
                    output: position as u64 + 1,
                },
                ownership: vec![],
            }
        })
        .collect();
    function.provenance.operations = function.blocks[0]
        .instructions
        .iter()
        .map(|row| row.operation)
        .collect();
    plan
}

#[test]
fn ieee_comparison_identity_binds_relation_format_and_ordered_operands() {
    use semantic_vocabulary::{IeeeFloatComparisonOperation as Comparison, IeeeFloatFormat};
    let mut plan = operation_plan();
    plan.scalar_functions[0].blocks[0].instructions[4].kind =
        LegalizedScalarInstructionKind::IeeeFloatCompare {
            comparison: Comparison::Less,
            format: IeeeFloatFormat::Binary32,
            left: id(200),
            right: id(201),
        };
    let identity = legalized_operation_plan_identity(&plan);
    for mutation in 0..4 {
        let mut changed = plan.clone();
        let LegalizedScalarInstructionKind::IeeeFloatCompare {
            comparison,
            format,
            left,
            right,
        } = &mut changed.scalar_functions[0].blocks[0].instructions[4].kind
        else {
            panic!("comparison")
        };
        match mutation {
            0 => *comparison = Comparison::Greater,
            1 => *format = IeeeFloatFormat::Binary64,
            2 => std::mem::swap(left, right),
            3 => *left = id(202),
            _ => unreachable!(),
        }
        assert_ne!(identity, legalized_operation_plan_identity(&changed));
    }
}

#[test]
fn wrapping_remainder_identity_binds_policy_width_operands_and_nonzero_fact() {
    let mut plan = operation_plan();
    let row = &mut plan.scalar_functions[0].blocks[0].instructions[2];
    row.result.as_mut().unwrap().scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    row.kind = LegalizedScalarInstructionKind::WrappingRemainder {
        left: id(200),
        right: id(201),
        obligation: id(300),
        accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
    };
    let identity = legalized_operation_plan_identity(&plan);
    for mutation in 0..6 {
        let mut changed = plan.clone();
        let row = &mut changed.scalar_functions[0].blocks[0].instructions[2];
        let LegalizedScalarInstructionKind::WrappingRemainder {
            left,
            right,
            obligation,
            accepted_fact,
        } = &mut row.kind
        else {
            panic!("remainder");
        };
        match mutation {
            0 => std::mem::swap(left, right),
            1 => *left = id(999),
            2 => *obligation = id(999),
            3 => *accepted_fact = AcceptedObligationFactIdentity::from_bytes([4; 32]),
            4 => {
                row.result.as_mut().unwrap().scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap())
            }
            _ => {
                row.kind = LegalizedScalarInstructionKind::ExactBinary {
                    operator: LegalizedExactIntegerOperator::Divide,
                    left: *left,
                    right: *right,
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                }
            }
        }
        assert_identity_drift(identity, &changed);
    }
}

#[test]
fn wrapping_divide_identity_binds_operands_and_nonzero_fact() {
    let mut plan = operation_plan();
    let row = &mut plan.scalar_functions[0].blocks[0].instructions[2];
    row.result.as_mut().unwrap().scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
    row.kind = LegalizedScalarInstructionKind::WrappingDivide {
        left: id(200),
        right: id(201),
        obligation: id(300),
        accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
    };
    let identity = legalized_operation_plan_identity(&plan);
    for mutation in 0..6 {
        let mut changed = plan.clone();
        let row = &mut changed.scalar_functions[0].blocks[0].instructions[2];
        let LegalizedScalarInstructionKind::WrappingDivide {
            left,
            right,
            obligation,
            accepted_fact,
        } = &mut row.kind
        else {
            panic!("divide");
        };
        match mutation {
            0 => std::mem::swap(left, right),
            1 => *left = id(999),
            2 => *obligation = id(999),
            3 => *accepted_fact = AcceptedObligationFactIdentity::from_bytes([4; 32]),
            4 => {
                row.result.as_mut().unwrap().scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
            }
            _ => {
                row.kind = LegalizedScalarInstructionKind::WrappingRemainder {
                    left: *left,
                    right: *right,
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                }
            }
        }
        assert_identity_drift(identity, &changed);
    }
}

#[test]
fn saturating_multiply_identity_binds_carrier_operation_and_operands() {
    use crate::SaturatingCarrier;
    let mut plan = operation_plan();
    let row = &mut plan.scalar_functions[0].blocks[0].instructions[2];
    row.kind = LegalizedScalarInstructionKind::SaturatingMultiply {
        carrier: SaturatingCarrier::U8,
        left: id(200),
        right: id(201),
    };
    let identity = legalized_operation_plan_identity(&plan);
    let (left, right) = (id(200), id(201));
    let mut substitutes = vec![
        LegalizedScalarInstructionKind::SaturatingMultiply {
            carrier: SaturatingCarrier::U8,
            left: right,
            right: left,
        },
        LegalizedScalarInstructionKind::WrappingMultiply { left, right },
    ];
    for carrier in SaturatingCarrier::ALL {
        if carrier != SaturatingCarrier::U8 {
            substitutes.push(LegalizedScalarInstructionKind::SaturatingMultiply {
                carrier,
                left,
                right,
            });
        }
        substitutes.push(LegalizedScalarInstructionKind::SaturatingAdd {
            carrier,
            left,
            right,
        });
        substitutes.push(LegalizedScalarInstructionKind::SaturatingSubtract {
            carrier,
            left,
            right,
        });
    }
    for substitute in substitutes {
        let mut changed = plan.clone();
        changed.scalar_functions[0].blocks[0].instructions[2].kind = substitute;
        assert_identity_drift(identity, &changed);
    }
}

#[test]
fn bitwise_and_wrapping_kind_identity_binds_the_named_operation() {
    let plan = operation_plan();
    let identity = legalized_operation_plan_identity(&plan);
    for kind in [
        LegalizedScalarInstructionKind::WrappingSubtract {
            left: id(200),
            right: id(201),
        },
        LegalizedScalarInstructionKind::WrappingMultiply {
            left: id(200),
            right: id(201),
        },
        LegalizedScalarInstructionKind::BitwiseOr {
            left: id(200),
            right: id(201),
        },
        LegalizedScalarInstructionKind::BitwiseNot { operand: id(200) },
    ] {
        let mut changed = plan.clone();
        changed.scalar_functions[0].blocks[0].instructions[2].kind = kind;
        assert_identity_drift(identity, &changed);
    }
    let mut multiply = plan.clone();
    multiply.scalar_functions[0].blocks[0].instructions[2].kind =
        LegalizedScalarInstructionKind::WrappingMultiply {
            left: id(200),
            right: id(201),
        };
    let multiply_identity = legalized_operation_plan_identity(&multiply);
    let mut changed = multiply.clone();
    let LegalizedScalarInstructionKind::WrappingMultiply { left, .. } =
        &mut changed.scalar_functions[0].blocks[0].instructions[2].kind
    else {
        panic!("multiply fixture")
    };
    *left = id(999);
    assert_identity_drift(multiply_identity, &changed);
}

#[test]
fn scalar_operation_identity_binds_each_authored_row_envelope_and_order() {
    let plan = operation_plan();
    let identity = legalized_operation_plan_identity(&plan);
    assert_eq!(identity, legalized_operation_plan_identity(&plan.clone()));
    for position in 0..6 {
        for mutation in 0..9 {
            let mut changed = plan.clone();
            let row = &mut changed.scalar_functions[0].blocks[0].instructions[position];
            match mutation {
                0 => row.operation = id(999),
                1 => row.result.as_mut().unwrap().value = id(999),
                2 => {
                    row.result.as_mut().unwrap().scalar_type =
                        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
                }
                3 => {
                    row.result.as_mut().unwrap().definition_site =
                        ValueDefinitionSite::FunctionParameter(0)
                }
                4 => row.work[0].site = PsiProvenance::Operation(id(999)),
                5 => row.work[0].units += 1,
                6 => row.effect.input += 1,
                7 => row.effect.output += 1,
                _ => row.ownership.push(OwnershipEvent::Cleanup(vec![])),
            }
            assert_identity_drift(identity, &changed);
        }
        let mut omitted = plan.clone();
        omitted.scalar_functions[0].blocks[0]
            .instructions
            .remove(position);
        assert_identity_drift(identity, &omitted);
        if position > 0 {
            let mut reordered = plan.clone();
            reordered.scalar_functions[0].blocks[0]
                .instructions
                .swap(position - 1, position);
            assert_identity_drift(identity, &reordered);
        }
    }
}

#[test]
fn scalar_operation_identity_binds_narrow_proof_widening_and_boolean_sources() {
    let plan = operation_plan();
    let identity = legalized_operation_plan_identity(&plan);
    for mutation in 0..15 {
        let mut changed = plan.clone();
        let rows = &mut changed.scalar_functions[0].blocks[0].instructions;
        match mutation {
            0 => rows[0].kind = LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(8)),
            1..=5 | 12..=14 => {
                let LegalizedScalarInstructionKind::ExactBinary {
                    operator,
                    left,
                    right,
                    obligation,
                    accepted_fact,
                } = &mut rows[2].kind
                else {
                    panic!("exact binary fixture")
                };
                match mutation {
                    1 => *operator = LegalizedExactIntegerOperator::Subtract,
                    12 => *operator = LegalizedExactIntegerOperator::Divide,
                    13 => *operator = LegalizedExactIntegerOperator::Multiply,
                    14 => *operator = LegalizedExactIntegerOperator::Remainder,
                    2 => *left = id(999),
                    3 => *right = id(999),
                    4 => *obligation = id(999),
                    _ => *accepted_fact = AcceptedObligationFactIdentity::from_bytes([4; 32]),
                }
            }
            6..=7 => {
                let LegalizedScalarInstructionKind::IntegerWiden {
                    operand,
                    source_type,
                } = &mut rows[3].kind
                else {
                    panic!("widen fixture")
                };
                if mutation == 6 {
                    *operand = id(999);
                } else {
                    *source_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
                }
            }
            8..=10 => {
                let LegalizedScalarInstructionKind::Compare {
                    predicate,
                    left,
                    right,
                    ..
                } = &mut rows[4].kind
                else {
                    panic!("compare fixture")
                };
                match mutation {
                    8 => *predicate = LegalizedScalarComparison::LessOrEqual,
                    9 => *left = id(999),
                    _ => *right = id(999),
                }
            }
            _ => rows[5].kind = LegalizedScalarInstructionKind::BooleanNot { operand: id(999) },
        }
        assert_identity_drift(identity, &changed);
    }
    let mut role = plan.clone();
    role.scalar_functions[0].blocks[0].instructions[5].kind =
        LegalizedScalarInstructionKind::IntegerWiden {
            operand: id(204),
            source_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        };
    assert_identity_drift(identity, &role);
}
