//! Exact U64 parameter-not-equals-zero producer/replay coverage.

use super::*;

fn fixture() -> Fixture {
    let mut fixture = super::fixture();
    let zero = fixture.right;
    let zero_operation = id::<OperationId>(16);
    let equality_result = id::<ValueId>(17);
    let boolean_not_operation = id::<OperationId>(18);
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
        result: equality_result,
        left: fixture.left,
        right: zero,
    };
    let boolean_not = AbstractOperation::BooleanNot {
        psi_operation: boolean_not_operation,
        result: fixture.condition,
        operand: equality_result,
    };
    fixture.abstracted.parameters.truncate(1);
    fixture.abstracted.operations =
        vec![zero_constant.clone(), equality.clone(), boolean_not.clone()];
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
                value: equality_result,
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
        OptimizationNode {
            operation: boolean_not,
            provenance: vec![PsiProvenance::Operation(boolean_not_operation)],
            fuel: vec![FuelSettlement {
                site: PsiProvenance::Operation(boolean_not_operation),
                units: 1,
            }],
            effect: EffectLink {
                input: 2,
                output: 3,
            },
            definitions: vec![ValueDefinition {
                value: fixture.condition,
                scalar_type: ScalarType::Boolean,
                site: ValueDefinitionSite::Node {
                    block: entry,
                    node: 2,
                },
            }],
            uses: vec![ValueUse {
                value: equality_result,
                block: entry,
                node: 2,
            }],
            successors: Vec::new(),
            ownership: Vec::new(),
        },
    ];
    let TargetOperation::ReturnIntegerExpressionConditionalControl { condition, .. } =
        &mut fixture.target.operation
    else {
        unreachable!("comparison fixture")
    };
    *condition = TargetBooleanExpression::Not {
        psi_operation: boolean_not_operation,
        operand: Box::new(TargetBooleanExpression::IntegerEqual {
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
        }),
    };
    fixture
}

fn assert_source_rejected(fixture: &Fixture) {
    assert!(
        source::derive_condition_for_test(
            0,
            &fixture.target,
            &fixture.abstracted,
            &fixture.optimized,
        )
        .is_err()
    );
}

fn assert_corruption_rejected(
    fixture: &Fixture,
    proposed_source: ValueId,
    proposed: &LegalizedCondition,
) {
    assert_eq!(
        replay::replay_condition_for_test(
            0,
            Architecture::X86_64,
            &fixture.target,
            &fixture.abstracted,
            &fixture.optimized,
            proposed_source,
            proposed,
        )
        .map(|_| ()),
        Err(LegalizationError::NonCanonicalLegalizedPlan)
    );
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
    .expect("U64 parameter-not-equals-zero source condition");
    assert_eq!(derived.source, fixture.condition);
    assert_eq!(
        derived.shape,
        ScalarConditionShape::U64NotEqualZeroParameter
    );
    assert_eq!(derived.conditional_node_index, 3);
    let LegalizedCondition::U64NotEqualZeroParameterV1 {
        equality_operation,
        equality_result,
        boolean_not_operation,
        boolean_not_result,
        parameter,
        zero,
        ..
    } = &derived.legalized
    else {
        panic!("U64 parameter-not-equals-zero custody")
    };
    assert_eq!(*equality_operation, fixture.operation);
    assert_eq!(*equality_result, id(17));
    assert_eq!(*boolean_not_operation, id(18));
    assert_eq!(*boolean_not_result, fixture.condition);
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
    .expect("independent U64 parameter-not-equals-zero replay");
    assert_eq!(replayed.source, fixture.condition);
    assert_eq!(
        replayed.shape,
        ScalarConditionShape::U64NotEqualZeroParameter
    );
    assert_eq!(replayed.conditional_node_index, 3);
}

#[test]
fn source_rejects_nonzero_swapped_signed_and_extra_parameter_grammar() {
    let mut nonzero = fixture();
    let TargetOperation::ReturnIntegerExpressionConditionalControl {
        condition: TargetBooleanExpression::Not { operand, .. },
        ..
    } = &mut nonzero.target.operation
    else {
        unreachable!()
    };
    let TargetBooleanExpression::IntegerEqual { right, .. } = operand.as_mut() else {
        unreachable!()
    };
    let TargetIntegerExpression::Immediate { value, .. } = right.as_mut() else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(1);
    assert_source_rejected(&nonzero);

    let mut swapped = fixture();
    let TargetOperation::ReturnIntegerExpressionConditionalControl {
        condition: TargetBooleanExpression::Not { operand, .. },
        ..
    } = &mut swapped.target.operation
    else {
        unreachable!()
    };
    let TargetBooleanExpression::IntegerEqual { left, right, .. } = operand.as_mut() else {
        unreachable!()
    };
    std::mem::swap(left, right);
    assert_source_rejected(&swapped);

    let mut signed = fixture();
    let TargetOperation::ReturnIntegerExpressionConditionalControl {
        condition: TargetBooleanExpression::Not { operand, .. },
        ..
    } = &mut signed.target.operation
    else {
        unreachable!()
    };
    let TargetBooleanExpression::IntegerEqual { scalar_type, .. } = operand.as_mut() else {
        unreachable!()
    };
    *scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    assert_source_rejected(&signed);

    let mut extra_parameter = fixture();
    extra_parameter
        .abstracted
        .parameters
        .push(AbstractParameter {
            value: id(90),
            scalar_type: extra_parameter.abstracted.parameters[0].scalar_type,
        });
    extra_parameter.optimized.parameters.push(ValueDefinition {
        value: id(90),
        scalar_type: extra_parameter.optimized.parameters[0].scalar_type,
        site: ValueDefinitionSite::FunctionParameter(1),
    });
    assert_source_rejected(&extra_parameter);
}

