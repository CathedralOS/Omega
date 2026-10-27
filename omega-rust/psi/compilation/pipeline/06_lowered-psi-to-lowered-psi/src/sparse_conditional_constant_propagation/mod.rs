//! Optimizer module role: stage group. Sparse conditional constant propagation.
//!
//! A goal-free scalar leaf whose operands all resolve to literals — literal
//! operation results or results of leaves already folded under this rule — is
//! a constant computation: it rewrites in place to the literal operation
//! computing its denotation. The folded operation keeps its identity, result
//! declaration, and position, so every value identity, block, edge, debug
//! subject, and qualification roster survives unchanged. Nothing is removed;
//! literal producers a fold leaves unreferenced remain for the dead-scalar
//! rule when it is also selected.
//!
//! Propositions, ranking rows, and custody sidecars name value identities
//! without listing direct uses: folding never removes an identity, and a
//! ranked machine is left unchanged. Inside a proof-bearing module a fold
//! survives only while the reconstructed proof question is unchanged; a fold
//! the question cannot carry verbatim refuses, leaving the complete closure
//! as authored until proof-context transport lets the question move with the
//! rewrite.
//!
//! The scan folds each operation in place. The corpus fixtures observe the
//! rewrite; the stage does not re-derive its own output (verification
//! placement, `omega-rust/pipeline.md`).

mod folding;

use crate::PsiOptimizationStageError;
use lowered_psi::LoweredPsi;

/// Every scalar the machine names and its declared type — machine
/// parameters and result, then block parameters and operation results.
fn machine_scalar_value_types(
    machine: &terminal_psi::TerminalMachine,
) -> std::collections::BTreeMap<semantic_vocabulary::ValueId, semantic_vocabulary::ScalarType> {
    let mut value_types = std::collections::BTreeMap::new();
    for parameter in &machine.parameters {
        value_types.insert(parameter.id, parameter.scalar_type);
    }
    if let Some(result) = machine.result.scalar() {
        value_types.insert(result.id, result.scalar_type);
    }
    for block in &machine.blocks {
        for parameter in &block.parameters {
            value_types.insert(parameter.id, parameter.scalar_type);
        }
        for operation in &block.operations {
            if let Some(result) = operation.result.scalar() {
                value_types.insert(result.id, result.scalar_type);
            }
        }
    }
    value_types
}

pub(super) fn propagate(before: LoweredPsi) -> Result<LoweredPsi, PsiOptimizationStageError> {
    let mut after = before.clone();
    for machine in &mut after.semantic_module.machines {
        folding::fold(
            machine,
            &before.semantic_module.boundary_application_demands,
        );
    }
    if let Some(debug) = after.debug_map.as_mut() {
        debug.semantic = terminal_codec::terminal_psi_identity(&after.semantic_module)
            .map_err(PsiOptimizationStageError::InvalidSemantic)?;
    }
    Ok(after)
}
