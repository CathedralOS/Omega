//! Control-flow and copy-propagation fixture programs.

use super::*;

pub(crate) fn propagated_block_parameter_unit(constant: bool) -> PsiOptimizationUnit {
    let machine = id(601, MachineId::new);
    let entry = id(602, BlockId::new);
    let when_true = id(603, BlockId::new);
    let when_false = id(604, BlockId::new);
    let merge = id(605, BlockId::new);
    let condition = id(606, ValueId::new);
    let true_value = id(607, ValueId::new);
    let false_value = id(608, ValueId::new);
    let parameter = id(609, ValueId::new);
    let result = id(610, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let binding = |argument| ValueBinding {
        parameter,
        argument,
        scalar_type,
    };
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([21; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            placed_view_inputs: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: when_true,
                        parameters: Vec::new(),
                        operation_offset: 2,
                    },
                    AbstractBlockEntry {
                        block: when_false,
                        parameters: Vec::new(),
                        operation_offset: 4,
                    },
                    AbstractBlockEntry {
                        block: merge,
                        parameters: vec![AbstractParameter {
                            value: parameter,
                            scalar_type,
                        }],
                        operation_offset: 6,
                    },
                ],
                operations: vec![
                    AbstractOperation::BooleanConstant {
                        psi_operation: id(611, OperationId::new),
                        result: condition,
                        value: constant,
                    },
                    AbstractOperation::Conditional {
                        condition,
                        when_true: AbstractSuccessor {
                            psi_edge: id(612, EdgeId::new),
                            target: when_true,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: AbstractSuccessor {
                            psi_edge: id(613, EdgeId::new),
                            target: when_false,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    AbstractOperation::IntegerConstant {
                        psi_operation: id(614, OperationId::new),
                        result: true_value,
                        scalar_type,
                        value: IntegerValue::Unsigned(7),
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(615, EdgeId::new),
                        target: merge,
                        bindings: vec![binding(true_value)],
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::IntegerConstant {
                        psi_operation: id(616, OperationId::new),
                        result: false_value,
                        scalar_type,
                        value: IntegerValue::Unsigned(8),
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(617, EdgeId::new),
                        target: merge,
                        bindings: vec![binding(false_value)],
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::IntegerBitwiseNot {
                        psi_operation: id(618, OperationId::new),
                        result,
                        scalar_type: integer,
                        operand: parameter,
                    },
                    AbstractOperation::Return {
                        psi_edge: id(619, EdgeId::new),
                        result,
                        value: result,
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(crate) fn constant_conditional_dead_service_unit() -> PsiOptimizationUnit {
    let mut unit = propagated_block_parameter_unit(true);
    let service = id(620, ServiceId::new);
    let operation = id(621, OperationId::new);
    unit.services = vec![ServiceDeclaration {
        id: service,
        identity: "validation::dead-branch-service".into(),
        parents: Vec::new(),
    }]
    .into();
    unit.functions[0].published_service_ceiling = vec![service];
    let rejected = unit.functions[0]
        .blocks
        .iter_mut()
        .find(|block| block.id == id(604, BlockId::new))
        .expect("constant fixture retains its rejected branch");
    rejected.nodes.insert(
        1,
        OptimizationNode {
            operation: AbstractOperation::PortWrite {
                psi_operation: operation,
                service,
                port: 0x3f8,
                value: 0x41,
            },
            provenance: vec![PsiProvenance::Operation(operation)],
            fuel: vec![omega_optimization_unit::FuelSettlement {
                site: PsiProvenance::Operation(operation),
                units: 1,
            }],
            effect: omega_optimization_unit::EffectLink {
                input: 0,
                output: 0,
            },
            definitions: Vec::new(),
            uses: Vec::new(),
            successors: Vec::new(),
            ownership: Vec::new(),
        },
    );
    let mut effect = 0u64;
    for block in &mut unit.functions[0].blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            let node_index = u32::try_from(node_index).expect("fixture node index fits u32");
            for definition in &mut node.definitions {
                if let omega_optimization_unit::ValueDefinitionSite::Node {
                    block: site_block,
                    node: site_node,
                } = &mut definition.site
                {
                    *site_block = block.id;
                    *site_node = node_index;
                }
            }
            for value_use in &mut node.uses {
                value_use.block = block.id;
                value_use.node = node_index;
            }
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect + 1,
            };
            effect += 1;
        }
    }
    unit.root_service_reach.concrete = vec![service];
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    unit
}

pub(crate) fn linear_empty_block_unit() -> PsiOptimizationUnit {
    let machine = id(901, MachineId::new);
    let entry = id(902, BlockId::new);
    let empty = id(903, BlockId::new);
    let target = id(904, BlockId::new);
    let left = id(905, ValueId::new);
    let right = id(906, ValueId::new);
    let first = id(907, ValueId::new);
    let second = id(908, ValueId::new);
    let target_first = id(909, ValueId::new);
    let target_second = id(910, ValueId::new);
    let scalar_type = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 8).expect("valid fixture integer"),
    );
    let parameter = |value| AbstractParameter { value, scalar_type };
    let binding = |parameter, argument| ValueBinding {
        parameter,
        argument,
        scalar_type,
    };
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([31; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            placed_view_inputs: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry,
                parameters: vec![
                    AbstractParameter {
                        value: left,
                        scalar_type,
                    },
                    AbstractParameter {
                        value: right,
                        scalar_type,
                    },
                ],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: empty,
                        parameters: vec![parameter(first), parameter(second)],
                        operation_offset: 1,
                    },
                    AbstractBlockEntry {
                        block: target,
                        parameters: vec![parameter(target_first), parameter(target_second)],
                        operation_offset: 2,
                    },
                ],
                operations: vec![
                    AbstractOperation::Jump {
                        psi_edge: id(911, EdgeId::new),
                        target: empty,
                        bindings: vec![binding(first, left), binding(second, right)],
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(912, EdgeId::new),
                        target,
                        bindings: vec![
                            binding(target_first, second),
                            binding(target_second, first),
                        ],
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(913, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(crate) fn path_qualified_empty_block_unit() -> PsiOptimizationUnit {
    let machine = id(921, MachineId::new);
    let entry = id(922, BlockId::new);
    let left_block = id(923, BlockId::new);
    let right_block = id(924, BlockId::new);
    let empty = id(925, BlockId::new);
    let target = id(926, BlockId::new);
    let condition = id(927, ValueId::new);
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([32; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            placed_view_inputs: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry,
                parameters: vec![AbstractParameter {
                    value: condition,
                    scalar_type: ScalarType::Boolean,
                }],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: left_block,
                        parameters: Vec::new(),
                        operation_offset: 1,
                    },
                    AbstractBlockEntry {
                        block: right_block,
                        parameters: Vec::new(),
                        operation_offset: 2,
                    },
                    AbstractBlockEntry {
                        block: empty,
                        parameters: Vec::new(),
                        operation_offset: 3,
                    },
                    AbstractBlockEntry {
                        block: target,
                        parameters: Vec::new(),
                        operation_offset: 4,
                    },
                ],
                operations: vec![
                    AbstractOperation::Conditional {
                        condition,
                        when_true: AbstractSuccessor {
                            psi_edge: id(931, EdgeId::new),
                            target: left_block,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: AbstractSuccessor {
                            psi_edge: id(932, EdgeId::new),
                            target: right_block,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(933, EdgeId::new),
                        target: empty,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(934, EdgeId::new),
                        target: empty,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(935, EdgeId::new),
                        target,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(936, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(crate) fn shared_terminal_unit() -> PsiOptimizationUnit {
    let machine = id(921, MachineId::new);
    let entry = id(922, BlockId::new);
    let left_block = id(923, BlockId::new);
    let right_block = id(924, BlockId::new);
    let target = id(926, BlockId::new);
    let condition = id(927, ValueId::new);
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([38; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            placed_view_inputs: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry,
                parameters: vec![AbstractParameter {
                    value: condition,
                    scalar_type: ScalarType::Boolean,
                }],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: left_block,
                        parameters: Vec::new(),
                        operation_offset: 1,
                    },
                    AbstractBlockEntry {
                        block: right_block,
                        parameters: Vec::new(),
                        operation_offset: 2,
                    },
                    AbstractBlockEntry {
                        block: target,
                        parameters: Vec::new(),
                        operation_offset: 3,
                    },
                ],
                operations: vec![
                    AbstractOperation::Conditional {
                        condition,
                        when_true: AbstractSuccessor {
                            psi_edge: id(931, EdgeId::new),
                            target: left_block,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: AbstractSuccessor {
                            psi_edge: id(932, EdgeId::new),
                            target: right_block,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(933, EdgeId::new),
                        target,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(934, EdgeId::new),
                        target,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(936, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(crate) fn adjacent_conditional_merge_unit() -> PsiOptimizationUnit {
    let machine = id(1_101, MachineId::new);
    let entry = id(1_102, BlockId::new);
    let decision = id(1_103, BlockId::new);
    let left = id(1_104, BlockId::new);
    let right = id(1_105, BlockId::new);
    let condition = id(1_106, ValueId::new);
    let forwarded = id(1_107, ValueId::new);
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([37; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            placed_view_inputs: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry,
                parameters: vec![AbstractParameter {
                    value: condition,
                    scalar_type: ScalarType::Boolean,
                }],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: decision,
                        parameters: vec![AbstractParameter {
                            value: forwarded,
                            scalar_type: ScalarType::Boolean,
                        }],
                        operation_offset: 1,
                    },
                    AbstractBlockEntry {
                        block: left,
                        parameters: Vec::new(),
                        operation_offset: 2,
                    },
                    AbstractBlockEntry {
                        block: right,
                        parameters: Vec::new(),
                        operation_offset: 3,
                    },
                ],
                operations: vec![
                    AbstractOperation::Jump {
                        psi_edge: id(1_110, EdgeId::new),
                        target: decision,
                        bindings: vec![ValueBinding {
                            parameter: forwarded,
                            argument: condition,
                            scalar_type: ScalarType::Boolean,
                        }],
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::Conditional {
                        condition: forwarded,
                        when_true: AbstractSuccessor {
                            psi_edge: id(1_111, EdgeId::new),
                            target: left,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: AbstractSuccessor {
                            psi_edge: id(1_112, EdgeId::new),
                            target: right,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(1_113, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(1_114, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(crate) fn non_adjacent_merge_unit(target_before_predecessor: bool) -> PsiOptimizationUnit {
    let machine = id(1_501, MachineId::new);
    let entry = id(1_502, BlockId::new);
    let descendant = id(1_503, BlockId::new);
    let target = id(1_504, BlockId::new);
    let sibling = id(1_505, BlockId::new);
    let predecessor = id(1_506, BlockId::new);
    let condition = id(1_507, ValueId::new);
    let incoming = id(1_508, ValueId::new);
    let target_parameter = id(1_509, ValueId::new);
    let target_result = id(1_510, ValueId::new);
    let descendant_result = id(1_511, ValueId::new);
    let predecessor_value = id(1_520, ValueId::new);

    let entry_operation = AbstractOperation::Conditional {
        condition,
        when_true: AbstractSuccessor {
            psi_edge: id(1_512, EdgeId::new),
            target: predecessor,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: AbstractSuccessor {
            psi_edge: id(1_513, EdgeId::new),
            target: sibling,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    let descendant_operations = vec![
        AbstractOperation::BooleanEqual {
            psi_operation: id(1_514, OperationId::new),
            result: descendant_result,
            left: target_parameter,
            right: target_result,
        },
        AbstractOperation::Return {
            psi_edge: id(1_515, EdgeId::new),
            result: descendant_result,
            value: descendant_result,
            scalar_type: ScalarType::Boolean,
            cleanup_actions: Vec::new(),
        },
    ];
    let target_operations = vec![
        AbstractOperation::BooleanNot {
            psi_operation: id(1_516, OperationId::new),
            result: target_result,
            operand: target_parameter,
        },
        AbstractOperation::Jump {
            psi_edge: id(1_517, EdgeId::new),
            target: descendant,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    ];
    let sibling_operation = AbstractOperation::Return {
        psi_edge: id(1_518, EdgeId::new),
        result: descendant_result,
        value: incoming,
        scalar_type: ScalarType::Boolean,
        cleanup_actions: Vec::new(),
    };
    let predecessor_operations = vec![
        AbstractOperation::BooleanNot {
            psi_operation: id(1_521, OperationId::new),
            result: predecessor_value,
            operand: incoming,
        },
        AbstractOperation::Jump {
            psi_edge: id(1_519, EdgeId::new),
            target,
            bindings: vec![ValueBinding {
                parameter: target_parameter,
                argument: predecessor_value,
                scalar_type: ScalarType::Boolean,
            }],
            trivial_affine_discards: Vec::new(),
        },
    ];

    let mut block_entries = Vec::new();
    let mut operations = Vec::new();
    let mut push_block = |block, parameters, block_operations: Vec<_>| {
        block_entries.push(AbstractBlockEntry {
            block,
            parameters,
            operation_offset: operations.len(),
        });
        operations.extend(block_operations);
    };
    push_block(entry, Vec::new(), vec![entry_operation]);
    if target_before_predecessor {
        push_block(descendant, Vec::new(), descendant_operations);
        push_block(
            target,
            vec![AbstractParameter {
                value: target_parameter,
                scalar_type: ScalarType::Boolean,
            }],
            target_operations,
        );
        push_block(sibling, Vec::new(), vec![sibling_operation]);
        push_block(predecessor, Vec::new(), predecessor_operations);
    } else {
        push_block(predecessor, Vec::new(), predecessor_operations);
        push_block(sibling, Vec::new(), vec![sibling_operation]);
        push_block(
            target,
            vec![AbstractParameter {
                value: target_parameter,
                scalar_type: ScalarType::Boolean,
            }],
            target_operations,
        );
        push_block(descendant, Vec::new(), descendant_operations);
    }

    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([44; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            placed_view_inputs: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry,
                parameters: vec![
                    AbstractParameter {
                        value: condition,
                        scalar_type: ScalarType::Boolean,
                    },
                    AbstractParameter {
                        value: incoming,
                        scalar_type: ScalarType::Boolean,
                    },
                ],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: descendant_result,
                    scalar_type: ScalarType::Boolean,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries,
                operations,
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(crate) fn constant_conditional_same_target_unit(constant: bool) -> PsiOptimizationUnit {
    let machine = id(651, MachineId::new);
    let entry = id(652, BlockId::new);
    let merge = id(653, BlockId::new);
    let condition = id(654, ValueId::new);
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([23; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            placed_view_inputs: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: merge,
                        parameters: Vec::new(),
                        operation_offset: 2,
                    },
                ],
                operations: vec![
                    AbstractOperation::BooleanConstant {
                        psi_operation: id(655, OperationId::new),
                        result: condition,
                        value: constant,
                    },
                    AbstractOperation::Conditional {
                        condition,
                        when_true: AbstractSuccessor {
                            psi_edge: id(656, EdgeId::new),
                            target: merge,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: AbstractSuccessor {
                            psi_edge: id(657, EdgeId::new),
                            target: merge,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(658, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(crate) fn redundant_block_parameter_unit(redundant: bool) -> PsiOptimizationUnit {
    let machine = id(701, MachineId::new);
    let entry = id(702, BlockId::new);
    let merge = id(703, BlockId::new);
    let condition = id(704, ValueId::new);
    let shared = id(705, ValueId::new);
    let alternate = id(706, ValueId::new);
    let parameter = id(707, ValueId::new);
    let result = id(708, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let binding = |argument| ValueBinding {
        parameter,
        argument,
        scalar_type,
    };
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([22; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            placed_view_inputs: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry,
                parameters: vec![
                    AbstractParameter {
                        value: condition,
                        scalar_type: ScalarType::Boolean,
                    },
                    AbstractParameter {
                        value: shared,
                        scalar_type,
                    },
                    AbstractParameter {
                        value: alternate,
                        scalar_type,
                    },
                ],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: merge,
                        parameters: vec![AbstractParameter {
                            value: parameter,
                            scalar_type,
                        }],
                        operation_offset: 1,
                    },
                ],
                operations: vec![
                    AbstractOperation::Conditional {
                        condition,
                        when_true: AbstractSuccessor {
                            psi_edge: id(709, EdgeId::new),
                            target: merge,
                            bindings: vec![binding(shared)],
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: AbstractSuccessor {
                            psi_edge: id(710, EdgeId::new),
                            target: merge,
                            bindings: vec![binding(if redundant { shared } else { alternate })],
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    AbstractOperation::ExactIntegerAdd {
                        psi_operation: id(711, OperationId::new),
                        obligation: id(713, ObligationId::new),
                        result,
                        scalar_type: integer,
                        left: parameter,
                        right: alternate,
                    },
                    AbstractOperation::Return {
                        psi_edge: id(712, EdgeId::new),
                        result,
                        value: result,
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}
