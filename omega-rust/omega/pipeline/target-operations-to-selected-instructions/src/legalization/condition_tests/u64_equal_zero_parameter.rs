//! Exact U64 parameter-equals-zero producer/replay coverage.

use super::*;

fn fixture() -> Fixture {
    let mut fixture = super::fixture();
    let zero = fixture.right;
    let zero_operation = id::<OperationId>(16);
    let entry = fixture.optimized.entry;
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let scalar_type = ScalarType::Integer(u64_type);
    let zero_constant = AbstractOperation::IntegerConstant {
        psi_operation: zero_operation,
        result: zero,
        scalar_type,
        value: IntegerValue::Unsigned(0),
    };
    let equality = AbstractOperation::IntegerEqual {
        psi_operation: fixture.operation,
        result: fixture.condition,
        left: fixture.left,
        right: zero,
    };
    fixture.abstracted.parameters.truncate(1);
    fixture.abstracted.operations = vec![zero_constant.clone(), equality.clone()];
    fixture.optimized.parameters.truncate(1);
    fixture.optimized.blocks[0].nodes = vec![
        OptimizationNode {
            operation: zero_constant,
            provenance: vec![PsiProvenance::Operation(zero_operation)],
            fuel: vec![FuelSettlement {
                site: PsiProvenance::Operation(zero_operation),
                units: 1,
            }],
            effect: EffectLink {
                input: 0,
                output: 1,
            },
            definitions: vec![ValueDefinition {
                value: zero,
                scalar_type,
                site: ValueDefinitionSite::Node {
                    block: entry,
                    node: 0,
                },
            }],
            uses: Vec::new(),
            successors: Vec::new(),
            ownership: Vec::new(),
        },
        OptimizationNode {
            operation: equality,
            provenance: vec![PsiProvenance::Operation(fixture.operation)],
            fuel: vec![FuelSettlement {
                site: PsiProvenance::Operation(fixture.operation),
                units: 1,
            }],
            effect: EffectLink {
                input: 1,
                output: 2,
            },
            definitions: vec![ValueDefinition {
                value: fixture.condition,
                scalar_type: ScalarType::Boolean,
                site: ValueDefinitionSite::Node {
                    block: entry,
                    node: 1,
                },
            }],
            uses: vec![
                ValueUse {
                    value: fixture.left,
                    block: entry,
                    node: 1,
                },
                ValueUse {
                    value: zero,
                    block: entry,
                    node: 1,
                },
            ],
            successors: Vec::new(),
            ownership: Vec::new(),
        },
    ];
    let TargetOperation::ReturnIntegerExpressionConditionalControl { condition, .. } =
        &mut fixture.target.operation
    else {
        unreachable!("comparison fixture")
    };
    *condition = TargetBooleanExpression::IntegerEqual {
        psi_operation: fixture.operation,
        scalar_type: u64_type,
        left: Box::new(TargetIntegerExpression::Parameter {
            source_value: fixture.left,
            parameter_index: 0,
            location: ScalarParameterLocation::Register(MachineRegister::X86Rdi),
        }),
        right: Box::new(TargetIntegerExpression::Immediate {
            source_value: zero,
            value: IntegerValue::Unsigned(0),
        }),
    };
    fixture
}

#[test]
fn condition_is_produced_and_independently_replayed() {
    let fixture = fixture();
    let derived = source::derive_condition_for_test(
        0,
        &fixture.target,
        &fixture.abstracted,
        &fixture.optimized,
    )
    .expect("U64 parameter-equals-zero source condition");
    assert_eq!(derived.source, fixture.condition);
    assert_eq!(derived.shape, ScalarConditionShape::U64EqualZeroParameter);
    let LegalizedCondition::U64EqualZeroParameterV1 {
        operation,
        parameter,
        zero,
        ..
    } = &derived.legalized
    else {
        panic!("U64 parameter-equals-zero custody")
    };
    assert_eq!(*operation, fixture.operation);
    assert_eq!(parameter.source_value, fixture.left);
    assert_eq!(parameter.parameter_index, 0);
    assert_eq!(zero.source_value, fixture.right);
    assert_eq!(zero.value, IntegerValue::Unsigned(0));

    let replayed = replay::replay_condition_for_test(
        0,
        Architecture::X86_64,
        &fixture.target,
        &fixture.abstracted,
        &fixture.optimized,
        derived.source,
        &derived.legalized,
    )
    .expect("independent U64 parameter-equals-zero replay");
    assert_eq!(replayed.source, fixture.condition);
    assert_eq!(replayed.shape, ScalarConditionShape::U64EqualZeroParameter);
}

