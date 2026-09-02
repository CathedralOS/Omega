//! Focused producer/replay coverage for exact runtime comparison conditions.

use std::collections::BTreeSet;

use omega_abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractParameter, AbstractResult,
};
use omega_legalized_operations::{LegalizedCondition, LegalizedConditionParameter};
use omega_optimization_unit::{
    EffectLink, FuelSettlement, OptimizationBlock, OptimizationNode, PsiOptimizationFunction,
    PsiProvenance, ValueDefinition, ValueDefinitionSite, ValueUse,
};
use omega_target::Architecture;
use omega_target_operations::{
    MachineRegister, ScalarParameterLocation, TargetBooleanExpression, TargetConditionalIntegerArm,
    TargetFunction, TargetIntegerControl, TargetIntegerExpression, TargetOperation,
    TerminalPsiProvenance,
};
use psi_core::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, ScalarType,
    ValueId,
};

use super::catalog::ScalarConditionShape;
use super::{LegalizationError, replay, source};

mod i64_less_than;

struct Fixture {
    target: TargetFunction,
    abstracted: AbstractFunction,
    optimized: PsiOptimizationFunction,
    operation: OperationId,
    condition: ValueId,
    left: ValueId,
    right: ValueId,
}

fn id<T: psi_core::PsiSemanticId>(raw: u64) -> T {
    T::new(raw).expect("nonzero fixture identity")
}

fn fixture() -> Fixture {
    let machine = id::<MachineId>(1);
    let entry = id::<BlockId>(2);
    let left = id::<ValueId>(3);
    let right = id::<ValueId>(4);
    let condition = id::<ValueId>(5);
    let result = id::<ValueId>(6);
    let operation = id::<OperationId>(7);
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let scalar_type = ScalarType::Integer(u64_type);
    let parameters = vec![
        AbstractParameter {
            value: left,
            scalar_type,
        },
        AbstractParameter {
            value: right,
            scalar_type,
        },
    ];
    let comparison = AbstractOperation::IntegerLessThan {
        psi_operation: operation,
        result: condition,
        left,
        right,
    };
    let result_declaration = AbstractResult {
        value: result,
        scalar_type,
    };
    let abstracted = AbstractFunction {
        machine,
        attachment: None,
        entry,
        parameters: parameters.clone(),
        structural_parameters: Vec::new(),
        result: AbstractFunctionResult::Scalar(result_declaration),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: Vec::new(),
        operations: vec![comparison.clone()],
    };
    let definitions = vec![ValueDefinition {
        value: condition,
        scalar_type: ScalarType::Boolean,
        site: ValueDefinitionSite::Node {
            block: entry,
            node: 0,
        },
    }];
    let optimized = PsiOptimizationFunction {
        machine,
        attachment: None,
        entry,
        parameters: vec![
            ValueDefinition {
                value: left,
                scalar_type,
                site: ValueDefinitionSite::FunctionParameter(0),
            },
            ValueDefinition {
                value: right,
                scalar_type,
                site: ValueDefinitionSite::FunctionParameter(1),
            },
        ],
        structural_parameters: Vec::new(),
        structural_places: Vec::new(),
        result: AbstractFunctionResult::Scalar(result_declaration),
        declared_places: BTreeSet::new(),
        entry_claim_declarations: Vec::new(),
        content_entry_claims: Vec::new(),
        verified_contract: None,
        evidence_contract_lanes: Vec::new(),
        entry_claims: BTreeSet::new(),
        published_service_ceiling: Vec::new(),
        facts: Vec::new(),
        blocks: vec![OptimizationBlock {
            id: entry,
            parameters: Vec::new(),
            nodes: vec![OptimizationNode {
                operation: comparison,
                provenance: vec![PsiProvenance::Operation(operation)],
                fuel: vec![FuelSettlement {
                    site: PsiProvenance::Operation(operation),
                    units: 1,
                }],
                effect: EffectLink {
                    input: 0,
                    output: 1,
                },
                definitions,
                uses: vec![
                    ValueUse {
                        value: left,
                        block: entry,
                        node: 0,
                    },
                    ValueUse {
                        value: right,
                        block: entry,
                        node: 0,
                    },
                ],
                successors: Vec::new(),
                ownership: Vec::new(),
            }],
        }],
    };
    let return_arm = |edge_raw, return_raw, value_raw, literal| TargetConditionalIntegerArm {
        psi_edge: id::<EdgeId>(edge_raw),
        control: Box::new(TargetIntegerControl::Return {
            psi_return_edge: id::<EdgeId>(return_raw),
            source_value: id::<ValueId>(value_raw),
            expression: TargetIntegerExpression::Immediate {
                source_value: id::<ValueId>(value_raw),
                value: IntegerValue::Unsigned(literal),
            },
        }),
    };
    let target = TargetFunction {
        machine,
        attachment: None,
        fixed_integer_scalar_abi: None,
        provenance: TerminalPsiProvenance::default(),
        operation: TargetOperation::ReturnIntegerExpressionConditionalControl {
            condition_source: condition,
            condition: TargetBooleanExpression::IntegerLessThan {
                psi_operation: operation,
                scalar_type: u64_type,
                left: Box::new(TargetIntegerExpression::Parameter {
                    source_value: left,
                    parameter_index: 0,
                    location: ScalarParameterLocation::Register(MachineRegister::X86Rdi),
                }),
                right: Box::new(TargetIntegerExpression::Parameter {
                    source_value: right,
                    parameter_index: 1,
                    location: ScalarParameterLocation::Register(MachineRegister::X86Rsi),
                }),
            },
            scalar_type: u64_type,
            when_true: return_arm(8, 9, 10, 7),
            when_false: return_arm(11, 12, 13, 9),
        },
    };
    Fixture {
        target,
        abstracted,
        optimized,
        operation,
        condition,
        left,
        right,
    }
}

