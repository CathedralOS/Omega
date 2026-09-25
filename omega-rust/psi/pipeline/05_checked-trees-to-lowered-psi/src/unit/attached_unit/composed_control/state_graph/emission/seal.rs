//! Sealing one emitted state: the exit custody its returns and edges
//! dispose of, the selection cleanups its returns publish, the rank its
//! blocks carry, its root block, and the catalog counters and occurrence
//! rows it hands back to the machine emission.

use super::super::super::super::super::{
    CheckedComposedUnitControlTerminatorPlan, StructuralParameterDeclaration, ValueId,
};
use super::super::super::super::{Block, CheckedUnitEffectOperationPlan, Terminator, unsupported};
use super::super::super::LoweringError;
use super::super::{case_emission, edges};
use super::StateGraphEmission;
use crate::emission::operation_emission::buffer::OperationBuffer;

impl StateGraphEmission<'_, '_> {
    /// Close the state at `position` with `terminator` and publish its
    /// blocks, edge blocks, counters and occurrence rows.
    pub(super) fn seal_state(
        &mut self,
        position: usize,
        mut terminator: Terminator,
        mut edge_blocks: Vec<Block>,
        mut evaluation: crate::unit::attached_unit::argument_evaluation::Evaluation,
        mut operations: OperationBuffer,
        frame: SealFrame<'_>,
    ) -> Result<(), LoweringError> {
        let SealFrame {
            state_parameters,
            current_rank,
            is_guarded_return,
            dispatches,
            body_end,
            next_value,
            next_block,
            next_edge,
        } = frame;
        let checked = self.checked;
        let plan = self.plan;
        let state = &plan.states[position];
        // Producing a value does not dispose of the remaining entry owners.
        // Reuse the source exit-custody join for structural and Unit returns;
        // the returned owner itself is not a discard.
        for completion in std::iter::once(&mut terminator)
            .chain(edge_blocks.iter_mut().map(|block| &mut block.terminator))
            .filter(|_| !is_guarded_return)
        {
            if let Terminator::ReturnStructural {
                trivial_affine_discards,
                ..
            } = completion
            {
                let source = checked
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == plan.machine)
                    .and_then(|machine| {
                        checked
                            .machine_states(machine)
                            .iter()
                            .find(|source| source.symbol == state.state)
                    })
                    .ok_or(LoweringError::Unsupported(
                        "structural return source state missing",
                    ))?;
                *trivial_affine_discards =
                    edges::return_discards(checked, plan.machine, source, state)?
                        .into_iter()
                        .map(|index| {
                            evaluation.current_structural_place(state_parameters[index].place)
                        })
                        .collect();
            }
        }
        if !is_guarded_return && !evaluation.selection_cleanups.is_empty() {
            match &mut terminator {
                Terminator::ReturnStructural {
                    trivial_affine_discards,
                    ..
                }
                | Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } => {
                    let discards = trivial_affine_discards;
                    let mut local_discards = Vec::new();
                    for operation in state.operations.iter().rev() {
                        if let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                            result,
                            discard_result_on_return,
                            ..
                        }
                        | CheckedUnitEffectOperationPlan::StructuralCall {
                            result,
                            discard_result_on_return,
                            ..
                        }
                        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                            result,
                            discard_result_on_return,
                            ..
                        } = operation
                        {
                            local_discards.push((
                                case_emission::result(state, result.binding_ordinal, &operations)?
                                    .place,
                                *discard_result_on_return,
                            ));
                        }
                    }
                    // Parameter selection sources keep the same roster shape
                    // as the guarded edge: they follow the result rows in
                    // descending authored position so the splice substitutes
                    // each residual join parameter at its own slot.
                    let mut parameter_sources = Vec::new();
                    for cleanup in &evaluation.selection_cleanups {
                        for source in &cleanup.sources {
                            if let Some((position, _)) = evaluation
                                .structural_parameters
                                .iter()
                                .find(|(_, declaration)| declaration.place == *source)
                            {
                                parameter_sources.push((*position, *source));
                            }
                        }
                    }
                    parameter_sources.sort_by_key(|(position, _)| std::cmp::Reverse(*position));
                    parameter_sources.dedup_by_key(|(_, place)| *place);
                    local_discards.extend(
                        parameter_sources
                            .into_iter()
                            .map(|(_, place)| (place, false)),
                    );
                    local_discards.extend(discards.iter().map(|place| (*place, true)));
                    *discards = evaluation.selection_return_discards(local_discards)?;
                }
                // Ordinary successors already partitioned each edge's residual
                // custody inside `successor`; no global return splice applies.
                Terminator::Jump { .. } | Terminator::Conditional { .. } => {}
                _ => {
                    return unsupported(
                        "owned selection residuals crossing authored states require retained cleanup transfer correspondence",
                    );
                }
            }
        }
        if let Some(rank) = current_rank {
            if matches!(
                state.terminator,
                CheckedComposedUnitControlTerminatorPlan::Guarded { .. }
            ) {
                // Ordered guard/return blocks stay inside this authored state;
                // none of their edges asserts an authored decrease.
                self.block_ranks
                    .extend(edge_blocks.iter().map(|block| (block.id, rank)));
                self.rank_edges.extend(edge_blocks.iter().flat_map(|block| {
                    block.terminator.edges().map(|edge| {
                        (
                            edge,
                            (
                                rank,
                                terminal_psi::TerminalNaturalRankComparison::Preserving,
                            ),
                        )
                    })
                }));
                self.rank_edges.extend(terminator.edges().map(|edge| {
                    (
                        edge,
                        (
                            rank,
                            terminal_psi::TerminalNaturalRankComparison::Preserving,
                        ),
                    )
                }));
            }
            // Completed evaluation blocks stay inside this authored state.
            // Their private edges preserve its incoming rank; only the state
            // successor constructed above claims an authored strict decrease.
            self.rank_edges
                .extend(evaluation.blocks.iter().flat_map(|block| {
                    block.terminator.edges().map(|edge| {
                        (
                            edge,
                            (
                                rank,
                                terminal_psi::TerminalNaturalRankComparison::Preserving,
                            ),
                        )
                    })
                }));
        }
        evaluation.remap_transported_call_operands(&mut operations);
        evaluation.blocks.push(Block {
            id: evaluation.current,
            parameters: evaluation.parameters,
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: evaluation.block_structural_parameters,
            operations: operations[evaluation.operation_start
                ..if dispatches || !edge_blocks.is_empty() {
                    body_end
                } else {
                    operations.len()
                }]
                .to_vec(),
            terminator,
        });
        if position != 0 || self.entry_reentered {
            // Argument evaluation can split the body; bindings belong to its source root.
            // Claim-aliased parameters keep the entry parameter's place, so
            // they are not block parameters either.
            let aliased = &self.admitted.claim_transport.aliased;
            let root = evaluation
                .blocks
                .iter_mut()
                .find(|block| block.id == self.state_ids[position])
                .ok_or(LoweringError::Unsupported(
                    "Unit graph state root disappeared during evaluation",
                ))?;
            root.structural_parameters = std::mem::take(&mut self.state_views[position])
                .into_iter()
                .enumerate()
                .filter(|(dense, parameter)| {
                    !parameter.is_self
                        && !aliased
                            .get(position)
                            .is_some_and(|aliased| aliased.contains_key(&(*dense as u32)))
                })
                .map(|(_, parameter)| parameter)
                .collect();
            // The authored state's own erased roster rides on its root block;
            // a plain entry's formals already live on the machine contract.
            root.erased_scalar_formals = self.state_erased[position].clone();
            root.erased_proof_formals =
                crate::scalar_graph::scalar_contracts::erased_proof_formal_declarations(
                    &self.state_erased_proof[position],
                );
        }
        if let Some(rank) = current_rank {
            self.block_ranks
                .extend(evaluation.blocks.iter().map(|block| (block.id, rank)));
        }
        self.blocks.extend(evaluation.blocks);
        self.blocks.extend(edge_blocks);
        self.catalogs.next_value = next_value;
        self.catalogs.next_block = next_block;
        self.catalogs.next_edge = next_edge;
        self.catalogs.next_operation = operations.next_identity;
        self.occurrences.retain(operations);
        Ok(())
    }
}

/// What sealing reads from the state's emission besides the pieces it
/// consumes: the state's structural parameters, its entry rank, whether an
/// ordered guard/return owns its exits, whether a guard or case dispatch
/// follows its body and where that body ends, and the counters to hand back.
pub(super) struct SealFrame<'s> {
    pub(super) state_parameters: &'s [StructuralParameterDeclaration],
    pub(super) current_rank: Option<ValueId>,
    pub(super) is_guarded_return: bool,
    pub(super) dispatches: bool,
    pub(super) body_end: usize,
    pub(super) next_value: u64,
    pub(super) next_block: u64,
    pub(super) next_edge: u64,
}