#[test]
fn source_rejects_nonzero_and_swapped_immediate_grammar() {
    let mut nonzero = fixture();
    let TargetOperation::ReturnIntegerExpressionConditionalControl {
        condition: TargetBooleanExpression::IntegerEqual { right, .. },
        ..
    } = &mut nonzero.target.operation
    else {
        panic!("U64 parameter-equals-zero fixture")
    };
    let TargetIntegerExpression::Immediate { value, .. } = right.as_mut() else {
        panic!("zero immediate")
    };
    *value = IntegerValue::Unsigned(1);
    assert!(
        source::derive_condition_for_test(
            0,
            &nonzero.target,
            &nonzero.abstracted,
            &nonzero.optimized,
        )
        .is_err()
    );

    let mut swapped = fixture();
    let TargetOperation::ReturnIntegerExpressionConditionalControl {
        condition: TargetBooleanExpression::IntegerEqual { left, right, .. },
        ..
    } = &mut swapped.target.operation
    else {
        panic!("U64 parameter-equals-zero fixture")
    };
    std::mem::swap(left, right);
    assert!(
        source::derive_condition_for_test(
            0,
            &swapped.target,
            &swapped.abstracted,
            &swapped.optimized,
        )
        .is_err()
    );
}

#[test]
fn source_rejects_signed_attached_and_block_parameter_grammar() {
    let assert_rejected = |fixture: Fixture| {
        assert!(
            source::derive_condition_for_test(
                0,
                &fixture.target,
                &fixture.abstracted,
                &fixture.optimized,
            )
            .is_err()
        );
    };

    let mut signed = fixture();
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let signed_scalar = ScalarType::Integer(i64_type);
    signed.abstracted.parameters[0].scalar_type = signed_scalar;
    signed.optimized.parameters[0].scalar_type = signed_scalar;
    let AbstractOperation::IntegerConstant { scalar_type, .. } =
        &mut signed.abstracted.operations[0]
    else {
        unreachable!("zero constant")
    };
    *scalar_type = signed_scalar;
    let AbstractOperation::IntegerConstant { scalar_type, .. } =
        &mut signed.optimized.blocks[0].nodes[0].operation
    else {
        unreachable!("zero constant")
    };
    *scalar_type = signed_scalar;
    signed.optimized.blocks[0].nodes[0].definitions[0].scalar_type = signed_scalar;
    let TargetOperation::ReturnIntegerExpressionConditionalControl {
        condition: TargetBooleanExpression::IntegerEqual { scalar_type, .. },
        ..
    } = &mut signed.target.operation
    else {
        unreachable!("equal-zero target")
    };
    *scalar_type = i64_type;
    assert_rejected(signed);

    let mut attached = fixture();
    let attachment = id::<semantic_vocabulary::StructuralTypeId>(95);
    attached.target.attachment = Some(attachment);
    attached.abstracted.attachment = Some(attachment);
    attached.optimized.attachment = Some(attachment);
    assert_rejected(attached);

    let mut block_parameter = fixture();
    let block = block_parameter.optimized.entry;
    let value = id::<ValueId>(96);
    let scalar_type = block_parameter.optimized.parameters[0].scalar_type;
    let parameter = AbstractParameter { value, scalar_type };
    block_parameter
        .abstracted
        .block_entries
        .push(abstract_operations::AbstractBlockEntry {
            block,
            parameters: vec![parameter],
            operation_offset: 0,
        });
    block_parameter.optimized.blocks[0]
        .parameters
        .push(ValueDefinition {
            value,
            scalar_type,
            site: ValueDefinitionSite::BlockParameter { block, position: 0 },
        });
    assert_rejected(block_parameter);
}