fn less_or_equal_fixture() -> Fixture {
    let mut fixture = fixture();
    let TargetOperation::ReturnIntegerExpressionConditionalControl { condition, .. } =
        &mut fixture.target.operation
    else {
        unreachable!("comparison fixture")
    };
    let TargetBooleanExpression::IntegerLessThan {
        psi_operation,
        scalar_type,
        left,
        right,
    } = condition
    else {
        unreachable!("comparison fixture")
    };
    *condition = TargetBooleanExpression::IntegerLessOrEqual {
        psi_operation: *psi_operation,
        scalar_type: *scalar_type,
        left: left.clone(),
        right: right.clone(),
    };
    let AbstractOperation::IntegerLessThan {
        psi_operation,
        result,
        left,
        right,
    } = fixture.abstracted.operations[0].clone()
    else {
        unreachable!("comparison fixture")
    };
    fixture.abstracted.operations[0] = AbstractOperation::IntegerLessOrEqual {
        psi_operation,
        result,
        left,
        right,
    };
    let AbstractOperation::IntegerLessThan {
        psi_operation,
        result,
        left,
        right,
    } = fixture.optimized.blocks[0].nodes[0].operation.clone()
    else {
        unreachable!("comparison fixture")
    };
    fixture.optimized.blocks[0].nodes[0].operation = AbstractOperation::IntegerLessOrEqual {
        psi_operation,
        result,
        left,
        right,
    };
    fixture
}

fn not_equal_fixture() -> Fixture {
    let mut fixture = fixture();
    let equality_result = id::<ValueId>(14);
    let boolean_not_operation = id::<OperationId>(15);
    let TargetOperation::ReturnIntegerExpressionConditionalControl { condition, .. } =
        &mut fixture.target.operation
    else {
        unreachable!("comparison fixture")
    };
    let TargetBooleanExpression::IntegerLessThan {
        psi_operation,
        scalar_type,
        left,
        right,
    } = condition
    else {
        unreachable!("comparison fixture")
    };
    *condition = TargetBooleanExpression::Not {
        psi_operation: boolean_not_operation,
        operand: Box::new(TargetBooleanExpression::IntegerEqual {
            psi_operation: *psi_operation,
            scalar_type: *scalar_type,
            left: left.clone(),
            right: right.clone(),
        }),
    };

    let AbstractOperation::IntegerLessThan {
        psi_operation,
        left,
        right,
        ..
    } = fixture.abstracted.operations[0].clone()
    else {
        unreachable!("comparison fixture")
    };
    fixture.abstracted.operations = vec![
        AbstractOperation::IntegerEqual {
            psi_operation,
            result: equality_result,
            left,
            right,
        },
        AbstractOperation::BooleanNot {
            psi_operation: boolean_not_operation,
            result: fixture.condition,
            operand: equality_result,
        },
    ];

    let equality_node = &mut fixture.optimized.blocks[0].nodes[0];
    equality_node.operation = fixture.abstracted.operations[0].clone();
    equality_node.definitions[0].value = equality_result;
    fixture.optimized.blocks[0].nodes.push(OptimizationNode {
        operation: fixture.abstracted.operations[1].clone(),
        provenance: vec![PsiProvenance::Operation(boolean_not_operation)],
        fuel: vec![FuelSettlement {
            site: PsiProvenance::Operation(boolean_not_operation),
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
                block: fixture.optimized.entry,
                node: 1,
            },
        }],
        uses: vec![ValueUse {
            value: equality_result,
            block: fixture.optimized.entry,
            node: 1,
        }],
        successors: Vec::new(),
        ownership: Vec::new(),
    });
    fixture
}

