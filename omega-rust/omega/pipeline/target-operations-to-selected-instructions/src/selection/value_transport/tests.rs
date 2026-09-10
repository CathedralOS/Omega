//! Analysis fixtures only; explicit expected sets do not confer verification or execution authority.
use abstract_operations::ValueBinding;
use calling_conventions::{CallPlan, CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use legalized_operations::{
    LegalizedCallUnitSource, LegalizedScalarArgument, LegalizedScalarBlock, LegalizedScalarCall,
    LegalizedScalarComparison, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind as Instruction, LegalizedScalarParameter, LegalizedScalarReturn,
    LegalizedScalarReturnValue, LegalizedScalarSuccessor, LegalizedScalarTerminator as Terminator,
    LegalizedValueDefinition,
};
use optimization_unit::{EffectLink, ValueDefinition, ValueDefinitionSite};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, ScalarType, ValueId,
};
use std::collections::BTreeSet;

fn value(ordinal: u64) -> ValueId {
    ValueId::new(ordinal).unwrap()
}
fn integer() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}
fn assert_required_values(source: &LegalizedScalarFunction, ordinals: &[u64]) {
    let expected_values: BTreeSet<ValueId> = ordinals.iter().copied().map(value).collect();
    assert_eq!(
        crate::selection::value_transport::required_values(source),
        expected_values,
        "construction demand differs from the explicit set",
    );
    assert_eq!(
        crate::selection::validation::value_transport::required_values(source),
        expected_values,
        "independent replay demand differs from the explicit set",
    );
}
fn call_plan(parameter_count: usize, has_result: bool) -> CallPlan {
    evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8); parameter_count],
            result: has_result.then_some(ValueShape::integer(8, 8)),
        },
    )
    .unwrap()
}
fn effect() -> EffectLink {
    EffectLink {
        input: 0,
        output: 0,
    }
}
fn successor(destination: u64, bindings: &[(u64, u64)]) -> LegalizedScalarSuccessor {
    LegalizedScalarSuccessor {
        edge: EdgeId::new(destination).unwrap(),
        target: BlockId::new(destination).unwrap(),
        bindings: bindings
            .iter()
            .map(|&(parameter, argument)| ValueBinding {
                parameter: value(parameter),
                argument: value(argument),
                scalar_type: ScalarType::Integer(integer()),
            })
            .collect(),
        structural_bindings: Vec::new(),
        fuel: Vec::new(),
    }
}
fn jump(destination: u64, bindings: &[(u64, u64)]) -> Terminator {
    Terminator::Jump {
        successor: successor(destination, bindings),
        effect: effect(),
        ownership: Vec::new(),
    }
}
fn returned(ordinal: Option<u64>) -> Terminator {
    Terminator::Return(LegalizedScalarReturn {
        edge: EdgeId::new(99).unwrap(),
        value: ordinal.map_or(LegalizedScalarReturnValue::Unit, |ordinal| {
            LegalizedScalarReturnValue::Value {
                value: value(ordinal),
                scalar_type: integer(),
            }
        }),
        fuel: Vec::new(),
        effect: effect(),
        ownership: Vec::new(),
    })
}
fn block(ordinal: u64, parameters: &[u64], terminator: Terminator) -> LegalizedScalarBlock {
    let id = BlockId::new(ordinal).unwrap();
    LegalizedScalarBlock {
        id,
        parameters: parameters
            .iter()
            .enumerate()
            .map(|(position, ordinal)| ValueDefinition {
                value: value(*ordinal),
                scalar_type: ScalarType::Integer(integer()),
                site: ValueDefinitionSite::BlockParameter {
                    block: id,
                    position: position as u32,
                },
            })
            .collect(),
        structural_parameters: Vec::new(),
        instructions: Vec::new(),
        terminator,
    }
}
fn function(
    integer_parameters: &[u64],
    boolean_parameters: &[u64],
    blocks: Vec<LegalizedScalarBlock>,
) -> LegalizedScalarFunction {
    let declarations = integer_parameters
        .iter()
        .map(|ordinal| (*ordinal, ScalarType::Integer(integer())))
        .chain(
            boolean_parameters
                .iter()
                .map(|ordinal| (*ordinal, ScalarType::Boolean)),
        )
        .collect::<Vec<_>>();
    let plan = evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: declarations
                .iter()
                .map(|(_, scalar_type)| {
                    if *scalar_type == ScalarType::Boolean {
                        ValueShape::integer(1, 1)
                    } else {
                        ValueShape::integer(8, 8)
                    }
                })
                .collect(),
            result: blocks
                .iter()
                .any(|block| {
                    matches!(
                        &block.terminator,
                        Terminator::Return(LegalizedScalarReturn {
                            value: LegalizedScalarReturnValue::Value { .. },
                            ..
                        })
                    )
                })
                .then_some(ValueShape::integer(8, 8)),
        },
    )
    .unwrap();
    let parameters = declarations
        .iter()
        .zip(&plan.parameters)
        .enumerate()
        .map(
            |(position, ((ordinal, scalar_type), placement))| LegalizedScalarParameter {
                value: value(*ordinal),
                scalar_type: *scalar_type,
                definition_site: ValueDefinitionSite::FunctionParameter(position as u32),
                placement: placement.clone(),
            },
        )
        .collect();
    LegalizedScalarFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        provenance: Default::default(),
        call_plan: plan,
        parameters,
        structural: None,
        ranked: None,
        entry_block: blocks[0].id,
        blocks,
    }
}
fn retained_instruction(
    parameters: &[u64],
    kind: Instruction,
    result_type: ScalarType,
) -> LegalizedScalarFunction {
    let mut entry = block(1, &[], returned(None));
    entry.instructions.push(LegalizedScalarInstruction {
        operation: OperationId::new(1).unwrap(),
        result: Some(LegalizedValueDefinition {
            value: value(99),
            scalar_type: result_type,
            definition_site: ValueDefinitionSite::Node {
                block: entry.id,
                node: 0,
            },
        }),
        kind,
        fuel: Vec::new(),
        effect: effect(),
        ownership: Vec::new(),
    });
    function(parameters, &[], vec![entry])
}