#[test]
fn source_rejects_boolean_control_nested_beyond_the_admitted_not_equal_form() {
    let mut nested = fixture();
    let intermediate = id::<ValueId>(97);
    let boolean_not = id::<OperationId>(98);
    let entry = nested.optimized.entry;
    let TargetOperation::ReturnIntegerExpressionConditionalControl { condition, .. } =
        &mut nested.target.operation
    else {
        unreachable!("equal-zero target")
    };
    let TargetBooleanExpression::IntegerEqual {
        psi_operation,
        scalar_type,
        left,
        right,
    } = condition.clone()
    else {
        unreachable!("equal-zero condition")
    };
    *condition = TargetBooleanExpression::Not {
        psi_operation: boolean_not,
        operand: Box::new(TargetBooleanExpression::IntegerEqual {
            psi_operation,
            scalar_type,
            left,
            right,
        }),
    };
    let AbstractOperation::IntegerEqual { result, .. } = &mut nested.abstracted.operations[1]
    else {
        unreachable!("integer equality")
    };
    *result = intermediate;
    nested
        .abstracted
        .operations
        .push(AbstractOperation::BooleanNot {
            psi_operation: boolean_not,
            result: nested.condition,
            operand: intermediate,
        });
    let equality = &mut nested.optimized.blocks[0].nodes[1];
    let AbstractOperation::IntegerEqual { result, .. } = &mut equality.operation else {
        unreachable!("integer equality")
    };
    *result = intermediate;
    equality.definitions[0].value = intermediate;
    nested.optimized.blocks[0].nodes.push(OptimizationNode {
        operation: AbstractOperation::BooleanNot {
            psi_operation: boolean_not,
            result: nested.condition,
            operand: intermediate,
        },
        provenance: vec![PsiProvenance::Operation(boolean_not)],
        fuel: vec![FuelSettlement {
            site: PsiProvenance::Operation(boolean_not),
            units: 1,
        }],
        effect: EffectLink {
            input: 2,
            output: 3,
        },
        definitions: vec![ValueDefinition {
            value: nested.condition,
            scalar_type: ScalarType::Boolean,
            site: ValueDefinitionSite::Node {
                block: entry,
                node: 2,
            },
        }],
        uses: vec![ValueUse {
            value: intermediate,
            block: entry,
            node: 2,
        }],
        successors: Vec::new(),
        ownership: Vec::new(),
    });
    let TargetOperation::ReturnIntegerExpressionConditionalControl { condition, .. } =
        &mut nested.target.operation
    else {
        unreachable!("not-equal-zero target")
    };
    *condition = TargetBooleanExpression::Not {
        psi_operation: id(99),
        operand: Box::new(condition.clone()),
    };
    assert!(source::derive_condition_for_test(
        0,
        &nested.target,
        &nested.abstracted,
        &nested.optimized,
    )
    .is_err());
}

#[test]
fn source_rejects_an_extra_entry_operation_before_control() {
    let mut extra = fixture();
    let entry = extra.optimized.entry;
    let operation = id::<OperationId>(99);
    let value = id::<ValueId>(100);
    let scalar_type = extra.optimized.parameters[0].scalar_type;
    let constant = AbstractOperation::IntegerConstant {
        psi_operation: operation,
        result: value,
        scalar_type,
        value: IntegerValue::Unsigned(1),
    };
    extra.abstracted.operations.push(constant.clone());
    extra.optimized.blocks[0].nodes.push(OptimizationNode {
        operation: constant,
        provenance: vec![PsiProvenance::Operation(operation)],
        fuel: vec![FuelSettlement {
            site: PsiProvenance::Operation(operation),
            units: 1,
        }],
        effect: EffectLink {
            input: 2,
            output: 3,
        },
        definitions: vec![ValueDefinition {
            value,
            scalar_type,
            site: ValueDefinitionSite::Node {
                block: entry,
                node: 2,
            },
        }],
        uses: Vec::new(),
        successors: Vec::new(),
        ownership: Vec::new(),
    });
    assert!(
        source::derive_condition_for_test(0, &extra.target, &extra.abstracted, &extra.optimized,)
            .is_err()
    );
}