#[test]
fn strict_less_than_condition_is_produced_and_independently_replayed() {
    let fixture = fixture();
    let derived = source::derive_condition_for_test(
        0,
        &fixture.target,
        &fixture.abstracted,
        &fixture.optimized,
    )
    .expect("strict less-than source condition");
    assert_eq!(derived.source, fixture.condition);
    assert_eq!(
        derived.shape,
        ScalarConditionShape::IntegerLessThanU64Parameters
    );
    let LegalizedCondition::IntegerLessThanParametersV1 {
        operation,
        left,
        right,
        fuel,
        ..
    } = &derived.legalized
    else {
        panic!("strict less-than custody")
    };
    assert_eq!(*operation, fixture.operation);
    assert_eq!(left.source_value, fixture.left);
    assert_eq!(left.parameter_index, 0);
    assert_eq!(right.source_value, fixture.right);
    assert_eq!(right.parameter_index, 1);
    assert_eq!(fuel.len(), 1);

    let replayed = replay::replay_condition_for_test(
        0,
        Architecture::X86_64,
        &fixture.target,
        &fixture.abstracted,
        &fixture.optimized,
        derived.source,
        &derived.legalized,
    )
    .expect("independent strict less-than replay");
    assert_eq!(replayed.source, fixture.condition);
    assert_eq!(
        replayed.shape,
        ScalarConditionShape::IntegerLessThanU64Parameters
    );
}

