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
use semantic_vocabulary::EdgeId;

impl StateGraphEmission<'_, '_> {
    /// Record every edge in `edges` as rank-preserving at `rank`.
    pub(super) fn preserve_edges(&mut self, edges: impl Iterator<Item = EdgeId>, rank: ValueId) {
        self.rank_edges.extend(edges.map(|edge| {
            (
                edge,
                (
                    rank,
                    terminal_psi::TerminalNaturalRankComparison::Preserving,
                ),
            )
        }));
    }

    /// Close the state at `position` with `terminator` and publish its
    /// blocks, edge blocks, counters and occurrence rows.
    pub(super) fn seal_state(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        position: usize,
        mut terminator: Terminator,
        edge_blocks: Vec<Block>,
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
            mut next_value,
            mut next_block,
            mut next_edge,
        } = frame;
        let current_parameters = evaluation.current_parameters(state_parameters);
        let state_parameters = current_parameters.as_ref();
        let plan = self.plan;
        let state = &plan.states[position];
        // The ordinary structural exit has one source-owned disposition
        // schedule. Capture a projected result on its transport edge before
        // selected cleanup; whole results are excluded from that partition.
        if !is_guarded_return
            && let CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result } =
                &state.terminator
        {
            let statement_index = (state.statement_count as usize).checked_sub(1).ok_or(
                LoweringError::Unsupported("structural return has no source statement"),
            )?;
            let exit = super::super::cleanup::coordinate(
                self.catalogs.cleanup_control,
                state,
                statement_index,
                false,
            )?;
            let mut regions = super::super::cleanup::for_exit_regions(
                identities,
                self.claims.published(),
                self.checked,
                state,
                exit,
                self.catalogs,
                state_parameters,
                &evaluation,
                &operations,
            )?;
            super::super::cleanup::selection(&evaluation, &operations, &mut regions)?;
            let mut values = Vec::new();
            if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal,
            } = result.source
            {
                let binding =
                    super::super::case_emission::result(state, binding_ordinal, &operations)?
                        .into_owned();
                let source = evaluation.current_structural_place(binding.place());
                let (completion, arrival) = super::super::returns::projected::materialize(
                    identities,
                    self.checked,
                    &binding,
                    source,
                    terminator,
                    regions,
                    self.catalogs,
                    &mut evaluation,
                    &mut values,
                    &mut next_value,
                    &mut next_block,
                    &mut next_edge,
                    &mut operations,
                )?;
                terminator = completion;
                if let Some(arrival) = arrival {
                    if let Some(rank) = current_rank {
                        self.preserve_edges(terminator.edges(), rank);
                    }
                    evaluation.blocks.push(arrival);
                }
            } else {
                let actions = evaluation.emit_cleanup_regions(
                    identities,
                    self.checked,
                    regions,
                    &mut values,
                    &mut next_value,
                    &mut next_block,
                    &mut next_edge,
                    &mut operations,
                )?;
                let Terminator::ReturnStructural {
                    cleanup_actions, ..
                } = &mut terminator
                else {
                    return unsupported("structural return lost its ordinary completion");
                };
                *cleanup_actions = actions;
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
                self.preserve_edges(
                    edge_blocks
                        .iter()
                        .flat_map(|block| block.terminator.edges()),
                    rank,
                );
                self.preserve_edges(terminator.edges(), rank);
            }
            // Completed evaluation blocks stay inside this authored state.
            // Their private edges preserve its incoming rank; only the state
            // successor constructed above claims an authored strict decrease.
            self.preserve_edges(
                evaluation
                    .blocks
                    .iter()
                    .flat_map(|block| block.terminator.edges()),
                rank,
            );
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
            // Entry-aliased parameters keep the entry parameter's place, so
            // they are not block parameters either.
            let aliased = &self.admitted.entry_aliases;
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
            root.structural_parameters
                .extend(self.captured_loans.declarations(position));
            // The authored state's own erased roster rides on its root block;
            // a plain entry's formals already live on the machine contract.
            root.erased_scalar_formals = self.state_erased[position].clone();
            root.erased_proof_formals =
                crate::scalar_lowering::contracts::erased_proof_formal_declarations(
                    identities,
                    self.checked,
                    &self.state_erased_proof[position],
                );
        }
        if let Some(rank) = current_rank {
            self.block_ranks
                .extend(evaluation.blocks.iter().map(|block| (block.id, rank)));
        }
        crate::emission::structural_byte_sequence_store::retain_literal_declarations(
            &operations,
            &mut self.catalogs.temporary_places,
        )?;
        evaluation.blocks.extend(edge_blocks);
        super::super::cleanup::retire_captured_subjects(state, &operations, &mut evaluation.blocks);
        crate::emission::private_borrow_transport::forward_private_state_borrows(
            self.state_ids[position],
            &mut evaluation.blocks,
            &self.catalogs.structural_types,
            &mut self.catalogs.next_place,
            &mut self.structural_places,
        )?;
        // Qualification edges name the emitted machine, not a fused source
        // owner. Publish their exact edge/value correspondence with its body.
        self.qualification_coercions
            .append(&mut evaluation.qualification_coercions);
        self.blocks.extend(evaluation.blocks);
        self.catalogs.next_value = next_value;
        self.catalogs.next_block = next_block;
        self.catalogs.next_edge = next_edge;
        self.catalogs.next_operation = operations.next_identity;
        self.scalar_block_invariants
            .append(&mut operations.scalar_block_invariants);
        self.loan_completions
            .append(&mut operations.loan_completions);
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
