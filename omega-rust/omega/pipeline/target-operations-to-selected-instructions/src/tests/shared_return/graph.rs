//! Uncanned scalar CFG shapes and signed comparison custody.
use super::*;
use legalized_operations::{LegalizedScalarComparison, LegalizedScalarInstructionKind};
fn expanded(
    native: target::NativeTarget,
    signed: bool,
    comparison: u8,
) -> (
    AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut plan, _, old) = fixture(native);
    let function = &mut plan.functions[0];
    let operand_type = ScalarType::Integer(
        IntegerType::new(
            if signed {
                IntegerSign::Signed
            } else {
                IntegerSign::Unsigned
            },
            64,
        )
        .unwrap(),
    );
    for parameter in &mut function.parameters {
        parameter.scalar_type = operand_type;
    }
    for arm in &mut function.block_entries[2..] {
        for parameter in &mut arm.parameters {
            parameter.scalar_type = operand_type;
        }
    }
    function.operations[0] = match comparison {
        0 => AbstractOperation::IntegerEqual {
            psi_operation: operation(1),
            result: value(4),
            left: value(1),
            right: value(2),
        },
        1 => AbstractOperation::IntegerLessThan {
            psi_operation: operation(1),
            result: value(4),
            left: value(1),
            right: value(2),
        },
        _ => AbstractOperation::IntegerLessOrEqual {
            psi_operation: operation(1),
            result: value(4),
            left: value(1),
            right: value(2),
        },
    };
    let AbstractOperation::Conditional {
        when_true,
        when_false,
        ..
    } = &mut function.operations[1]
    else {
        panic!("conditional");
    };
    for binding in when_true
        .bindings
        .iter_mut()
        .chain(&mut when_false.bindings)
    {
        binding.scalar_type = operand_type;
    }
    let bindings = std::mem::take(&mut when_true.bindings);
    when_true.target = block(5);
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    function.block_entries.push(AbstractBlockEntry {
        block: block(5),
        parameters: vec![],
        operation_offset: function.operations.len(),
    });
    function.operations.extend([
        AbstractOperation::IntegerConstant {
            psi_operation: operation(91),
            result: value(92),
            scalar_type: scalar,
            value: IntegerValue::Unsigned(13),
        },
        AbstractOperation::Jump {
            psi_edge: edge(6),
            target: block(3),
            bindings,
            trivial_affine_discards: vec![],
        },
    ]);
    let target =
        abstract_operations_to_target_operations::lower_to_target_operations(&plan, native)
            .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(&plan, old.fuel_schedule)
        .unwrap();
    (plan, target, unit)
}
#[test]
fn scalar_cfg_accepts_signed_comparisons_and_more_than_four_blocks() {
    for native in [
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        for signed in [false, true] {
            for comparison in 0..3 {
                let (plan, target, unit) = expanded(native, signed, comparison);
                assert!(crate::legalization::accepts_fragment_publication_input(
                    &target, &plan, &unit
                ));
                let legalized = legalize_target_operations(&target, &plan, &unit).unwrap();
                assert!(legalized.plan().functions.is_empty());
                let graph = &legalized.plan().scalar_functions[0];
                assert_eq!(graph.blocks.len(), 5);
                assert_eq!(graph.blocks[4].instructions[0].operation, operation(91));
                let LegalizedScalarInstructionKind::Compare { operand_type, .. } =
                    graph.blocks[0].instructions[0].kind
                else {
                    panic!("comparison");
                };
                assert_eq!(
                    operand_type,
                    IntegerType::new(
                        if signed {
                            IntegerSign::Signed
                        } else {
                            IntegerSign::Unsigned
                        },
                        64
                    )
                    .unwrap()
                );
                validate_legalized_operations(&target, &plan, &unit, legalized.plan().clone())
                    .unwrap();
                let environment =
                    register_environment::baseline_target_register_environment(native).unwrap();
                let constraints = selection_constraints(&legalized, &environment);
                let selected = select_instructions(
                    &legalized,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
                .unwrap();
                assert_eq!(selected.plan().functions[0].blocks.len(), 5);
                validate_selected_instructions(
                    &legalized,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    selected.plan().clone(),
                )
                .unwrap();
            }
        }
    }
}
#[test]
fn scalar_cfg_replay_rejects_comparison_type_operands_and_fuel_substitution() {
    let (plan, target, unit) = expanded(target::NativeTarget::linux_x64(), true, 1);
    let legalized = legalize_target_operations(&target, &plan, &unit).unwrap();
    for change in 0..7 {
        let mut proposed = legalized.plan().clone();
        let row = &mut proposed.scalar_functions[0].blocks[0].instructions[0];
        let LegalizedScalarInstructionKind::Compare {
            predicate,
            operand_type,
            left,
            right,
        } = &mut row.kind
        else {
            panic!("comparison");
        };
        match change {
            0 => *predicate = LegalizedScalarComparison::LessOrEqual,
            1 => *operand_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            2 => *left = value(2),
            3 => *right = value(1),
            4 => {
                row.scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
            }
            5 => row.fuel.clear(),
            _ => {
                row.definition_site = optimization_unit::ValueDefinitionSite::Node {
                    block: block(5),
                    node: 0,
                }
            }
        }
        assert!(validate_legalized_operations(&target, &plan, &unit, proposed).is_err());
    }
}
