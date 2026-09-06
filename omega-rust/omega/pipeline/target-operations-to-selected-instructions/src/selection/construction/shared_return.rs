//! Select the actual four-block graph without duplicating its return.

use crate::selection::constraints::{fixed_input_constraint, instruction, row};
use crate::selection::shared::*;
use legalized_operations::LegalizedSharedReturnConditionalFunction;

pub(super) fn build(
    function: usize,
    source: &LegalizedSharedReturnConditionalFunction,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedFunction, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::UnsupportedSourceShape { function };
    let (operation, fuel, left, right, signed, inclusive, equality) = match &source.condition {
        LegalizedCondition::IntegerEqualParametersV1 {
            operation,
            fuel,
            left,
            right,
            ..
        } => (operation, fuel, left, right, false, false, true),
        LegalizedCondition::IntegerLessThanParametersV1 {
            operation,
            fuel,
            left,
            right,
            ..
        } => (operation, fuel, left, right, false, false, false),
        LegalizedCondition::IntegerLessOrEqualParametersV1 {
            operation,
            fuel,
            left,
            right,
            ..
        } => (operation, fuel, left, right, false, true, false),
        LegalizedCondition::I64LessThanParametersV1 {
            operation,
            fuel,
            left,
            right,
            ..
        } => (operation, fuel, left, right, true, false, false),
        LegalizedCondition::I64LessOrEqualParametersV1 {
            operation,
            fuel,
            left,
            right,
            ..
        } => (operation, fuel, left, right, true, true, false),
        _ => return Err(invalid()),
    };
    let keys = &constraints.keys;
    let class = row(catalog, keys.materialize_i64)?.operands[0].class;
    let scalar_type = ScalarType::Integer(source.abi.result.scalar_type);
    let mut registers = Vec::new();
    for (index, parameter) in [left, right].into_iter().enumerate() {
        let fixed = fixed_input_constraint(
            source.machine,
            parameter.source_value,
            parameter.parameter_index,
            parameter.register,
            &constraints.fixed_inputs,
        )
        .ok_or_else(invalid)?;
        let view = physical
            .model()
            .views
            .iter()
            .find(|view| view.id == fixed.fixed_view)
            .ok_or_else(invalid)?;
        registers.push(VirtualRegister {
            id: VirtualRegisterId(index as u32),
            scalar_type: ScalarType::Integer(
                source.abi.parameters[parameter.parameter_index].scalar_type,
            ),
            class: view.class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: parameter.source_value,
                parameter_index: parameter.parameter_index,
            },
            definition_site: parameter.definition_site,
            entry_fixed_view: Some(fixed.fixed_view),
        });
    }
    let when_true = SelectedSuccessor {
        psi_edge: source.when_true.branch_edge,
        block: SelectedBlockId(1),
        source_target: source.when_true.block,
        bindings: source.when_true.branch_bindings.clone(),
        fuel: source.when_true.branch_fuel.clone(),
    };
    let when_false = SelectedSuccessor {
        psi_edge: source.when_false.branch_edge,
        block: SelectedBlockId(2),
        source_target: source.when_false.block,
        bindings: source.when_false.branch_bindings.clone(),
        fuel: source.when_false.branch_fuel.clone(),
    };
    let branch_kind = if equality {
        SelectedInstructionKind::ConditionalBranchNonZero
    } else if signed {
        SelectedInstructionKind::ConditionalBranchI64LessThan
    } else {
        SelectedInstructionKind::ConditionalBranchU64LessThan
    };
    let branch = instruction(
        SelectedInstructionId(1),
        branch_kind,
        keys.conditional_branch,
        &[],
        SelectedInstructionProvenance {
            values: vec![source.condition_source],
            ..Default::default()
        },
        catalog,
    )?;
    let terminator = if equality {
        SelectedTerminator::ConditionalBranch {
            instruction: branch,
            when_nonzero: when_false,
            when_zero: when_true,
        }
    } else {
        let (when_less, when_not_less) = if inclusive {
            (when_false, when_true)
        } else {
            (when_true, when_false)
        };
        if signed {
            SelectedTerminator::ConditionalBranchI64LessThan {
                instruction: branch,
                when_less,
                when_not_less,
            }
        } else {
            SelectedTerminator::ConditionalBranchU64LessThan {
                instruction: branch,
                when_less,
                when_not_less,
            }
        }
    };
    let mut blocks = vec![SelectedBlock {
        id: SelectedBlockId(0),
        source_block: source.entry_block,
        instructions: vec![instruction(
            SelectedInstructionId(0),
            SelectedInstructionKind::CompareI64,
            keys.compare_i64,
            &if inclusive {
                [VirtualRegisterId(1), VirtualRegisterId(0)]
            } else {
                [VirtualRegisterId(0), VirtualRegisterId(1)]
            },
            SelectedInstructionProvenance {
                operations: vec![*operation],
                values: vec![
                    left.source_value,
                    right.source_value,
                    source.condition_source,
                ],
                fuel: fuel.clone(),
                ..Default::default()
            },
            catalog,
        )?],
        terminator,
    }];
    for (position, arm) in [&source.when_true, &source.when_false]
        .into_iter()
        .enumerate()
    {
        let virtual_id = VirtualRegisterId(position as u32 + 2);
        let materialize_id = SelectedInstructionId(position as u32 * 2 + 2);
        registers.push(VirtualRegister {
            id: virtual_id,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: materialize_id,
                source_value: arm.constant.source_value,
            },
            definition_site: arm.constant.definition_site,
            entry_fixed_view: None,
        });
        blocks.push(SelectedBlock {
            id: SelectedBlockId(position as u32 + 1),
            source_block: arm.block,
            instructions: vec![instruction(
                materialize_id,
                SelectedInstructionKind::MaterializeI64 {
                    value: arm.constant.value,
                },
                keys.materialize_i64,
                &[virtual_id],
                SelectedInstructionProvenance {
                    operations: vec![arm.constant.constant_operation],
                    values: vec![arm.constant.source_value],
                    fuel: arm.constant.fuel.clone(),
                    ..Default::default()
                },
                catalog,
            )?],
            terminator: SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(materialize_id.0 + 1),
                    SelectedInstructionKind::Jump,
                    keys.jump,
                    &[],
                    SelectedInstructionProvenance::default(),
                    catalog,
                )?,
                successor: SelectedSuccessor {
                    psi_edge: arm.transfer_edge,
                    block: SelectedBlockId(3),
                    source_target: source.return_block,
                    bindings: vec![arm.transfer_binding],
                    fuel: arm.transfer_fuel.clone(),
                },
            },
        });
    }
    registers.push(VirtualRegister {
        id: VirtualRegisterId(4),
        scalar_type,
        class,
        origin: VirtualRegisterOrigin::BlockParameter {
            source_value: source.return_parameter.value,
            block: SelectedBlockId(3),
            parameter_index: 0,
        },
        definition_site: source.return_parameter.site,
        entry_fixed_view: None,
    });
    blocks.push(SelectedBlock {
        id: SelectedBlockId(3),
        source_block: source.return_block,
        instructions: Vec::new(),
        terminator: SelectedTerminator::Return {
            instruction: instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::ReturnI64,
                keys.return_i64,
                &[VirtualRegisterId(4)],
                SelectedInstructionProvenance {
                    values: vec![source.return_parameter.value],
                    edges: vec![source.return_edge],
                    fuel: source.return_fuel.clone(),
                    ..Default::default()
                },
                catalog,
            )?,
            psi_return_edge: source.return_edge,
        },
    });
    Ok(SelectedFunction {
        machine: source.machine,
        attachment: source.attachment,
        provenance: source.provenance.clone(),
        entry_block: SelectedBlockId(0),
        virtual_registers: registers,
        blocks,
    })
}
