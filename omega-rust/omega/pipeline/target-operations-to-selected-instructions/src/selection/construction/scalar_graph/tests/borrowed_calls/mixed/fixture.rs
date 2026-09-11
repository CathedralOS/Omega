//! Raw selection inputs exercise transport, without asserting proof admission.
use super::*;
use legalized_operations::{LegalizedExactIntegerOperator, LegalizedScalarSuccessor};
use semantic_vocabulary::ObligationId;

pub(super) fn source(
    target: target::NativeTarget,
    parameter_count: usize,
    scalar_count: usize,
    conditional: bool,
) -> LegalizedScalarFunction {
    let mut source = borrowed_call(target);
    let mut shapes = vec![ValueShape::integer(8, 8); parameter_count];
    shapes.push(ValueShape::borrowed_reference(16, 8));
    source.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: shapes,
            result: Some(ValueShape::integer(8, 8)),
        },
    )
    .unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    source.parameters = source.call_plan.parameters[..parameter_count]
        .iter()
        .enumerate()
        .map(|(position, placement)| LegalizedScalarParameter {
            value: ValueId::new(20 + position as u64).unwrap(),
            scalar_type,
            definition_site: ValueDefinitionSite::FunctionParameter(position as u32),
            placement: placement.clone(),
        })
        .collect();
    source.structural.as_mut().unwrap().parameters[0]
        .target
        .placement = source.call_plan.parameters[parameter_count].clone();
    let LegalizedScalarInstructionKind::Call(template) = &source.blocks[0].instructions[0].kind
    else {
        unreachable!();
    };
    let mut template = template.clone();
    let mut shapes = vec![ValueShape::integer(8, 8); scalar_count];
    shapes.push(ValueShape::borrowed_reference(16, 8));
    template.call_plan = evaluate_call_plan(
        source.call_plan.policy,
        &CallSignature {
            parameters: shapes,
            result: Some(ValueShape::integer(8, 8)),
        },
    )
    .unwrap();
    template.result_placement = template.call_plan.result.clone();
    let mut borrowed = template.arguments.remove(0);
    let LegalizedScalarArgument::Structural {
        target: argument, ..
    } = &mut borrowed
    else {
        unreachable!();
    };
    argument.source = source.call_plan.parameters[parameter_count].clone().into();
    argument.destination = template.call_plan.parameters[scalar_count].clone();
    source.blocks[0].instructions = super::super::super::fixture(target, 1)
        .blocks
        .remove(0)
        .instructions;
    // Value 2 is computed from an actual entry scalar when one is present.
    source.blocks[0].instructions[1].kind = LegalizedScalarInstructionKind::ExactBinary {
        operator: LegalizedExactIntegerOperator::Add,
        left: source
            .parameters
            .first()
            .map_or(ValueId::new(1).unwrap(), |parameter| parameter.value),
        right: ValueId::new(1).unwrap(),
        obligation: ObligationId::new(2).unwrap(),
        accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([2; 32]),
    };
    for (position, row) in source.blocks[0].instructions.iter_mut().enumerate().skip(2) {
        let mut call = template.clone();
        call.callee = MachineId::new(10 + position as u64).unwrap();
        call.arguments = call.call_plan.parameters[..scalar_count]
            .iter()
            .enumerate()
            .map(
                |(argument_position, placement)| LegalizedScalarArgument::Scalar {
                    source: if argument_position == 0 {
                        ValueId::new(position as u64).unwrap()
                    } else {
                        source
                            .parameters
                            .get((argument_position - 1) % parameter_count.max(1))
                            .map_or(ValueId::new(1).unwrap(), |parameter| parameter.value)
                    },
                    placement: placement.clone(),
                },
            )
            .chain(std::iter::once(borrowed.clone()))
            .collect();
        row.kind = LegalizedScalarInstructionKind::Call(call);
        row.ownership = vec![optimization_unit::OwnershipEvent::ClaimTransfer(Vec::new())];
    }
    returned(&mut source.blocks[0]).value = LegalizedScalarReturnValue::Value {
        value: ValueId::new(4).unwrap(),
        scalar_type: semantic_vocabulary::ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        ),
    };
    if conditional {
        branches(&mut source);
    }
    source.provenance.operations = source
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter().map(|row| row.operation))
        .collect();
    source
}

fn branches(source: &mut LegalizedScalarFunction) {
    let second_call = source.blocks[0].instructions.pop().unwrap();
    let mut comparison = second_call.clone();
    comparison.operation = OperationId::new(6).unwrap();
    comparison.result = Some(LegalizedValueDefinition {
        value: ValueId::new(6).unwrap(),
        scalar_type: ScalarType::Boolean,
        definition_site: ValueDefinitionSite::Node {
            block: source.entry_block,
            node: 3,
        },
    });
    comparison.kind = LegalizedScalarInstructionKind::Compare {
        predicate: legalized_operations::LegalizedScalarComparison::LessThan,
        operand_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        left: ValueId::new(3).unwrap(),
        right: ValueId::new(1).unwrap(),
    };
    comparison.fuel[0].site = PsiProvenance::Operation(comparison.operation);
    comparison.ownership.clear();
    source.blocks[0].instructions.push(comparison);
    let successor = |raw| LegalizedScalarSuccessor {
        structural_bindings: Vec::new(),
        edge: EdgeId::new(raw).unwrap(),
        target: BlockId::new(raw + 1).unwrap(),
        bindings: Vec::new(),
        fuel: Vec::new(),
    };
    source.blocks[0].terminator = LegalizedScalarTerminator::Conditional {
        condition: ValueId::new(6).unwrap(),
        when_true: successor(1),
        when_false: successor(2),
        effect: second_call.effect,
        ownership: Vec::new(),
    };
    for raw in [2, 3] {
        let mut call = second_call.clone();
        let block = BlockId::new(raw).unwrap();
        call.operation = OperationId::new(raw + 2).unwrap();
        call.result.as_mut().unwrap().value = ValueId::new(raw + 2).unwrap();
        call.result.as_mut().unwrap().definition_site =
            ValueDefinitionSite::Node { block, node: 0 };
        call.fuel[0].site = PsiProvenance::Operation(call.operation);
        source.blocks.push(LegalizedScalarBlock {
            structural_parameters: Vec::new(),
            id: block,
            parameters: Vec::new(),
            terminator: LegalizedScalarTerminator::Return(LegalizedScalarReturn {
                edge: EdgeId::new(raw + 1).unwrap(),
                value: LegalizedScalarReturnValue::Value {
                    value: call.result.unwrap().value,
                    scalar_type: semantic_vocabulary::ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    ),
                },
                fuel: Vec::new(),
                effect: call.effect,
                ownership: Vec::new(),
            }),
            instructions: vec![call],
        });
    }
    source.provenance.edges = (1..=4).map(|raw| EdgeId::new(raw).unwrap()).collect();
}

pub(super) fn constraints(
    source: &LegalizedScalarFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> SelectedSelectionConstraints {
    SelectedSelectionConstraints {
        keys: environment.selected_keys(),
        fixed_inputs: source
            .parameters
            .iter()
            .enumerate()
            .map(|(parameter_index, parameter)| {
                let [ValueLocation::Register { register, .. }] =
                    parameter.placement.locations.as_slice()
                else {
                    panic!("register parameter");
                };
                SelectedFixedInputConstraint {
                    machine: source.machine,
                    source_value: parameter.value,
                    parameter_index,
                    register: *register,
                    fixed_view: environment.fixed_register_view(*register).unwrap(),
                }
            })
            .collect(),
    }
}
