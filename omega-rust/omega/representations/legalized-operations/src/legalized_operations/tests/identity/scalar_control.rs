use super::*;
use abstract_operations::ValueBinding;
use optimization_unit::{ValueDefinition, ValueDefinitionSite};

#[test]
fn structural_control_identity_binds_descriptor_telescope_and_actual_source() {
    let mut plan = scalar_call_unit_plan();
    let parameter = terminal_psi::StructuralParameterDeclaration {
        place: id(500),
        position: 0,
        is_self: false,
        structural_type: id(501),
        access: terminal_psi::StructuralAccess::SharedBorrow,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    plan.scalar_functions[0].blocks[0]
        .structural_parameters
        .push(parameter);
    plan.scalar_functions[0].blocks[0].terminator = LegalizedScalarTerminator::Jump {
        successor: LegalizedScalarSuccessor {
            edge: id(502),
            target: id(503),
            bindings: Vec::new(),
            fuel: Vec::new(),
            structural_bindings: vec![abstract_operations::AbstractStructuralBinding {
                parameter: id(500),
                argument: terminal_psi::StructuralArgument {
                    place: id(504),
                    path: Vec::new(),
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                },
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
            2 => successor.structural_bindings[0].parameter = id(506),
            3 => successor.structural_bindings[0].argument.place = id(507),
            _ => {
                successor.structural_bindings[0].argument.access =
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
                operand_type: integer,
                left: id(112),
                right: id(114),
            },
            fuel: vec![FuelSettlement {
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
        structural_bindings: Vec::new(),
        edge: id(edge),
        target: id(target),
        bindings: vec![ValueBinding {
            parameter: id(130),
            argument: id(argument),
            scalar_type,
        }],
        fuel: vec![FuelSettlement {
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
                structural_bindings: Vec::new(),
                edge: id(125),
                target: id(126),
                bindings: vec![],
                fuel: vec![FuelSettlement {
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
            6 => when_true.fuel[0].units += 1,
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
                } = &mut function.blocks[0].instructions[5].kind
                else {
                    panic!("comparison fixture")
                };
                match mutation {
                    13 => *predicate = LegalizedScalarComparison::LessThan,
                    14 => *predicate = LegalizedScalarComparison::LessOrEqual,
                    15 => *operand_type = IntegerType::new(IntegerSign::Signed, 64).unwrap(),
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
                    20 => successor.fuel[0].units += 1,
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
