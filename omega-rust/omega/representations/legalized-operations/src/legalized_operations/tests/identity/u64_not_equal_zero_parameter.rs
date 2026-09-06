//! Append-only identity evidence for exact U64 parameter-not-equals-zero custody.

use super::*;

fn plan() -> LegalizedOperationPlan {
    let mut plan = call_aware_plan();
    plan.structural_unit_functions.clear();
    let machine = id(20);
    let entry = id::<BlockId>(21);
    let true_block = id::<BlockId>(22);
    let false_block = id::<BlockId>(23);
    let parameter = id::<ValueId>(24);
    let zero = id::<ValueId>(25);
    let equality_result = id::<ValueId>(26);
    let condition = id::<ValueId>(27);
    let zero_operation = id::<OperationId>(28);
    let equality_operation = id::<OperationId>(29);
    let boolean_not_operation = id::<OperationId>(30);
    let true_edge = id::<EdgeId>(31);
    let false_edge = id::<EdgeId>(32);
    let true_return = id::<EdgeId>(33);
    let false_return = id::<EdgeId>(34);
    let operation_fuel = |operation| {
        vec![FuelSettlement {
            site: PsiProvenance::Operation(operation),
            units: 1,
        }]
    };
    let leaf = |source_value, value, operation, block, edge| LegalizedLeaf {
        return_edge: edge,
        source_value,
        return_fuel: vec![FuelSettlement {
            site: PsiProvenance::Edge(edge),
            units: 1,
        }],
        value: LegalizedLeafValue::Immediate {
            value: IntegerValue::Unsigned(value),
            constant_operation: operation,
            definition_site: optimization_unit::ValueDefinitionSite::Node { block, node: 0 },
            constant_fuel: operation_fuel(operation),
        },
    };
    let true_value = id::<ValueId>(35);
    let false_value = id::<ValueId>(36);
    let true_constant = id::<OperationId>(37);
    let false_constant = id::<OperationId>(38);
    plan.functions.push(LegalizedFunction::Conditional(
        LegalizedConditionalFunction {
            machine,
            attachment: None,
            provenance: TerminalPsiProvenance {
                operations: vec![
                    zero_operation,
                    equality_operation,
                    boolean_not_operation,
                    true_constant,
                    false_constant,
                ],
                edges: vec![true_edge, false_edge, true_return, false_return],
            },
            recipe: LegalizationRecipe::ReturnU64NotEqualZeroParameterConditionalV1,
            condition_source: condition,
            condition: LegalizedCondition::U64NotEqualZeroParameterV1 {
                equality_operation,
                equality_result,
                equality_result_definition_site: optimization_unit::ValueDefinitionSite::Node {
                    block: entry,
                    node: 1,
                },
                equality_fuel: operation_fuel(equality_operation),
                boolean_not_operation,
                boolean_not_result: condition,
                boolean_not_result_definition_site: optimization_unit::ValueDefinitionSite::Node {
                    block: entry,
                    node: 2,
                },
                boolean_not_fuel: operation_fuel(boolean_not_operation),
                parameter: LegalizedConditionParameter {
                    source_value: parameter,
                    parameter_index: 0,
                    register: MachineRegister::X86Rdi,
                    definition_site: optimization_unit::ValueDefinitionSite::FunctionParameter(0),
                },
                zero: LegalizedImmediate {
                    source_value: zero,
                    value: IntegerValue::Unsigned(0),
                    constant_operation: zero_operation,
                    definition_site: optimization_unit::ValueDefinitionSite::Node {
                        block: entry,
                        node: 0,
                    },
                    fuel: operation_fuel(zero_operation),
                },
            },
            entry_block: entry,
            true_block,
            false_block,
            branch_true_edge: true_edge,
            branch_false_edge: false_edge,
            branch_true_fuel: vec![FuelSettlement {
                site: PsiProvenance::Edge(true_edge),
                units: 1,
            }],
            branch_false_fuel: vec![FuelSettlement {
                site: PsiProvenance::Edge(false_edge),
                units: 1,
            }],
            branch_true_bindings: Vec::new(),
            branch_false_bindings: Vec::new(),
            when_true: leaf(true_value, 7, true_constant, true_block, true_return),
            when_false: leaf(false_value, 9, false_constant, false_block, false_return),
        },
    ));
    plan
}

fn assert_condition_corruption(
    plan: &LegalizedOperationPlan,
    identity: LegalizedOperationPlanIdentity,
    corrupt: impl FnOnce(&mut LegalizedCondition),
) {
    let mut corrupted = plan.clone();
    corrupt(&mut corrupted.functions[0].conditional_mut().condition);
    assert_identity_drift(identity, &corrupted);
}

