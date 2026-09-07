//! Current graph identity coverage; raw mutation tests do not grant source admission.
use super::*;
use optimization_core::AcceptedObligationFactIdentity;
use optimization_unit::ValueDefinitionSite;

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
                operand_type: wide,
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
                result: id(200 + position as u64),
                scalar_type,
                kind,
                definition_site: ValueDefinitionSite::Node {
                    block: function.entry_block,
                    node: position as u32,
                },
                fuel: vec![FuelSettlement {
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
                1 => row.result = id(999),
                2 => {
                    row.scalar_type =
                        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
                }
                3 => row.definition_site = ValueDefinitionSite::FunctionParameter(0),
                4 => row.fuel[0].site = PsiProvenance::Operation(id(999)),
                5 => row.fuel[0].units += 1,
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
    for mutation in 0..12 {
        let mut changed = plan.clone();
        let rows = &mut changed.scalar_functions[0].blocks[0].instructions;
        match mutation {
            0 => rows[0].kind = LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(8)),
            1..=5 => {
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
