//! Append-only identity evidence for exact U64 parameter-equals-zero custody.

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
    let condition = id::<ValueId>(26);
    let zero_operation = id::<OperationId>(27);
    let comparison = id::<OperationId>(28);
    let true_edge = id::<EdgeId>(29);
    let false_edge = id::<EdgeId>(30);
    let true_return = id::<EdgeId>(31);
    let false_return = id::<EdgeId>(32);
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
    let true_value = id::<ValueId>(33);
    let false_value = id::<ValueId>(34);
    let true_constant = id::<OperationId>(35);
    let false_constant = id::<OperationId>(36);
    plan.functions.push(LegalizedFunction::Conditional(
        LegalizedConditionalFunction {
            machine,
            attachment: None,
            provenance: TerminalPsiProvenance {
                operations: vec![zero_operation, comparison, true_constant, false_constant],
                edges: vec![true_edge, false_edge, true_return, false_return],
            },
            recipe: LegalizationRecipe::ReturnU64EqualZeroParameterConditionalV1,
            condition_source: condition,
            condition: LegalizedCondition::U64EqualZeroParameterV1 {
                operation: comparison,
                result_definition_site: optimization_unit::ValueDefinitionSite::Node {
                    block: entry,
                    node: 1,
                },
                fuel: operation_fuel(comparison),
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

#[test]
fn identity_binds_zero_and_parameter_custody_append_only() {
    let plan = plan();
    let identity = legalized_operation_plan_identity(&plan);
    assert_ne!(
        identity,
        legalized_operation_plan_identity_v18_legacy(&plan),
        "V18 predates exact U64 parameter-equals-zero custody"
    );

    let mut corruptions = Vec::new();
    let mut corrupted = plan.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 { operation, .. } =
        &mut corrupted.functions[0].conditional_mut().condition
    else {
        unreachable!("U64 parameter-equals-zero condition")
    };
    *operation = id(99);
    corruptions.push(corrupted);
    let mut corrupted = plan.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 {
        result_definition_site,
        fuel,
        ..
    } = &mut corrupted.functions[0].conditional_mut().condition
    else {
        unreachable!("U64 parameter-equals-zero condition")
    };
    *result_definition_site = optimization_unit::ValueDefinitionSite::Node {
        block: id(99),
        node: 1,
    };
    fuel[0].units += 1;
    corruptions.push(corrupted);
    let mut corrupted = plan.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 { parameter, .. } =
        &mut corrupted.functions[0].conditional_mut().condition
    else {
        unreachable!("U64 parameter-equals-zero condition")
    };
    parameter.parameter_index = 1;
    corruptions.push(corrupted);
    let mut corrupted = plan.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 { parameter, .. } =
        &mut corrupted.functions[0].conditional_mut().condition
    else {
        unreachable!("U64 parameter-equals-zero condition")
    };
    parameter.register = MachineRegister::X86Rsi;
    corruptions.push(corrupted);
    let mut corrupted = plan.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 { zero, .. } =
        &mut corrupted.functions[0].conditional_mut().condition
    else {
        unreachable!("U64 parameter-equals-zero condition")
    };
    zero.value = IntegerValue::Unsigned(1);
    corruptions.push(corrupted);
    let mut corrupted = plan.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 { zero, .. } =
        &mut corrupted.functions[0].conditional_mut().condition
    else {
        unreachable!("U64 parameter-equals-zero condition")
    };
    zero.constant_operation = id(99);
    corruptions.push(corrupted);
    let mut corrupted = plan.clone();
    let LegalizedCondition::U64EqualZeroParameterV1 { zero, .. } =
        &mut corrupted.functions[0].conditional_mut().condition
    else {
        unreachable!("U64 parameter-equals-zero condition")
    };
    zero.fuel[0].units += 1;
    corruptions.push(corrupted);

    for corrupted in corruptions {
        assert_identity_drift(identity, &corrupted);
    }
}
