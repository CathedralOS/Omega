use super::*;
use optimization_core::AcceptedObligationFactIdentity;
use optimization_unit::ValueDefinitionSite;

fn sequence() -> LegalizedExactIntegerSequence {
    LegalizedExactIntegerSequence {
        steps: vec![
            LegalizedIntegerStep::Immediate(LegalizedImmediate {
                source_value: id(2),
                value: IntegerValue::Unsigned(7),
                constant_operation: id(1),
                definition_site: ValueDefinitionSite::Node {
                    block: id(1),
                    node: 0,
                },
                fuel: vec![FuelSettlement {
                    site: PsiProvenance::Operation(id(1)),
                    units: 1,
                }],
            }),
            LegalizedIntegerStep::ExactBinary(LegalizedExactIntegerBinary {
                operator: LegalizedExactIntegerOperator::Add,
                source_value: id(3),
                obligation: id(1),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
                operation: id(2),
                definition_site: ValueDefinitionSite::Node {
                    block: id(1),
                    node: 1,
                },
                fuel: vec![FuelSettlement {
                    site: PsiProvenance::Operation(id(2)),
                    units: 2,
                }],
                left: id(1),
                right: id(2),
            }),
            LegalizedIntegerStep::ExactBinary(LegalizedExactIntegerBinary {
                operator: LegalizedExactIntegerOperator::Subtract,
                source_value: id(4),
                obligation: id(2),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([4; 32]),
                operation: id(3),
                definition_site: ValueDefinitionSite::Node {
                    block: id(1),
                    node: 2,
                },
                fuel: vec![],
                left: id(3),
                right: id(2),
            }),
        ],
    }
}

fn binary(sequence: &mut LegalizedExactIntegerSequence) -> &mut LegalizedExactIntegerBinary {
    let LegalizedIntegerStep::ExactBinary(binary) = &mut sequence.steps[1] else {
        panic!("binary fixture");
    };
    binary
}

#[test]
fn sequence_shape_retains_entry_uses_shared_values_and_nonfinal_results() {
    let sequence = sequence();
    assert_eq!(sequence.validate_shape(&[id(1)], id(4)), Ok(()));
    assert_eq!(sequence.validate_shape(&[id(1)], id(3)), Ok(()));
    assert_eq!(sequence.validate_shape(&[id(1)], id(1)), Ok(()));
    assert_eq!(
        LegalizedExactIntegerSequence { steps: vec![] }.validate_shape(&[id(1)], id(1)),
        Ok(())
    );
}

#[test]
fn sequence_shape_rejects_unavailable_and_redefined_values() {
    use LegalizedIntegerSequenceError as Error;
    let mut sequence = sequence();
    assert_eq!(
        sequence.validate_shape(&[], id(4)),
        Err(Error::UnavailableValue(id(1)))
    );
    assert_eq!(
        sequence.validate_shape(&[id(1), id(1)], id(4)),
        Err(Error::DuplicateValue(id(1)))
    );
    assert_eq!(
        sequence.validate_shape(&[id(1)], id(9)),
        Err(Error::UnavailableValue(id(9)))
    );
    binary(&mut sequence).left = id(4);
    assert_eq!(
        sequence.validate_shape(&[id(1)], id(4)),
        Err(Error::UnavailableValue(id(4)))
    );
    binary(&mut sequence).left = id(1);
    binary(&mut sequence).source_value = id(1);
    assert_eq!(
        sequence.validate_shape(&[id(1)], id(4)),
        Err(Error::DuplicateValue(id(1)))
    );
}

#[test]
fn sequence_shape_rejects_noncanonical_constants_and_definition_custody() {
    let original = sequence();
    for value in [
        IntegerValue::Signed(7),
        IntegerValue::Unsigned(u128::from(u64::MAX) + 1),
    ] {
        let mut sequence = original.clone();
        let LegalizedIntegerStep::Immediate(immediate) = &mut sequence.steps[0] else {
            unreachable!()
        };
        immediate.value = value;
        assert!(sequence.validate_shape(&[id(1)], id(4)).is_err());
    }
    let mut sequence = original.clone();
    binary(&mut sequence).operation = id(1);
    assert!(sequence.validate_shape(&[id(1)], id(4)).is_err());
    let mut sequence = original.clone();
    binary(&mut sequence).definition_site = ValueDefinitionSite::FunctionParameter(0);
    assert!(sequence.validate_shape(&[id(1)], id(4)).is_err());
    let mut sequence = original;
    binary(&mut sequence).definition_site = ValueDefinitionSite::Node {
        block: id(1),
        node: 0,
    };
    assert!(sequence.validate_shape(&[id(1)], id(4)).is_err());
}