#[test]
fn source_rejects_attachment_block_parameter_nested_and_extra_operation_grammar() {
    let mut attached = fixture();
    let attachment = id::<semantic_vocabulary::StructuralTypeId>(91);
    attached.target.attachment = Some(attachment);
    attached.abstracted.attachment = Some(attachment);
    attached.optimized.attachment = Some(attachment);
    assert_source_rejected(&attached);

    let mut block_parameter = fixture();
    let block = block_parameter.optimized.entry;
    let value = id::<ValueId>(92);
    let scalar_type = block_parameter.optimized.parameters[0].scalar_type;
    block_parameter
        .abstracted
        .block_entries
        .push(abstract_operations::AbstractBlockEntry {
            block,
            parameters: vec![AbstractParameter { value, scalar_type }],
            operation_offset: 0,
        });
    block_parameter.optimized.blocks[0]
        .parameters
        .push(ValueDefinition {
            value,
            scalar_type,
            site: ValueDefinitionSite::BlockParameter { block, position: 0 },
        });
    assert_source_rejected(&block_parameter);

    let mut nested = fixture();
    let TargetOperation::ReturnIntegerExpressionConditionalControl { condition, .. } =
        &mut nested.target.operation
    else {
        unreachable!()
    };
    *condition = TargetBooleanExpression::Not {
        psi_operation: id(93),
        operand: Box::new(condition.clone()),
    };
    assert_source_rejected(&nested);

    let mut extra = fixture();
    let entry = extra.optimized.entry;
    let operation = id::<OperationId>(94);
    let value = id::<ValueId>(95);
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
            input: 3,
            output: 4,
        },
        definitions: vec![ValueDefinition {
            value,
            scalar_type,
            site: ValueDefinitionSite::Node {
                block: entry,
                node: 3,
            },
        }],
        uses: Vec::new(),
        successors: Vec::new(),
        ownership: Vec::new(),
    });
    assert_source_rejected(&extra);
}

#[test]
fn replay_rejects_every_condition_custody_corruption() {
    let fixture = fixture();
    let derived = source::derive_condition_for_test(
        0,
        &fixture.target,
        &fixture.abstracted,
        &fixture.optimized,
    )
    .expect("U64 parameter-not-equals-zero source condition");

    macro_rules! reject_corruption {
        ($field:ident, $body:expr) => {{
            let mut corrupted = derived.legalized.clone();
            let LegalizedCondition::U64NotEqualZeroParameterV1 { $field, .. } = &mut corrupted
            else {
                unreachable!()
            };
            $body($field);
            assert_corruption_rejected(&fixture, derived.source, &corrupted);
        }};
    }

    reject_corruption!(equality_operation, |value: &mut OperationId| *value =
        id(96));
    reject_corruption!(equality_result, |value: &mut ValueId| *value = id(96));
    reject_corruption!(
        equality_result_definition_site,
        |value: &mut ValueDefinitionSite| {
            *value = ValueDefinitionSite::Node {
                block: fixture.optimized.entry,
                node: 9,
            }
        }
    );
    reject_corruption!(equality_fuel, |value: &mut Vec<FuelSettlement>| value[0]
        .units +=
        1);
    reject_corruption!(boolean_not_operation, |value: &mut OperationId| *value =
        id(96));
    reject_corruption!(boolean_not_result, |value: &mut ValueId| *value = id(96));
    reject_corruption!(
        boolean_not_result_definition_site,
        |value: &mut ValueDefinitionSite| {
            *value = ValueDefinitionSite::Node {
                block: fixture.optimized.entry,
                node: 9,
            }
        }
    );
    reject_corruption!(boolean_not_fuel, |value: &mut Vec<FuelSettlement>| value
        [0]
    .units +=
        1);
    reject_corruption!(parameter, |value: &mut LegalizedConditionParameter| value
        .source_value =
        id(96));
    reject_corruption!(parameter, |value: &mut LegalizedConditionParameter| value
        .parameter_index =
        1);
    reject_corruption!(parameter, |value: &mut LegalizedConditionParameter| value
        .register =
        MachineRegister::X86Rsi);
    reject_corruption!(parameter, |value: &mut LegalizedConditionParameter| {
        value.definition_site = ValueDefinitionSite::FunctionParameter(1)
    });
    reject_corruption!(
        zero,
        |value: &mut legalized_operations::LegalizedImmediate| value.source_value = id(96)
    );
    reject_corruption!(
        zero,
        |value: &mut legalized_operations::LegalizedImmediate| value.value =
            IntegerValue::Unsigned(1)
    );
    reject_corruption!(
        zero,
        |value: &mut legalized_operations::LegalizedImmediate| value.constant_operation = id(96)
    );
    reject_corruption!(
        zero,
        |value: &mut legalized_operations::LegalizedImmediate| {
            value.definition_site = ValueDefinitionSite::Node {
                block: fixture.optimized.entry,
                node: 9,
            }
        }
    );
    reject_corruption!(
        zero,
        |value: &mut legalized_operations::LegalizedImmediate| value.fuel[0].units += 1
    );

    assert_corruption_rejected(&fixture, id(97), &derived.legalized);
}
