use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult, AbstractSuccessor, ValueBinding,
};
use omega_optimization_unit::{
    PsiOptimizationUnit, PsiProvenance, recompute_psi_optimization_unit_identity,
    reconstruct_psi_optimization_unit_seed,
};
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, ScalarType, ServiceId, ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::super::{OptimizationNode, ServiceDeclaration, id};

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
