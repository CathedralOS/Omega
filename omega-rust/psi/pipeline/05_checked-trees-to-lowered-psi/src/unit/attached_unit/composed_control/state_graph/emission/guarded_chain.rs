//! The later guards of an ordered multi-arm tail: each is observed in its
//! own private decision block, reached only along the previous decision's
//! false edge. Their evaluation drafts are staged here, before the
//! successor-edge closure exists, and the terminator assembles the chain.

use super::super::super::super::super::{BlockId, StructuralParameterDeclaration, block_id};
use super::super::super::super::{Operation, ValueDeclaration, allocate_dense};
use super::super::super::LoweringError;
use super::StateGraphEmission;
use super::state::{ChainGuard, plan_short_circuit_guard, successor_may_read_payloads};
use crate::emission::operation_emission::buffer::OperationBuffer;

/// One staged chain arm: its decision block, the parameters and structural
/// parameters that block gained while its guard evaluated, the guard's
/// operations, how the guard is observed, and the value namespace after it.
pub(super) type GuardedDraft = (
    BlockId,
    Vec<ValueDeclaration>,
    Vec<StructuralParameterDeclaration>,
    Vec<Operation>,
    ChainGuard,
    Vec<ValueDeclaration>,
);

impl StateGraphEmission<'_, '_> {
    /// Stage every guard after the first of `arms`: returns the decision
    /// blocks, the staged drafts, and the namespace the first guard observes.
    pub(super) fn stage_guarded_chain(
        &mut self,
        position: usize,
        arms: &[typed_trees_to_checked_trees::checked_trees::CheckedGuardedJumpPlan],
        values: &mut Vec<ValueDeclaration>,
        evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<(Vec<BlockId>, Vec<GuardedDraft>, Vec<ValueDeclaration>), LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let state = &plan.states[position];
        let mut guarded_decisions = Vec::new();
        let mut guarded_drafts = Vec::new();
        let guarded_first_namespace = values.clone();
        for _ in 1..arms.len() {
            guarded_decisions.push(block_id(allocate_dense(next_block)?));
        }
        let (resume_current, resume_start, entry_parameters, entry_structural) = (
            evaluation.current,
            evaluation.operation_start,
            std::mem::take(&mut evaluation.parameters),
            std::mem::take(&mut evaluation.block_structural_parameters),
        );
        for (index, arm) in arms.iter().enumerate().skip(1) {
            let decision = guarded_decisions[index - 1];
            evaluation.current = decision;
            evaluation.operation_start = operations.len();
            evaluation.parameters = Vec::new();
            evaluation.block_structural_parameters = Vec::new();
            // A short-circuit guard stages the same planned decision a
            // two-arm conditional uses; any other guard is one value.
            let reads_payloads = successor_may_read_payloads(&arm.successor);
            let guard = if let Some(expression) = evaluation.branch_guard(
                checked,
                plan.machine,
                state.state,
                arm.successor.statement_ordinal,
                values,
                reads_payloads,
            )? {
                ChainGuard::Decision(plan_short_circuit_guard(
                    &expression,
                    values,
                    &self.catalogs.structural_types,
                    &evaluation.structural_parameters,
                    next_value,
                    reads_payloads,
                )?)
            } else {
                let mut calls = self.catalogs.scalar_calls.emission_context();
                let guard = evaluation.guard_value(
                    checked,
                    plan.machine,
                    state.state,
                    arm.successor.statement_ordinal,
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                    &mut calls,
                )?;
                self.catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
                ChainGuard::Value(guard.id)
            };
            let expanded = evaluation.current != decision;
            guarded_drafts.push((
                evaluation.current,
                if expanded {
                    std::mem::take(&mut evaluation.parameters)
                } else {
                    Vec::new()
                },
                if expanded {
                    std::mem::take(&mut evaluation.block_structural_parameters)
                } else {
                    Vec::new()
                },
                operations[evaluation.operation_start..].to_vec(),
                guard,
                values.clone(),
            ));
        }
        evaluation.current = resume_current;
        evaluation.operation_start = resume_start;
        evaluation.parameters = entry_parameters;
        evaluation.block_structural_parameters = entry_structural;
        Ok((guarded_decisions, guarded_drafts, guarded_first_namespace))
    }
}
