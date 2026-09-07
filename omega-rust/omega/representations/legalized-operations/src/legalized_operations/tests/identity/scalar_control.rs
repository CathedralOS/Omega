use super::*;
use abstract_operations::ValueBinding;
use optimization_unit::{ValueDefinition, ValueDefinitionSite};

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
            result: id(121),
            scalar_type: ScalarType::Boolean,
            definition_site: ValueDefinitionSite::Node {
                block: function.entry_block,
                node: 5,
            },
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
            _ => function.blocks[0].instructions[5].scalar_type = scalar_type,
        }
        assert_identity_drift(identity, &changed);
    }
}