fn identity(sequence: LegalizedExactIntegerSequence) -> LegalizedOperationPlanIdentity {
    let mut plan = call_aware_plan();
    plan.structural_unit_functions.clear();
    let shape = ValueShape::integer(8, 8);
    let call_plan = evaluate_call_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![shape],
            result: Some(shape),
        },
    )
    .expect("scalar ABI");
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("U64");
    let instructions = sequence
        .steps
        .into_iter()
        .map(|step| {
            let (operation, result, definition_site, fuel, kind) = match step {
                LegalizedIntegerStep::Immediate(value) => (
                    value.constant_operation,
                    value.source_value,
                    value.definition_site,
                    value.fuel,
                    LegalizedScalarInstructionKind::Constant(value.value),
                ),
                LegalizedIntegerStep::ExactBinary(value) => (
                    value.operation,
                    value.source_value,
                    value.definition_site,
                    value.fuel,
                    LegalizedScalarInstructionKind::ExactBinary {
                        operator: value.operator,
                        left: value.left,
                        right: value.right,
                        obligation: value.obligation,
                        accepted_fact: value.accepted_fact,
                    },
                ),
            };
            LegalizedScalarInstruction {
                operation,
                result,
                scalar_type,
                definition_site,
                kind,
                fuel,
                effect: EffectLink {
                    input: 0,
                    output: 0,
                },
                ownership: vec![],
            }
        })
        .collect();
    plan.scalar_functions.push(LegalizedScalarFunction {
        machine: id(1),
        attachment: None,
        provenance: TerminalPsiProvenance {
            operations: vec![id(1), id(2), id(3)],
            edges: vec![id(1)],
        },
        parameters: vec![LegalizedScalarParameter {
            value: id(1),
            scalar_type,
            definition_site: ValueDefinitionSite::FunctionParameter(0),
            placement: call_plan.parameters[0].clone(),
        }],
        call_plan,
        entry_block: id(1),
        blocks: vec![LegalizedScalarBlock {
            id: id(1),
            instructions,
            terminator: LegalizedScalarReturn {
                edge: id(1),
                value: LegalizedScalarReturnValue::Value {
                    value: id(4),
                    scalar_type,
                },
                fuel: vec![],
                effect: EffectLink {
                    input: 0,
                    output: 0,
                },
                ownership: vec![],
            },
        }],
    });
    legalized_operation_plan_identity(&plan)
}

#[test]
fn sequence_identity_commits_to_order_operands_and_all_binary_evidence() {
    let original = sequence();
    let expected = identity(original.clone());
    assert_eq!(identity(original.clone()), expected);
    let mut reordered = original.clone();
    reordered.steps.swap(0, 1);
    assert_ne!(identity(reordered), expected);
    let mutations: &[fn(&mut LegalizedExactIntegerBinary)] = &[
        |binary| binary.operator = LegalizedExactIntegerOperator::Subtract,
        |binary| binary.source_value = id(9),
        |binary| binary.obligation = id(9),
        |binary| binary.accepted_fact = AcceptedObligationFactIdentity::from_bytes([9; 32]),
        |binary| binary.operation = id(9),
        |binary| {
            binary.definition_site = ValueDefinitionSite::Node {
                block: id(9),
                node: 9,
            }
        },
        |binary| binary.fuel[0].units += 1,
        |binary| binary.fuel[0].site = PsiProvenance::Edge(id(9)),
        |binary| binary.left = id(2),
        |binary| binary.right = id(1),
    ];
    for mutate in mutations {
        let mut changed = original.clone();
        mutate(binary(&mut changed));
        assert_ne!(identity(changed), expected);
    }
    let mut shortened = original;
    shortened.steps.pop();
    assert_ne!(identity(shortened), expected);
}
