//! Row-projection controls over raw source data, not source-admission evidence.

use super::*;
use calling_conventions::{CallSignature, ValueShape, evaluate_call_plan};
use legalized_operations::{
    LegalizedScalarCallUnitArgument, LegalizedScalarCallUnitCall, LegalizedScalarCallUnitConstant,
    ScalarCallUnitLegalizationRecipe,
};
use optimization_unit::EffectLink;
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerType, IntegerValue, MachineId, OperationId, StructuralTypeId,
};
use target_operations::{TargetUnitScalarArgumentSource, TargetUnitScalarHomeRequirement};

fn fixture(target: target::NativeTarget, count: usize) -> SourceScalarCallUnitFunction {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let shape = ValueShape::integer(8, 8);
    let site = |node| ValueDefinitionSite::Node {
        block: BlockId::new(1).unwrap(),
        node,
    };
    let fuel = |operation| {
        vec![FuelSettlement {
            site: PsiProvenance::Operation(operation),
            units: 1,
        }]
    };
    let effect = EffectLink {
        input: 0,
        output: 1,
    };
    let mut operations = Vec::new();
    for raw in 1..=2 {
        let operation = OperationId::new(raw).unwrap();
        operations.push(LegalizedScalarCallUnitOperation::Constant(
            LegalizedScalarCallUnitConstant {
                operation,
                result: ValueId::new(raw).unwrap(),
                scalar_type: integer,
                value: IntegerValue::Unsigned(u128::from(raw)),
                definition_site: site(raw as u32 - 1),
                fuel: fuel(operation),
                effect,
                ownership: Vec::new(),
            },
        ));
    }
    let mut previous = None;
    for (index, arity) in [count, 1].into_iter().enumerate() {
        let operation = OperationId::new(3 + index as u64).unwrap();
        let plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![shape; arity],
                result: Some(shape),
            },
        )
        .unwrap();
        let home = TargetUnitScalarHomeRequirement {
            defining_operation: operation,
            source_value: ValueId::new(3 + index as u64).unwrap(),
            scalar_type: ScalarType::Integer(integer),
            shape,
        };
        let arguments = plan
            .parameters
            .iter()
            .enumerate()
            .map(|(parameter_index, placement)| {
                let source = if let Some(previous) = previous {
                    TargetUnitScalarArgumentSource::Home(previous)
                } else {
                    let raw = 1 + (parameter_index % 2) as u64;
                    TargetUnitScalarArgumentSource::IntegerImmediate {
                        defining_operation: OperationId::new(raw).unwrap(),
                        source_value: ValueId::new(raw).unwrap(),
                        scalar_type: integer,
                        value: IntegerValue::Unsigned(u128::from(raw)),
                    }
                };
                LegalizedScalarCallUnitArgument {
                    parameter_index: parameter_index as u32,
                    source,
                    placement: placement.clone(),
                }
            })
            .collect();
        operations.push(LegalizedScalarCallUnitOperation::Call(
            LegalizedScalarCallUnitCall {
                operation,
                callee: MachineId::new(10 + index as u64).unwrap(),
                call_plan: plan,
                result_home: home,
                result_definition_site: site(2 + index as u32),
                arguments,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
                fuel: fuel(operation),
                effect,
                ownership: Vec::new(),
            },
        ));
        previous = Some(home);
    }
    SourceScalarCallUnitFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: StructuralTypeId::new(1).unwrap(),
        provenance: target_operations::TerminalPsiProvenance {
            operations: (1..=4).map(|raw| OperationId::new(raw).unwrap()).collect(),
            edges: vec![EdgeId::new(1).unwrap()],
        },
        recipe: ScalarCallUnitLegalizationRecipe::OrderedU64RegisterCallsThenReturnUnitV1,
        entry_block: BlockId::new(1).unwrap(),
        operations,
        return_edge: EdgeId::new(1).unwrap(),
        return_fuel: Vec::new(),
        return_effect: effect,
        return_ownership: Vec::new(),
    }
}

#[test]
fn register_call_arities_preserve_occurrences_and_reject_changed_projection() {
    for (target, maximum) in [
        (target::NativeTarget::linux_x64(), 6),
        (target::NativeTarget::linux_arm64(), 8),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        for count in [0, 1, 2, 3, maximum] {
            let source = fixture(target, count);
            let selected = build(
                0,
                &source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            let validate = |selected: &SelectedFunction| {
                crate::selection::validation::scalar_call_unit::validate(
                    0,
                    &source,
                    selected,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
            };
            validate(&selected).unwrap();
            let call_index = 2 + count;
            assert_eq!(selected.blocks[0].instructions.len(), count + 7);
            assert_eq!(
                selected.blocks[0].instructions[call_index].operands.len(),
                count + 1
            );
            // The following one-argument call consumes the prior durable result,
            // whose position depends on the preceding call's actual arity.
            assert_eq!(
                selected.blocks[0].instructions[call_index + 2].operands[0].virtual_register,
                VirtualRegisterId((call_index + 1) as u32)
            );
            for corruption in 0..6 {
                let mut changed = selected.clone();
                let instructions = &mut changed.blocks[0].instructions;
                match corruption {
                    0 => {
                        instructions[0].operands[0].virtual_register = VirtualRegisterId(1);
                        instructions[1].operands[0].virtual_register = VirtualRegisterId(0);
                    }
                    1 => {
                        instructions[call_index].operands.pop();
                    }
                    2 => {
                        instructions[call_index + 1].operands[0].virtual_register =
                            VirtualRegisterId(0)
                    }
                    3 => instructions[call_index]
                        .clobbers
                        .push(register_model::RegisterUnitId(999)),
                    4 => instructions[call_index].provenance.fuel[0].units += 1,
                    _ => {
                        instructions.pop();
                    }
                }
                assert!(
                    validate(&changed).is_err(),
                    "arity {count}, corruption {corruption}"
                );
            }
            let mut wrong_keys = constraints.clone();
            wrong_keys.keys.call_i64[count] =
                constraints.keys.call_i64[(count + 1) % (maximum + 1)];
            assert!(
                crate::selection::validation::scalar_call_unit::validate(
                    0,
                    &source,
                    &selected,
                    target,
                    &wrong_keys,
                    environment.physical(),
                    environment.constraints()
                )
                .is_err()
            );
        }
    }
}
