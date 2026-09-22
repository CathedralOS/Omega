//! Validation of sparse conditional constant propagation rewrites.

use crate::{
    ModuleError, reconstruct_optimizable_crash_obligations,
    reconstruct_optimizable_terminal_obligations, validate_module_for_optimization,
};
use semantic_vocabulary::{MachineId, ValueId};
use std::collections::BTreeMap;
use terminal_psi::TerminalModule;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SparseConditionalConstantPropagationRewriteError {
    InvalidModule(ModuleError),
    ChangedProgramStructure,
    ChangedMachine(MachineId),
    ChangedProofQuestion,
}

/// Check the exact constant-fold relation, not the producer's literal scan.
///
/// The fold is reconstructed from `before` alone: scanning reachable blocks in
/// reverse postorder meets every dominating definition first, so a literal
/// binding is recorded before each read. A goal-free scalar leaf whose
/// operands all resolve to literals — literal operation results and results
/// of leaves already folded under this same rule — rewrites in place to the
/// literal operation computing its denotation. The folded operation keeps its
/// identity, result declaration, and position; blocks, edges, parameters, and
/// every value identity are unchanged. Ranked machines carry ranking evidence
/// over execution positions and pass through unchanged.
///
/// A proof-bearing scalar leaf whose canonical goal literal-decides under the
/// same scan is never rewritten by this rule — its obligation belongs to the
/// reconstructed question — but the constant it computes still binds its
/// result value in `literals`, so a later goal-free leaf reading it folds.
///
/// A `before` module carrying reconstructed proof obligations admits only a
/// rewrite the question carries verbatim: the unchanged-question check above
/// is the refusal boundary, so a fold that leaves the question intact is
/// checked by the same fold relation while a fold that perturbs it fails
/// `ChangedProofQuestion` until proof-context transport is implemented.
pub fn validate_sparse_conditional_constant_propagation(
    before: &TerminalModule,
    after: &TerminalModule,
) -> Result<(), SparseConditionalConstantPropagationRewriteError> {
    use SparseConditionalConstantPropagationRewriteError as RewriteError;
    let before_valid =
        validate_module_for_optimization(before).map_err(RewriteError::InvalidModule)?;
    let after_valid =
        validate_module_for_optimization(after).map_err(RewriteError::InvalidModule)?;
    if before.machines.len() != after.machines.len() {
        return Err(RewriteError::ChangedProgramStructure);
    }
    let old_question = reconstruct_optimizable_terminal_obligations(before_valid)
        .map_err(RewriteError::InvalidModule)?;
    let new_question = reconstruct_optimizable_terminal_obligations(after_valid)
        .map_err(RewriteError::InvalidModule)?;
    if old_question != new_question {
        return Err(RewriteError::ChangedProofQuestion);
    }
    // The crash-obligation roster is a second reconstructed question: a
    // rewrite that leaves every terminal obligation intact may still retarget
    // a crash site's reconstructed paths or a continuation's coverage goal,
    // and the supplied certificates answer only the questions they were
    // produced against.
    let old_crash = reconstruct_optimizable_crash_obligations(before_valid)
        .map_err(RewriteError::InvalidModule)?;
    let new_crash = reconstruct_optimizable_crash_obligations(after_valid)
        .map_err(RewriteError::InvalidModule)?;
    if old_crash != new_crash {
        return Err(RewriteError::ChangedProofQuestion);
    }
    let mut expected = before.clone();
    for ((old, new), expected_machine) in before
        .machines
        .iter()
        .zip(&after.machines)
        .zip(&mut expected.machines)
    {
        if old.id != new.id || old.blocks.len() != new.blocks.len() {
            return Err(RewriteError::ChangedProgramStructure);
        }
        if old.ranked_scc.is_some() {
            if new != old {
                return Err(RewriteError::ChangedMachine(old.id));
            }
            continue;
        }
        let mut value_types = BTreeMap::new();
        for parameter in &old.parameters {
            value_types.insert(parameter.id, parameter.scalar_type);
        }
        if let Some(result) = old.result.scalar() {
            value_types.insert(result.id, result.scalar_type);
        }
        let mut block_positions = BTreeMap::new();
        for (position, block) in old.blocks.iter().enumerate() {
            block_positions.insert(block.id, position);
            for parameter in &block.parameters {
                value_types.insert(parameter.id, parameter.scalar_type);
            }
            for operation in &block.operations {
                if let Some(result) = operation.result.scalar() {
                    value_types.insert(result.id, result.scalar_type);
                }
            }
        }
        let mut literals: BTreeMap<ValueId, terminal_semantics::ScalarLeafLiteral> =
            BTreeMap::new();
        for block_id in crate::control_graph::reverse_postorder(old) {
            let position = block_positions
                .get(&block_id)
                .copied()
                .ok_or(RewriteError::ChangedProgramStructure)?;
            for (operation_position, operation) in
                old.blocks[position].operations.iter().enumerate()
            {
                let Some(result) = operation.result.scalar() else {
                    continue;
                };
                match &operation.kind {
                    terminal_psi::OperationKind::IntegerConstant { value } => {
                        literals.insert(
                            result.id,
                            terminal_semantics::ScalarLeafLiteral::Integer(*value),
                        );
                        continue;
                    }
                    terminal_psi::OperationKind::BooleanConstant { value } => {
                        literals.insert(
                            result.id,
                            terminal_semantics::ScalarLeafLiteral::Boolean(*value),
                        );
                        continue;
                    }
                    _ => {}
                }
                // A static reach binder position is semantic call evidence
                // carried by the operation row, never a fold candidate.
                if operation.static_reach_binding.is_some() {
                    continue;
                }
                let Some(literal) = terminal_semantics::constant_goal_free_scalar_leaf(
                    operation,
                    &literals,
                    &value_types,
                ) else {
                    // A proof-bearing leaf whose canonical goal literal-decides
                    // is not rewritten here — its obligation still enters the
                    // reconstructed question — but the constant it computes
                    // carries to every later fold that reads its result.
                    if let Some(elision) = terminal_semantics::elidable_proof_bearing_scalar_leaf(
                        operation,
                        &literals,
                        &value_types,
                    )
                    .map_err(|error| {
                        RewriteError::InvalidModule(ModuleError::OperationSemanticSchema(error))
                    })? && let Some(literal) = elision.result_literal()
                    {
                        literals.insert(
                            result.id,
                            terminal_semantics::ScalarLeafLiteral::Integer(literal),
                        );
                    }
                    continue;
                };
                literals.insert(result.id, literal);
                expected_machine.blocks[position].operations[operation_position].kind =
                    match literal {
                        terminal_semantics::ScalarLeafLiteral::Integer(value) => {
                            terminal_psi::OperationKind::IntegerConstant { value }
                        }
                        terminal_semantics::ScalarLeafLiteral::Boolean(value) => {
                            terminal_psi::OperationKind::BooleanConstant { value }
                        }
                    };
            }
        }
        if expected_machine != new {
            return Err(RewriteError::ChangedMachine(old.id));
        }
    }
    if &expected != after {
        return Err(RewriteError::ChangedProgramStructure);
    }
    Ok(())
}
