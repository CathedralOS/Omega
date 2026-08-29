use crate::selection::constraints::{fixed_input_constraint, instruction, row};
use crate::selection::shared::*;

pub(super) fn build_function(
    function: usize,
    source: &SourceFunction,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedFunction, SelectedInstructionError> {
    let input = fixed_input_constraint(
        source.machine,
        source.condition_source,
        source.condition_parameter_index,
        source.condition_register,
        &constraints.fixed_inputs,
    )
    .ok_or(SelectedInstructionError::MissingInputRegisterView { function })?;
    let input_view = input.fixed_view;
    let Some(input_view) = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == input_view)
    else {
        return Err(SelectedInstructionError::MissingInputRegisterView { function });
    };
    let input_class = input_view.class;
    let keys = constraints.keys;
    let result_class = match &source.when_true.value {
        SourceLeafValue::Immediate { .. } => row(catalog, keys.materialize_i64)?.operands[0].class,
        SourceLeafValue::EntryParameter {
            parameter_index,
            register,
            ..
        } => {
            let result_input = fixed_input_constraint(
                source.machine,
                source.when_true.source_value,
                *parameter_index,
                *register,
                &constraints.fixed_inputs,
            )
            .ok_or(SelectedInstructionError::MissingInputRegisterView { function })?;
            physical
                .model()
                .views
                .iter()
                .find(|view| view.id == result_input.fixed_view)
                .ok_or(SelectedInstructionError::MissingInputRegisterView { function })?
                .class
        }
        SourceLeafValue::ExactAdd { .. } | SourceLeafValue::WidenedExactAdd { .. } => {
            row(catalog, keys.add_i64)?.operands[2].class
        }
        SourceLeafValue::ActiveResidentExactAddChain(..) => {
            row(catalog, keys.add_i64)?.operands[2].class
        }
        SourceLeafValue::ExactSubtract { .. } | SourceLeafValue::WidenedExactSubtract { .. } => {
            row(catalog, keys.subtract_i64)?.operands[2].class
        }
    };
    let u64_type =
        ScalarType::Integer(psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"));
    Ok(SelectedFunction {
        machine: source.machine,
        attachment: source.attachment,
        provenance: source.provenance.clone(),
        entry_block: SelectedBlockId(0),
        virtual_registers: {
            let mut registers = vec![VirtualRegister {
                id: VirtualRegisterId(0),
                scalar_type: ScalarType::Boolean,
                class: input_class,
                origin: VirtualRegisterOrigin::EntryParameter {
                    source_value: source.condition_source,
                    parameter_index: source.condition_parameter_index,
                },
                definition_site: source.condition_definition_site,
                entry_fixed_view: Some(input_view.id),
            }];
            match (&source.when_true.value, &source.when_false.value) {
                (
                    SourceLeafValue::ActiveResidentExactAddChain(chain),
                    SourceLeafValue::Immediate {
                        definition_site: false_site,
                        ..
                    },
                ) => {
                    for (id, instruction, source_value, definition_site) in [
                        (
                            1,
                            2,
                            chain.resident.source_value,
                            chain.resident.definition_site,
                        ),
                        (2, 3, chain.left.source_value, chain.left.definition_site),
                        (3, 4, chain.right.source_value, chain.right.definition_site),
                        (4, 5, chain.inner.source_value, chain.inner.definition_site),
                        (
                            5,
                            6,
                            chain.middle.source_value,
                            chain.middle.definition_site,
                        ),
                        (
                            6,
                            7,
                            chain.result.source_value,
                            chain.result.definition_site,
                        ),
                        (7, 9, source.when_false.source_value, *false_site),
                    ] {
                        registers.push(VirtualRegister {
                            id: VirtualRegisterId(id),
                            scalar_type: u64_type,
                            class: result_class,
                            origin: VirtualRegisterOrigin::InstructionResult {
                                instruction: SelectedInstructionId(instruction),
                                source_value,
                            },
                            definition_site,
                            entry_fixed_view: None,
                        });
                    }
                }
                (
                    SourceLeafValue::Immediate {
                        definition_site: true_site,
                        ..
                    },
                    SourceLeafValue::Immediate {
                        definition_site: false_site,
                        ..
                    },
                ) => {
                    registers.push(VirtualRegister {
                        id: VirtualRegisterId(1),
                        scalar_type: u64_type,
                        class: result_class,
                        origin: VirtualRegisterOrigin::InstructionResult {
                            instruction: SelectedInstructionId(2),
                            source_value: source.when_true.source_value,
                        },
                        definition_site: *true_site,
                        entry_fixed_view: None,
                    });
                    registers.push(VirtualRegister {
                        id: VirtualRegisterId(2),
                        scalar_type: u64_type,
                        class: result_class,
                        origin: VirtualRegisterOrigin::InstructionResult {
                            instruction: SelectedInstructionId(4),
                            source_value: source.when_false.source_value,
                        },
                        definition_site: *false_site,
                        entry_fixed_view: None,
                    });
                }
                (
                    SourceLeafValue::EntryParameter {
                        parameter_index,
                        register,
                        definition_site,
                    },
                    SourceLeafValue::EntryParameter { .. },
                ) => {
                    let fixed = fixed_input_constraint(
                        source.machine,
                        source.when_true.source_value,
                        *parameter_index,
                        *register,
                        &constraints.fixed_inputs,
                    )
                    .ok_or(SelectedInstructionError::MissingInputRegisterView { function })?;
                    registers.push(VirtualRegister {
                        id: VirtualRegisterId(1),
                        scalar_type: u64_type,
                        class: result_class,
                        origin: VirtualRegisterOrigin::EntryParameter {
                            source_value: source.when_true.source_value,
                            parameter_index: *parameter_index,
                        },
                        definition_site: *definition_site,
                        entry_fixed_view: Some(fixed.fixed_view),
                    });
                }
                (
                    SourceLeafValue::WidenedExactAdd {
                        widen_definition_site: true_site,
                        left_temporary: true_left_temporary,
                        right_temporary: true_right_temporary,
                        left: true_left,
                        right: true_right,
                        ..
                    }
                    | SourceLeafValue::WidenedExactSubtract {
                        widen_definition_site: true_site,
                        left_temporary: true_left_temporary,
                        right_temporary: true_right_temporary,
                        left: true_left,
                        right: true_right,
                        ..
                    },
                    SourceLeafValue::WidenedExactAdd {
                        widen_definition_site: false_site,
                        left_temporary: false_left_temporary,
                        right_temporary: false_right_temporary,
                        left: false_left,
                        right: false_right,
                        ..
                    }
                    | SourceLeafValue::WidenedExactSubtract {
                        widen_definition_site: false_site,
                        left_temporary: false_left_temporary,
                        right_temporary: false_right_temporary,
                        left: false_left,
                        right: false_right,
                        ..
                    },
                ) => {
                    for (id, instruction, source_value, definition_site, legalized_temporary) in [
                        (
                            1,
                            2,
                            true_left.source_value,
                            true_left.definition_site,
                            Some(*true_left_temporary),
                        ),
                        (
                            2,
                            3,
                            true_right.source_value,
                            true_right.definition_site,
                            Some(*true_right_temporary),
                        ),
                        (3, 4, source.when_true.source_value, *true_site, None),
                        (
                            4,
                            6,
                            false_left.source_value,
                            false_left.definition_site,
                            Some(*false_left_temporary),
                        ),
                        (
                            5,
                            7,
                            false_right.source_value,
                            false_right.definition_site,
                            Some(*false_right_temporary),
                        ),
                        (6, 8, source.when_false.source_value, *false_site, None),
                    ] {
                        registers.push(VirtualRegister {
                            id: VirtualRegisterId(id),
                            scalar_type: u64_type,
                            class: result_class,
                            origin: match legalized_temporary {
                                Some(temporary) => VirtualRegisterOrigin::LegalizationTemporary {
                                    instruction: SelectedInstructionId(instruction),
                                    temporary,
                                    source_value,
                                },
                                None => VirtualRegisterOrigin::InstructionResult {
                                    instruction: SelectedInstructionId(instruction),
                                    source_value,
                                },
                            },
                            definition_site,
                            entry_fixed_view: None,
                        });
                    }
                }
                (
                    SourceLeafValue::ExactAdd {
                        definition_site: true_site,
                        left: true_left,
                        right: true_right,
                        ..
                    }
                    | SourceLeafValue::ExactSubtract {
                        definition_site: true_site,
                        left: true_left,
                        right: true_right,
                        ..
                    },
                    SourceLeafValue::ExactAdd {
                        definition_site: false_site,
                        left: false_left,
                        right: false_right,
                        ..
                    }
                    | SourceLeafValue::ExactSubtract {
                        definition_site: false_site,
                        left: false_left,
                        right: false_right,
                        ..
                    },
                ) => {
                    for (id, instruction, source_value, definition_site) in [
                        (1, 2, true_left.source_value, true_left.definition_site),
                        (2, 3, true_right.source_value, true_right.definition_site),
                        (3, 4, source.when_true.source_value, *true_site),
                        (4, 6, false_left.source_value, false_left.definition_site),
                        (5, 7, false_right.source_value, false_right.definition_site),
                        (6, 8, source.when_false.source_value, *false_site),
                    ] {
                        registers.push(VirtualRegister {
                            id: VirtualRegisterId(id),
                            scalar_type: u64_type,
                            class: result_class,
                            origin: VirtualRegisterOrigin::InstructionResult {
                                instruction: SelectedInstructionId(instruction),
                                source_value,
                            },
                            definition_site,
                            entry_fixed_view: None,
                        });
                    }
                }
                _ => return Err(SelectedInstructionError::UnsupportedSourceShape { function }),
            }
            registers
        },
        blocks: match (&source.when_true.value, &source.when_false.value) {
            (
                SourceLeafValue::ActiveResidentExactAddChain(..),
                SourceLeafValue::Immediate { .. },
            ) => vec![
                build_entry_block(source, keys, catalog)?,
                build_active_resident_exact_add_chain_block(
                    function,
                    SelectedBlockId(1),
                    source.true_block,
                    &source.when_true,
                    keys,
                    catalog,
                )?,
                build_constant_return_block(
                    function,
                    SelectedBlockId(2),
                    source.false_block,
                    9,
                    10,
                    VirtualRegisterId(7),
                    &source.when_false,
                    keys,
                    catalog,
                )?,
            ],
            (SourceLeafValue::Immediate { .. }, SourceLeafValue::Immediate { .. }) => vec![
                build_entry_block(source, keys, catalog)?,
                build_constant_return_block(
                    function,
                    SelectedBlockId(1),
                    source.true_block,
                    2,
                    3,
                    VirtualRegisterId(1),
                    &source.when_true,
                    keys,
                    catalog,
                )?,
                build_constant_return_block(
                    function,
                    SelectedBlockId(2),
                    source.false_block,
                    4,
                    5,
                    VirtualRegisterId(2),
                    &source.when_false,
                    keys,
                    catalog,
                )?,
            ],
            (SourceLeafValue::EntryParameter { .. }, SourceLeafValue::EntryParameter { .. }) => {
                vec![
                    build_entry_block(source, keys, catalog)?,
                    build_parameter_return_block(
                        function,
                        SelectedBlockId(1),
                        source.true_block,
                        2,
                        VirtualRegisterId(1),
                        &source.when_true,
                        keys,
                        catalog,
                    )?,
                    build_parameter_return_block(
                        function,
                        SelectedBlockId(2),
                        source.false_block,
                        3,
                        VirtualRegisterId(1),
                        &source.when_false,
                        keys,
                        catalog,
                    )?,
                ]
            }
            (SourceLeafValue::ExactAdd { .. }, SourceLeafValue::ExactAdd { .. }) => vec![
                build_entry_block(source, keys, catalog)?,
                build_exact_binary_return_block(
                    function,
                    SelectedBlockId(1),
                    source.true_block,
                    [2, 3, 4, 5],
                    [
                        VirtualRegisterId(1),
                        VirtualRegisterId(2),
                        VirtualRegisterId(3),
                    ],
                    &source.when_true,
                    keys,
                    catalog,
                )?,
                build_exact_binary_return_block(
                    function,
                    SelectedBlockId(2),
                    source.false_block,
                    [6, 7, 8, 9],
                    [
                        VirtualRegisterId(4),
                        VirtualRegisterId(5),
                        VirtualRegisterId(6),
                    ],
                    &source.when_false,
                    keys,
                    catalog,
                )?,
            ],
            (SourceLeafValue::WidenedExactAdd { .. }, SourceLeafValue::WidenedExactAdd { .. }) => {
                vec![
                    build_entry_block(source, keys, catalog)?,
                    build_exact_binary_return_block(
                        function,
                        SelectedBlockId(1),
                        source.true_block,
                        [2, 3, 4, 5],
                        [
                            VirtualRegisterId(1),
                            VirtualRegisterId(2),
                            VirtualRegisterId(3),
                        ],
                        &source.when_true,
                        keys,
                        catalog,
                    )?,
                    build_exact_binary_return_block(
                        function,
                        SelectedBlockId(2),
                        source.false_block,
                        [6, 7, 8, 9],
                        [
                            VirtualRegisterId(4),
                            VirtualRegisterId(5),
                            VirtualRegisterId(6),
                        ],
                        &source.when_false,
                        keys,
                        catalog,
                    )?,
                ]
            }
            (
                SourceLeafValue::WidenedExactSubtract { .. },
                SourceLeafValue::WidenedExactSubtract { .. },
            ) => vec![
                build_entry_block(source, keys, catalog)?,
                build_exact_binary_return_block(
                    function,
                    SelectedBlockId(1),
                    source.true_block,
                    [2, 3, 4, 5],
                    [
                        VirtualRegisterId(1),
                        VirtualRegisterId(2),
                        VirtualRegisterId(3),
                    ],
                    &source.when_true,
                    keys,
                    catalog,
                )?,
                build_exact_binary_return_block(
                    function,
                    SelectedBlockId(2),
                    source.false_block,
                    [6, 7, 8, 9],
                    [
                        VirtualRegisterId(4),
                        VirtualRegisterId(5),
                        VirtualRegisterId(6),
                    ],
                    &source.when_false,
                    keys,
                    catalog,
                )?,
            ],
            (SourceLeafValue::ExactSubtract { .. }, SourceLeafValue::ExactSubtract { .. }) => vec![
                build_entry_block(source, keys, catalog)?,
                build_exact_binary_return_block(
                    function,
                    SelectedBlockId(1),
                    source.true_block,
                    [2, 3, 4, 5],
                    [
                        VirtualRegisterId(1),
                        VirtualRegisterId(2),
                        VirtualRegisterId(3),
                    ],
                    &source.when_true,
                    keys,
                    catalog,
                )?,
                build_exact_binary_return_block(
                    function,
                    SelectedBlockId(2),
                    source.false_block,
                    [6, 7, 8, 9],
                    [
                        VirtualRegisterId(4),
                        VirtualRegisterId(5),
                        VirtualRegisterId(6),
                    ],
                    &source.when_false,
                    keys,
                    catalog,
                )?,
            ],
            _ => return Err(SelectedInstructionError::UnsupportedSourceShape { function }),
        },
    })
}

fn build_entry_block(
    source: &SourceFunction,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedBlock, SelectedInstructionError> {
    Ok(SelectedBlock {
        id: SelectedBlockId(0),
        source_block: source.entry_block,
        instructions: vec![instruction(
            SelectedInstructionId(0),
            SelectedInstructionKind::CompareI64Zero,
            keys.compare_i64_zero,
            &[VirtualRegisterId(0)],
            SelectedInstructionProvenance {
                values: vec![source.condition_source],
                ..Default::default()
            },
            catalog,
        )?],
        terminator: SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                SelectedInstructionId(1),
                SelectedInstructionKind::ConditionalBranchNonZero,
                keys.conditional_branch,
                &[],
                SelectedInstructionProvenance {
                    values: vec![source.condition_source],
                    ..Default::default()
                },
                catalog,
            )?,
            when_nonzero: SelectedSuccessor {
                psi_edge: source.branch_true_edge,
                block: SelectedBlockId(1),
                source_target: source.true_block,
                bindings: source.branch_true_bindings.clone(),
                fuel: source.branch_true_fuel.clone(),
            },
            when_zero: SelectedSuccessor {
                psi_edge: source.branch_false_edge,
                block: SelectedBlockId(2),
                source_target: source.false_block,
                bindings: source.branch_false_bindings.clone(),
                fuel: source.branch_false_fuel.clone(),
            },
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn build_constant_return_block(
    function: usize,
    id: SelectedBlockId,
    source_block: psi_core::BlockId,
    materialize_id: u32,
    return_id: u32,
    register: VirtualRegisterId,
    source: &SourceLeaf,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedBlock, SelectedInstructionError> {
    let SourceLeafValue::Immediate {
        value,
        constant_operation,
        constant_fuel,
        ..
    } = &source.value
    else {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    };
    Ok(SelectedBlock {
        id,
        source_block,
        instructions: vec![instruction(
            SelectedInstructionId(materialize_id),
            SelectedInstructionKind::MaterializeI64 { value: *value },
            keys.materialize_i64,
            &[register],
            SelectedInstructionProvenance {
                operations: vec![*constant_operation],
                values: vec![source.source_value],
                fuel: constant_fuel.clone(),
                ..Default::default()
            },
            catalog,
        )?],
        terminator: SelectedTerminator::Return {
            instruction: instruction(
                SelectedInstructionId(return_id),
                SelectedInstructionKind::ReturnI64,
                keys.return_i64,
                &[register],
                SelectedInstructionProvenance {
                    values: vec![source.source_value],
                    edges: vec![source.return_edge],
                    fuel: source.return_fuel.clone(),
                    ..Default::default()
                },
                catalog,
            )?,
            psi_return_edge: source.return_edge,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn build_parameter_return_block(
    function: usize,
    id: SelectedBlockId,
    source_block: psi_core::BlockId,
    return_id: u32,
    register: VirtualRegisterId,
    source: &SourceLeaf,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedBlock, SelectedInstructionError> {
    if !matches!(source.value, SourceLeafValue::EntryParameter { .. }) {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    }
    Ok(SelectedBlock {
        id,
        source_block,
        instructions: Vec::new(),
        terminator: SelectedTerminator::Return {
            instruction: instruction(
                SelectedInstructionId(return_id),
                SelectedInstructionKind::ReturnI64,
                keys.return_i64,
                &[register],
                SelectedInstructionProvenance {
                    values: vec![source.source_value],
                    edges: vec![source.return_edge],
                    fuel: source.return_fuel.clone(),
                    ..Default::default()
                },
                catalog,
            )?,
            psi_return_edge: source.return_edge,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn build_exact_binary_return_block(
    function: usize,
    id: SelectedBlockId,
    source_block: psi_core::BlockId,
    instruction_ids: [u32; 4],
    registers: [VirtualRegisterId; 3],
    source: &SourceLeaf,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedBlock, SelectedInstructionError> {
    let (obligation, operations, values, operation_fuel, left, right, kind, key) =
        match &source.value {
            SourceLeafValue::ExactAdd {
                obligation,
                accepted_fact,
                add_operation,
                add_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*add_operation],
                vec![left.source_value, right.source_value, source.source_value],
                add_fuel.clone(),
                left,
                right,
                SelectedInstructionKind::ExactAddI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.add_i64,
            ),
            SourceLeafValue::WidenedExactAdd {
                obligation,
                accepted_fact,
                add_operation,
                narrow_result,
                add_fuel,
                widen_operation,
                widen_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*add_operation, *widen_operation],
                vec![
                    left.source_value,
                    right.source_value,
                    *narrow_result,
                    source.source_value,
                ],
                add_fuel.iter().chain(widen_fuel).copied().collect(),
                left,
                right,
                SelectedInstructionKind::ExactAddI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.add_i64,
            ),
            SourceLeafValue::ExactSubtract {
                obligation,
                accepted_fact,
                subtract_operation,
                subtract_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*subtract_operation],
                vec![left.source_value, right.source_value, source.source_value],
                subtract_fuel.clone(),
                left,
                right,
                SelectedInstructionKind::ExactSubtractI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.subtract_i64,
            ),
            SourceLeafValue::WidenedExactSubtract {
                obligation,
                accepted_fact,
                subtract_operation,
                narrow_result,
                subtract_fuel,
                widen_operation,
                widen_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*subtract_operation, *widen_operation],
                vec![
                    left.source_value,
                    right.source_value,
                    *narrow_result,
                    source.source_value,
                ],
                subtract_fuel.iter().chain(widen_fuel).copied().collect(),
                left,
                right,
                SelectedInstructionKind::ExactSubtractI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.subtract_i64,
            ),
            _ => return Err(SelectedInstructionError::UnsupportedSourceShape { function }),
        };
    let materialize = |id, register, immediate: &SourceImmediate| {
        instruction(
            SelectedInstructionId(id),
            SelectedInstructionKind::MaterializeI64 {
                value: immediate.value,
            },
            keys.materialize_i64,
            &[register],
            SelectedInstructionProvenance {
                operations: vec![immediate.constant_operation],
                values: vec![immediate.source_value],
                fuel: immediate.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )
    };
    Ok(SelectedBlock {
        id,
        source_block,
        instructions: vec![
            materialize(instruction_ids[0], registers[0], left)?,
            materialize(instruction_ids[1], registers[1], right)?,
            instruction(
                SelectedInstructionId(instruction_ids[2]),
                kind,
                key,
                &registers,
                SelectedInstructionProvenance {
                    operations,
                    values,
                    obligations: vec![*obligation],
                    fuel: operation_fuel,
                    ..Default::default()
                },
                catalog,
            )?,
        ],
        terminator: SelectedTerminator::Return {
            instruction: instruction(
                SelectedInstructionId(instruction_ids[3]),
                SelectedInstructionKind::ReturnI64,
                keys.return_i64,
                &[registers[2]],
                SelectedInstructionProvenance {
                    values: vec![source.source_value],
                    edges: vec![source.return_edge],
                    fuel: source.return_fuel.clone(),
                    ..Default::default()
                },
                catalog,
            )?,
            psi_return_edge: source.return_edge,
        },
    })
}

fn build_active_resident_exact_add_chain_block(
    function: usize,
    id: SelectedBlockId,
    source_block: psi_core::BlockId,
    source: &SourceLeaf,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedBlock, SelectedInstructionError> {
    let SourceLeafValue::ActiveResidentExactAddChain(chain) = &source.value else {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    };
    let materialize = |id, register, immediate: &SourceImmediate| {
        instruction(
            SelectedInstructionId(id),
            SelectedInstructionKind::MaterializeI64 {
                value: immediate.value,
            },
            keys.materialize_i64,
            &[register],
            SelectedInstructionProvenance {
                operations: vec![immediate.constant_operation],
                values: vec![immediate.source_value],
                fuel: immediate.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )
    };
    let exact_add = |id,
                     operands: [VirtualRegisterId; 3],
                     add: &omega_legalized_operations::LegalizedExactAdd,
                     values: Vec<psi_core::ValueId>| {
        instruction(
            SelectedInstructionId(id),
            SelectedInstructionKind::ExactAddI64 {
                obligation: add.obligation,
                accepted_fact: add.accepted_fact,
            },
            keys.add_i64,
            &operands,
            SelectedInstructionProvenance {
                operations: vec![add.operation],
                values,
                obligations: vec![add.obligation],
                fuel: add.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )
    };
    Ok(SelectedBlock {
        id,
        source_block,
        instructions: vec![
            materialize(2, VirtualRegisterId(1), &chain.resident)?,
            materialize(3, VirtualRegisterId(2), &chain.left)?,
            materialize(4, VirtualRegisterId(3), &chain.right)?,
            exact_add(
                5,
                [
                    VirtualRegisterId(2),
                    VirtualRegisterId(3),
                    VirtualRegisterId(4),
                ],
                &chain.inner,
                vec![
                    chain.left.source_value,
                    chain.right.source_value,
                    chain.inner.source_value,
                ],
            )?,
            exact_add(
                6,
                [
                    VirtualRegisterId(1),
                    VirtualRegisterId(4),
                    VirtualRegisterId(5),
                ],
                &chain.middle,
                vec![
                    chain.resident.source_value,
                    chain.inner.source_value,
                    chain.middle.source_value,
                ],
            )?,
            exact_add(
                7,
                [
                    VirtualRegisterId(1),
                    VirtualRegisterId(5),
                    VirtualRegisterId(6),
                ],
                &chain.result,
                vec![
                    chain.resident.source_value,
                    chain.middle.source_value,
                    chain.result.source_value,
                ],
            )?,
        ],
        terminator: SelectedTerminator::Return {
            instruction: instruction(
                SelectedInstructionId(8),
                SelectedInstructionKind::ReturnI64,
                keys.return_i64,
                &[VirtualRegisterId(6)],
                SelectedInstructionProvenance {
                    values: vec![source.source_value],
                    edges: vec![source.return_edge],
                    fuel: source.return_fuel.clone(),
                    ..Default::default()
                },
                catalog,
            )?,
            psi_return_edge: source.return_edge,
        },
    })
}
