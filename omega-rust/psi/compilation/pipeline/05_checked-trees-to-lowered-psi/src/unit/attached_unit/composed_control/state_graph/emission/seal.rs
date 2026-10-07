//! Sealing one emitted state: the exit custody its returns and edges
//! dispose of, the selection cleanups its returns publish, the rank its
//! blocks carry, its root block, and the catalog counters and occurrence
//! rows it hands back to the machine emission.

use super::super::super::super::super::{
    CheckedComposedUnitControlTerminatorPlan, StructuralParameterDeclaration, ValueId,
};
use super::super::super::super::{Block, Terminator, unsupported};
use super::super::super::LoweringError;
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
                cleanup_actions, ..
            } = completion
            {
                let statement_index = match &state.terminator {
                    CheckedComposedUnitControlTerminatorPlan::ReturnStructural { .. } => {
                        // Admission authenticated a single final expression or
                        // unconditional value transition after the body.
                        let (_, authored) =
                            crate::declarations::state_declaration(checked, state.state)?;
                        checked
                            .statement_table
                            .statements(authored.statement_nodes)
                            .len()
                            .checked_sub(1)
                            .ok_or(LoweringError::Unsupported(
                                "structural return has no source statement",
                            ))?
                    }
                    CheckedComposedUnitControlTerminatorPlan::ReturnCase { result } => {
                        result.statement_ordinal as usize
                    }
                    _ => {
                        return unsupported(
                            "structural return lost its authored completion coordinate",
                        );
                    }
                };
                let exit =
                    super::super::cleanup::coordinate(checked, state, statement_index, false)?;
                *cleanup_actions = super::super::cleanup::for_exit(
                    checked,
                    state,
                    exit,
                    self.catalogs,
                    state_parameters,
                    &evaluation,
                    &operations,
                )?;
            }
        }
        // Scalar exits settled their selection receipts on their private
        // completion edges. Repeating the splice here would recreate owners
        // after those paths have already disposed of them.
        if !is_guarded_return
            && !matches!(
                &state.terminator,
                CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
                    completion: checked_trees::CheckedScalarReturnPlan::Exits(_),
                }
            )
            && !evaluation.selection_cleanups.is_empty()
        {
            match &mut terminator {
                Terminator::ReturnStructural {
                    cleanup_actions, ..
                }
                | Terminator::ReturnUnit {
                    cleanup_actions, ..
                }
                | Terminator::Return {
                    cleanup_actions, ..
                } => {
                    super::super::cleanup::selection(
                        state,
                        &evaluation,
                        &operations,
                        cleanup_actions,
                    )?;
                }
                // Ordinary successors already partitioned each edge's residual
                // custody inside `successor`; no global return splice applies.
                Terminator::Jump { .. }
                | Terminator::Conditional { .. }
                | Terminator::ExternalCompletion { .. } => {}
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
            // A chain member holding moved storage is declared as that owned
            // storage now, after the body emitted against its exclusive view
            // and before private-borrow transport would forward it. A shared
            // parameter holding a forwarded exclusive loan is declared as that
            // loan the same way, after its body emitted against the shared view.
            let loan_chains = &self.loan_chains;
            let exclusive_handoffs = self.admitted.exclusive_handoffs.get(position);
            root.structural_parameters = std::mem::take(&mut self.state_views[position])
                .into_iter()
                .enumerate()
                .filter(|(dense, _)| {
                    !aliased
                        .get(position)
                        .is_some_and(|aliased| aliased.contains_key(&(*dense as u32)))
                })
                .map(|(dense, mut parameter)| {
                    loan_chains.declare(position, dense as u32, &mut parameter);
                    if exclusive_handoffs.is_some_and(|promoted| promoted.contains(&(dense as u32)))
                    {
                        parameter.access = terminal_psi::StructuralAccess::MutableBorrow;
                    }
                    parameter
                })
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
        crate::scalar_graph::scalar_computations::literals::retain_declarations(
            &evaluation.literals,
            evaluation
                .blocks
                .iter()
                .chain(&edge_blocks)
                .flat_map(|block| &block.operations),
            &mut self.catalogs.temporary_places,
        )?;
        evaluation.blocks.extend(edge_blocks);
        crate::emission::private_borrow_transport::forward_private_state_borrows(
            self.state_ids[position],
            &mut evaluation.blocks,
            &self.catalogs.structural_types,
            &mut self.catalogs.next_place,
            &mut self.structural_places,
        )?;
        self.blocks.extend(evaluation.blocks);
        self.catalogs.next_value = next_value;
        self.catalogs.next_block = next_block;
        self.catalogs.next_edge = next_edge;
        self.catalogs.next_operation = operations.next_identity;
        self.scalar_block_invariants
            .append(&mut operations.scalar_block_invariants);
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
