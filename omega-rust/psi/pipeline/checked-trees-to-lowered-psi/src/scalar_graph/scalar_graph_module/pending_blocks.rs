//! The reserved conditional-binding and mixed-tuple block groups, finalized
//! after every state has been emitted so their identities stay dense.

use super::super::{
    Block, LoweringError, Terminator, block_id, boolean_decision_block_count,
    contains_short_circuit, edge_id, emit_direct_expression,
    emit_reserved_boolean_tuple_stage_blocks, lower_boolean_value_decision,
};
use super::GraphEmission;
use crate::emission::boolean_control::PendingNestedBlockGroup;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;

impl GraphEmission<'_> {
    /// Append the inlined blocks, then finalize every pending group in
    /// identity order.
    pub(super) fn resolve_pending_blocks(&mut self) -> Result<(), LoweringError> {
        self.blocks.extend(std::mem::take(&mut self.inlined_blocks));
        self.pending_blocks
            .sort_by_key(PendingNestedBlockGroup::first_id);
        for pending in std::mem::take(&mut self.pending_blocks) {
            match pending {
                PendingNestedBlockGroup::ConditionalBinding(pending) => {
                    let operation_start = self.all_operations.len();
                    let arguments = pending
                        .arguments
                        .iter()
                        .map(|argument| {
                            emit_direct_expression(
                                argument,
                                &pending.parameters,
                                &mut self.next_value_identity,
                                &mut self.all_operations,
                            )
                        })
                        .collect();
                    let edge = edge_id(self.next_edge_identity);
                    self.next_edge_identity = self
                        .next_edge_identity
                        .checked_add(1)
                        .expect("conditional binding jump edge identities advance");
                    self.blocks.push(Block {
                        structural_parameters: Vec::new(),
                        id: pending.id,
                        parameters: pending.parameters,
                        erased_scalar_formals: Vec::new(),
                        operations: self.all_operations[operation_start..].to_vec(),
                        terminator: Terminator::Jump {
                            structural_arguments: Vec::new(),
                            edge,
                            target: pending.target,
                            arguments,
                            erased_arguments: Vec::new(),
                            residual_affine_discards: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    });
                }
                PendingNestedBlockGroup::TupleBinding(pending) => {
                    let mut pending_stage_blocks = Vec::new();
                    let mut next_stage_identity = pending.first_id.get();
                    for (index, argument) in pending.arguments.iter().enumerate() {
                        let parameters = &pending.stage_parameters[index];
                        let carried_arguments = parameters
                            .iter()
                            .map(|parameter| parameter.id)
                            .collect::<Vec<_>>();
                        if let LoweredDirectExpression::Boolean { expression } = argument
                            && contains_short_circuit(expression)
                        {
                            let decision = lower_boolean_value_decision(expression);
                            let stage_block_count = boolean_decision_block_count(&decision);
                            let next_stage =
                                block_id(
                                    next_stage_identity
                                        .checked_add(u64::try_from(stage_block_count).expect(
                                            "mixed tuple stage count fits a semantic identity",
                                        ))
                                        .expect("mixed tuple stage block identities advance"),
                                );
                            let mut stage_blocks = Vec::with_capacity(stage_block_count);
                            let entry = emit_reserved_boolean_tuple_stage_blocks(
                                &decision,
                                parameters,
                                parameters.clone(),
                                next_stage,
                                &carried_arguments,
                                next_stage_identity,
                                &mut self.next_value_identity,
                                &mut self.next_edge_identity,
                                &mut self.all_operations,
                                &mut stage_blocks,
                            );
                            assert_eq!(entry.get(), next_stage_identity);
                            pending_stage_blocks.extend(stage_blocks);
                            next_stage_identity = next_stage.get();
                        } else {
                            let operation_start = self.all_operations.len();
                            let value = emit_direct_expression(
                                argument,
                                parameters,
                                &mut self.next_value_identity,
                                &mut self.all_operations,
                            );
                            let mut arguments = carried_arguments;
                            arguments.push(value);
                            let next_stage = block_id(
                                next_stage_identity
                                    .checked_add(1)
                                    .expect("mixed tuple stage block identity advances"),
                            );
                            let edge = edge_id(self.next_edge_identity);
                            self.next_edge_identity = self
                                .next_edge_identity
                                .checked_add(1)
                                .expect("mixed tuple stage edge identity advances");
                            pending_stage_blocks.push(Some(Block {
                                structural_parameters: Vec::new(),
                                id: block_id(next_stage_identity),
                                parameters: parameters.clone(),
                                erased_scalar_formals: Vec::new(),
                                operations: self.all_operations[operation_start..].to_vec(),
                                terminator: Terminator::Jump {
                                    structural_arguments: Vec::new(),
                                    edge,
                                    target: next_stage,
                                    arguments,
                                    erased_arguments: Vec::new(),
                                    residual_affine_discards: Vec::new(),
                                    trivial_affine_discards: Vec::new(),
                                },
                            }));
                            next_stage_identity = next_stage.get();
                        }
                    }
                    let parameters = pending
                        .stage_parameters
                        .last()
                        .expect("mixed tuple has a convergence parameter set");
                    let edge = edge_id(self.next_edge_identity);
                    self.next_edge_identity = self
                        .next_edge_identity
                        .checked_add(1)
                        .expect("mixed tuple convergence edge identity advances");
                    pending_stage_blocks.push(Some(Block {
                        structural_parameters: Vec::new(),
                        id: block_id(next_stage_identity),
                        parameters: parameters.clone(),
                        erased_scalar_formals: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Jump {
                            structural_arguments: Vec::new(),
                            edge,
                            target: pending.target,
                            arguments: parameters[pending.original_parameter_count..]
                                .iter()
                                .map(|parameter| parameter.id)
                                .collect(),
                            erased_arguments: Vec::new(),
                            residual_affine_discards: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    }));
                    self.blocks
                        .extend(pending_stage_blocks.into_iter().map(|block| {
                            block.expect("every reserved mixed tuple block is finalized")
                        }));
                }
            }
        }
        Ok(())
    }
}