#[test]
fn identity_binds_not_equal_zero_custody_append_only() {
    let plan = plan();
    let identity = legalized_operation_plan_identity(&plan);
    assert_ne!(
        identity,
        legalized_operation_plan_identity_v19_legacy(&plan),
        "V19 predates exact U64 parameter-not-equals-zero custody"
    );

    let mut corrupted = plan.clone();
    corrupted.functions[0].conditional_mut().recipe =
        LegalizationRecipe::ReturnU64EqualZeroParameterConditionalV1;
    assert_identity_drift(identity, &corrupted);
    let mut corrupted = plan.clone();
    corrupted.functions[0].conditional_mut().condition_source = id(99);
    assert_identity_drift(identity, &corrupted);

    assert_condition_corruption(&plan, identity, |condition| {
        let LegalizedCondition::U64NotEqualZeroParameterV1 {
            equality_operation, ..
        } = condition
        else {
            unreachable!()
        };
        *equality_operation = id(99);
    });
    assert_condition_corruption(&plan, identity, |condition| {
        let LegalizedCondition::U64NotEqualZeroParameterV1 {
            equality_result, ..
        } = condition
        else {
            unreachable!()
        };
        *equality_result = id(99);
    });
    assert_condition_corruption(&plan, identity, |condition| {
        let LegalizedCondition::U64NotEqualZeroParameterV1 {
            equality_result_definition_site,
            ..
        } = condition
        else {
            unreachable!()
        };
        *equality_result_definition_site = optimization_unit::ValueDefinitionSite::Node {
            block: id(99),
            node: 1,
        };
    });
    assert_condition_corruption(&plan, identity, |condition| {
        let LegalizedCondition::U64NotEqualZeroParameterV1 { equality_fuel, .. } = condition else {
            unreachable!()
        };
        equality_fuel[0].units += 1;
    });
    assert_condition_corruption(&plan, identity, |condition| {
        let LegalizedCondition::U64NotEqualZeroParameterV1 {
            boolean_not_operation,
            ..
        } = condition
        else {
            unreachable!()
        };
        *boolean_not_operation = id(99);
    });
    assert_condition_corruption(&plan, identity, |condition| {
        let LegalizedCondition::U64NotEqualZeroParameterV1 {
            boolean_not_result, ..
        } = condition
        else {
            unreachable!()
        };
        *boolean_not_result = id(99);
    });
    assert_condition_corruption(&plan, identity, |condition| {
        let LegalizedCondition::U64NotEqualZeroParameterV1 {
            boolean_not_result_definition_site,
            ..
        } = condition
        else {
            unreachable!()
        };
        *boolean_not_result_definition_site = optimization_unit::ValueDefinitionSite::Node {
            block: id(99),
            node: 2,
        };
    });
    assert_condition_corruption(&plan, identity, |condition| {
        let LegalizedCondition::U64NotEqualZeroParameterV1 {
            boolean_not_fuel, ..
        } = condition
        else {
            unreachable!()
        };
        boolean_not_fuel[0].units += 1;
    });

    for corrupt in [
        |parameter: &mut LegalizedConditionParameter| parameter.source_value = id(99),
        |parameter: &mut LegalizedConditionParameter| parameter.parameter_index = 1,
        |parameter: &mut LegalizedConditionParameter| parameter.register = MachineRegister::X86Rsi,
        |parameter: &mut LegalizedConditionParameter| {
            parameter.definition_site = optimization_unit::ValueDefinitionSite::FunctionParameter(1)
        },
    ] {
        assert_condition_corruption(&plan, identity, |condition| {
            let LegalizedCondition::U64NotEqualZeroParameterV1 { parameter, .. } = condition else {
                unreachable!()
            };
            corrupt(parameter);
        });
    }

    for corrupt in [
        |zero: &mut LegalizedImmediate| zero.source_value = id(99),
        |zero: &mut LegalizedImmediate| zero.value = IntegerValue::Unsigned(1),
        |zero: &mut LegalizedImmediate| zero.constant_operation = id(99),
        |zero: &mut LegalizedImmediate| {
            zero.definition_site = optimization_unit::ValueDefinitionSite::Node {
                block: id(99),
                node: 0,
            }
        },
        |zero: &mut LegalizedImmediate| zero.fuel[0].units += 1,
    ] {
        assert_condition_corruption(&plan, identity, |condition| {
            let LegalizedCondition::U64NotEqualZeroParameterV1 { zero, .. } = condition else {
                unreachable!()
            };
            corrupt(zero);
        });
    }
}