#[test]
fn unused_transfer_cycle_demands_no_values() {
    let source = function(
        &[1],
        &[],
        vec![
            block(1, &[], jump(2, &[(11, 1)])),
            block(2, &[11], jump(3, &[(12, 11)])),
            block(3, &[12], jump(2, &[(11, 12)])),
        ],
    );
    assert_required_values(&source, &[]);
}

#[test]
fn live_return_demands_every_origin_across_multiple_edges() {
    let source = function(
        &[10, 11],
        &[],
        vec![
            block(1, &[], jump(2, &[(20, 10), (21, 11)])),
            block(2, &[20, 21], jump(3, &[(30, 20), (31, 21)])),
            block(3, &[30, 31], jump(4, &[(40, 30), (41, 31)])),
            block(4, &[40, 41], returned(Some(40))),
        ],
    );
    assert_required_values(&source, &[10, 20, 30, 40]);
}

#[test]
fn observing_one_swapped_parameter_demands_both_incoming_origins() {
    let source = function(
        &[1, 2, 4],
        &[3],
        vec![
            block(1, &[], jump(2, &[(10, 1), (11, 2), (12, 4)])),
            block(
                2,
                &[10, 11, 12],
                Terminator::Conditional {
                    condition: value(3),
                    when_true: successor(2, &[(10, 11), (11, 10), (12, 12)]),
                    when_false: successor(3, &[]),
                    effect: effect(),
                    ownership: Vec::new(),
                },
            ),
            block(3, &[], returned(Some(10))),
        ],
    );
    // The swap feeds either original input to the observed first parameter.
    // Its unobserved third parameter and origin stay outside physical demand.
    assert_required_values(&source, &[1, 2, 3, 10, 11]);
}

#[test]
fn retained_comparison_demands_operands_even_when_its_result_is_unused() {
    let source = retained_instruction(
        &[1, 2],
        Instruction::Compare {
            predicate: LegalizedScalarComparison::LessThan,
            operand_type: ScalarType::Integer(integer()),
            left: value(1),
            right: value(2),
        },
        ScalarType::Boolean,
    );
    assert_required_values(&source, &[1, 2]);
}

#[test]
fn retained_call_demands_arguments_even_when_its_result_is_unused() {
    let plan = call_plan(2, true);
    let arguments = [3, 4]
        .into_iter()
        .zip(&plan.parameters)
        .map(|(ordinal, placement)| LegalizedScalarArgument::Scalar {
            source: value(ordinal),
            placement: placement.clone(),
        })
        .collect();
    let source = retained_instruction(
        &[3, 4],
        Instruction::Call(LegalizedScalarCall {
            structural_result: None,
            source: LegalizedCallUnitSource::AuthoredCallUnit,
            callee: MachineId::new(2).unwrap(),
            result_placement: plan.result.clone(),
            call_plan: plan,
            arguments,
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        }),
        ScalarType::Integer(integer()),
    );
    assert_required_values(&source, &[3, 4]);
}