#[test]
fn replay_rejects_zero_and_parameter_custody_corruption() {
    let fixture = fixture();
    let derived = source::derive_condition_for_test(
        0,
        &fixture.target,
        &fixture.abstracted,
        &fixture.optimized,
    )
    .expect("U64 parameter-equals-zero source condition");

    let mut corruptions = Vec::new();
    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 { operation, .. } = &mut corrupted else {
        unreachable!("U64 parameter-equals-zero custody")
    };
    *operation = id(90);
    corruptions.push(corrupted);
    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 {
        result_definition_site,
        ..
    } = &mut corrupted
    else {
        unreachable!("U64 parameter-equals-zero custody")
    };
    *result_definition_site = ValueDefinitionSite::Node {
        block: fixture.optimized.entry,
        node: 9,
    };
    corruptions.push(corrupted);
    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 { fuel, .. } = &mut corrupted else {
        unreachable!("U64 parameter-equals-zero custody")
    };
    fuel[0].units += 1;
    corruptions.push(corrupted);
    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 { zero, .. } = &mut corrupted else {
        unreachable!("U64 parameter-equals-zero custody")
    };
    zero.value = IntegerValue::Unsigned(1);
    corruptions.push(corrupted);
    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 { zero, .. } = &mut corrupted else {
        unreachable!("U64 parameter-equals-zero custody")
    };
    zero.fuel[0].units += 1;
    corruptions.push(corrupted);
    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 { zero, .. } = &mut corrupted else {
        unreachable!("U64 parameter-equals-zero custody")
    };
    zero.source_value = id(91);
    corruptions.push(corrupted);
    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 { zero, .. } = &mut corrupted else {
        unreachable!("U64 parameter-equals-zero custody")
    };
    zero.constant_operation = id(92);
    corruptions.push(corrupted);
    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 { zero, .. } = &mut corrupted else {
        unreachable!("U64 parameter-equals-zero custody")
    };
    zero.definition_site = ValueDefinitionSite::Node {
        block: fixture.optimized.entry,
        node: 9,
    };
    corruptions.push(corrupted);
    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 { parameter, .. } = &mut corrupted else {
        unreachable!("U64 parameter-equals-zero custody")
    };
    parameter.parameter_index = 1;
    corruptions.push(corrupted);
    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 { parameter, .. } = &mut corrupted else {
        unreachable!("U64 parameter-equals-zero custody")
    };
    parameter.source_value = id(93);
    corruptions.push(corrupted);
    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 { parameter, .. } = &mut corrupted else {
        unreachable!("U64 parameter-equals-zero custody")
    };
    parameter.register = MachineRegister::X86Rsi;
    corruptions.push(corrupted);
    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 { parameter, .. } = &mut corrupted else {
        unreachable!("U64 parameter-equals-zero custody")
    };
    parameter.definition_site = ValueDefinitionSite::FunctionParameter(1);
    corruptions.push(corrupted);

    for corrupted in corruptions {
        assert_eq!(
            replay::replay_condition_for_test(
                0,
                Architecture::X86_64,
                &fixture.target,
                &fixture.abstracted,
                &fixture.optimized,
                derived.source,
                &corrupted,
            )
            .map(|_| ()),
            Err(LegalizationError::NonCanonicalLegalizedPlan)
        );
    }

    assert_eq!(
        replay::replay_condition_for_test(
            0,
            Architecture::X86_64,
            &fixture.target,
            &fixture.abstracted,
            &fixture.optimized,
            id(94),
            &derived.legalized,
        )
        .map(|_| ()),
        Err(LegalizationError::NonCanonicalLegalizedPlan)
    );
}