#[test]
fn strict_less_than_rejects_reflexive_and_order_corruption() {
    let mut reflexive = fixture();
    let TargetOperation::ReturnIntegerExpressionConditionalControl {
        condition: TargetBooleanExpression::IntegerLessThan { left, right, .. },
        ..
    } = &mut reflexive.target.operation
    else {
        panic!("strict less-than fixture")
    };
    *right = left.clone();
    let AbstractOperation::IntegerLessThan {
        left: abstract_left,
        right: abstract_right,
        ..
    } = &mut reflexive.optimized.blocks[0].nodes[0].operation
    else {
        panic!("strict less-than fixture")
    };
    *abstract_right = *abstract_left;
    assert_eq!(
        source::derive_condition_for_test(
            0,
            &reflexive.target,
            &reflexive.abstracted,
            &reflexive.optimized,
        )
        .map(|_| ()),
        Err(LegalizationError::UnsupportedCondition { function: 0 })
    );

    let fixture = fixture();
    let derived = source::derive_condition_for_test(
        0,
        &fixture.target,
        &fixture.abstracted,
        &fixture.optimized,
    )
    .expect("strict less-than source condition");
    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::IntegerLessThanParametersV1 { left, right, .. } = &mut corrupted else {
        panic!("strict less-than custody")
    };
    std::mem::swap(left, right);
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

#[test]
fn strict_less_than_replay_rejects_equality_condition_substitution() {
    let fixture = fixture();
    let derived = source::derive_condition_for_test(
        0,
        &fixture.target,
        &fixture.abstracted,
        &fixture.optimized,
    )
    .expect("strict less-than source condition");
    let LegalizedCondition::IntegerLessThanParametersV1 {
        operation,
        result_definition_site,
        fuel,
        left,
        right,
    } = derived.legalized
    else {
        panic!("strict less-than custody")
    };
    let substituted = LegalizedCondition::IntegerEqualParametersV1 {
        operation,
        result_definition_site,
        fuel,
        left: LegalizedConditionParameter { ..left },
        right: LegalizedConditionParameter { ..right },
    };
    assert_eq!(
        replay::replay_condition_for_test(
            0,
            Architecture::X86_64,
            &fixture.target,
            &fixture.abstracted,
            &fixture.optimized,
            fixture.condition,
            &substituted,
        )
        .map(|_| ()),
        Err(LegalizationError::NonCanonicalLegalizedPlan)
    );
}

#[test]
fn inclusive_comparison_preserves_authored_order_through_independent_replay() {
    let fixture = less_or_equal_fixture();
    let derived = source::derive_condition_for_test(
        0,
        &fixture.target,
        &fixture.abstracted,
        &fixture.optimized,
    )
    .expect("inclusive source condition");
    assert_eq!(
        derived.shape,
        ScalarConditionShape::IntegerLessOrEqualU64Parameters
    );
    let LegalizedCondition::IntegerLessOrEqualParametersV1 {
        operation,
        left,
        right,
        fuel,
        ..
    } = &derived.legalized
    else {
        panic!("inclusive comparison custody")
    };
    assert_eq!(*operation, fixture.operation);
    assert_eq!(left.source_value, fixture.left);
    assert_eq!(left.parameter_index, 0);
    assert_eq!(right.source_value, fixture.right);
    assert_eq!(right.parameter_index, 1);
    assert_eq!(fuel.len(), 1);

    let replayed = replay::replay_condition_for_test(
        0,
        Architecture::X86_64,
        &fixture.target,
        &fixture.abstracted,
        &fixture.optimized,
        derived.source,
        &derived.legalized,
    )
    .expect("independent inclusive replay");
    assert_eq!(replayed.source, fixture.condition);
    assert_eq!(
        replayed.shape,
        ScalarConditionShape::IntegerLessOrEqualU64Parameters
    );
}

#[test]
fn inclusive_comparison_replay_rejects_order_and_predicate_substitution() {
    let mut reflexive = less_or_equal_fixture();
    let TargetOperation::ReturnIntegerExpressionConditionalControl {
        condition: TargetBooleanExpression::IntegerLessOrEqual { left, right, .. },
        ..
    } = &mut reflexive.target.operation
    else {
        panic!("inclusive comparison fixture")
    };
    *right = left.clone();
    assert_eq!(
        source::derive_condition_for_test(
            0,
            &reflexive.target,
            &reflexive.abstracted,
            &reflexive.optimized,
        )
        .map(|_| ()),
        Err(LegalizationError::UnsupportedCondition { function: 0 })
    );

    let fixture = less_or_equal_fixture();
    let derived = source::derive_condition_for_test(
        0,
        &fixture.target,
        &fixture.abstracted,
        &fixture.optimized,
    )
    .expect("inclusive source condition");
    let mut swapped = derived.legalized.clone();
    let LegalizedCondition::IntegerLessOrEqualParametersV1 { left, right, .. } = &mut swapped
    else {
        panic!("inclusive comparison custody")
    };
    std::mem::swap(left, right);
    assert_eq!(
        replay::replay_condition_for_test(
            0,
            Architecture::X86_64,
            &fixture.target,
            &fixture.abstracted,
            &fixture.optimized,
            derived.source,
            &swapped,
        )
        .map(|_| ()),
        Err(LegalizationError::NonCanonicalLegalizedPlan)
    );

    let LegalizedCondition::IntegerLessOrEqualParametersV1 {
        operation,
        result_definition_site,
        fuel,
        left,
        right,
    } = derived.legalized
    else {
        panic!("inclusive comparison custody")
    };
    let strict = LegalizedCondition::IntegerLessThanParametersV1 {
        operation,
        result_definition_site,
        fuel,
        left: LegalizedConditionParameter { ..left },
        right: LegalizedConditionParameter { ..right },
    };
    assert_eq!(
        replay::replay_condition_for_test(
            0,
            Architecture::X86_64,
            &fixture.target,
            &fixture.abstracted,
            &fixture.optimized,
            fixture.condition,
            &strict,
        )
        .map(|_| ()),
        Err(LegalizationError::NonCanonicalLegalizedPlan)
    );
}

#[test]
fn not_equal_condition_retains_both_operations_and_replays_independently() {
    let fixture = not_equal_fixture();
    let derived = source::derive_condition_for_test(
        0,
        &fixture.target,
        &fixture.abstracted,
        &fixture.optimized,
    )
    .expect("not-equal source condition");
    assert_eq!(
        derived.shape,
        ScalarConditionShape::IntegerNotEqualU64Parameters
    );
    let LegalizedCondition::IntegerNotEqualParametersV1 {
        equality_operation,
        equality_result,
        equality_fuel,
        boolean_not_operation,
        boolean_not_result,
        boolean_not_fuel,
        left,
        right,
        ..
    } = &derived.legalized
    else {
        panic!("not-equal custody")
    };
    assert_eq!(*equality_operation, fixture.operation);
    assert_eq!(
        *equality_result,
        fixture.optimized.blocks[0].nodes[0].definitions[0].value
    );
    assert_eq!(*boolean_not_result, fixture.condition);
    assert_ne!(*equality_operation, *boolean_not_operation);
    assert_eq!(equality_fuel.len(), 1);
    assert_eq!(boolean_not_fuel.len(), 1);
    assert_eq!(left.source_value, fixture.left);
    assert_eq!(right.source_value, fixture.right);
    assert_eq!(
        derived.provenance_operations,
        [*equality_operation, *boolean_not_operation]
    );

    let replayed = replay::replay_condition_for_test(
        0,
        Architecture::X86_64,
        &fixture.target,
        &fixture.abstracted,
        &fixture.optimized,
        derived.source,
        &derived.legalized,
    )
    .expect("independent not-equal replay");
    assert_eq!(replayed.source, fixture.condition);
    assert_eq!(
        replayed.shape,
        ScalarConditionShape::IntegerNotEqualU64Parameters
    );
}

#[test]
fn not_equal_replay_rejects_intermediate_result_fuel_and_order_corruption() {
    let fixture = not_equal_fixture();
    let derived = source::derive_condition_for_test(
        0,
        &fixture.target,
        &fixture.abstracted,
        &fixture.optimized,
    )
    .expect("not-equal source condition");

    let mut corruptions = Vec::new();
    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::IntegerNotEqualParametersV1 {
        equality_result, ..
    } = &mut corrupted
    else {
        unreachable!("not-equal custody")
    };
    *equality_result = fixture.condition;
    corruptions.push(corrupted);

    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::IntegerNotEqualParametersV1 {
        equality_result_definition_site,
        ..
    } = &mut corrupted
    else {
        unreachable!("not-equal custody")
    };
    *equality_result_definition_site = ValueDefinitionSite::Node {
        block: fixture.optimized.entry,
        node: 1,
    };
    corruptions.push(corrupted);

    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::IntegerNotEqualParametersV1 { equality_fuel, .. } = &mut corrupted
    else {
        unreachable!("not-equal custody")
    };
    equality_fuel[0].units += 1;
    corruptions.push(corrupted);

    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::IntegerNotEqualParametersV1 {
        boolean_not_result, ..
    } = &mut corrupted
    else {
        unreachable!("not-equal custody")
    };
    *boolean_not_result = fixture.optimized.blocks[0].nodes[0].definitions[0].value;
    corruptions.push(corrupted);

    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::IntegerNotEqualParametersV1 {
        boolean_not_result_definition_site,
        ..
    } = &mut corrupted
    else {
        unreachable!("not-equal custody")
    };
    *boolean_not_result_definition_site = ValueDefinitionSite::Node {
        block: fixture.optimized.entry,
        node: 0,
    };
    corruptions.push(corrupted);

    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::IntegerNotEqualParametersV1 {
        boolean_not_fuel, ..
    } = &mut corrupted
    else {
        unreachable!("not-equal custody")
    };
    boolean_not_fuel[0].units += 1;
    corruptions.push(corrupted);

    let mut corrupted = derived.legalized.clone();
    let LegalizedCondition::IntegerNotEqualParametersV1 { left, right, .. } = &mut corrupted else {
        unreachable!("not-equal custody")
    };
    std::mem::swap(left, right);
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
}
