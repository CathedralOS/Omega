//! Optimizer module role: stage group. Proof-check elision.
//!
//! A proof-bearing scalar leaf whose canonical goal is discharged by literal
//! bindings alone rewrites in place to the goal-free leaf denoting the same
//! result, and its obligation leaves the reconstructed question with it. The
//! rewrite keeps the operation's identity, result declaration, and position;
//! the consumed obligation's evidence row leaves the proof bundle, and every
//! surviving obligation's axiom snapshot substitutes the original result
//! equation with the replacement's.
//!
//! The scan proposes folds; the independent verifier re-derives the elision
//! set from the original module and checks the exact before/after relation,
//! the transported proof question, and the consumed evidence rows.

use std::collections::{BTreeMap, BTreeSet};

use crate::PsiOptimizationStageError;
use lowered_psi::LoweredPsi;
use semantic_vocabulary::{ObligationId, ScalarType, ValueId};
use terminal_psi::{OperationKind, TerminalMachine};
use terminal_semantics::ScalarLeafLiteral;
use terminal_verifier::{validate_module_for_optimization, validate_proof_check_elision};

pub(super) fn elide(before: LoweredPsi) -> Result<LoweredPsi, PsiOptimizationStageError> {
    validate_module_for_optimization(&before.semantic_module)
        .map_err(PsiOptimizationStageError::InvalidModule)?;
    let mut after = before.clone();
    // Obligation identities a recursive or control-cycle certificate names are
    // consumed by that certificate: they cannot leave with an operation.
    let mut reserved = BTreeSet::new();
    for certificate in before
        .proof_bundle
        .recursive_components
        .iter()
        .map(|row| &row.certificate)
        .chain(
            before
                .proof_bundle
                .control_cycles
                .iter()
                .map(|row| &row.certificate),
        )
    {
        for edge in &certificate.edges {
            reserved.insert(edge.obligation);
        }
    }
    let mut elided = BTreeSet::new();
    for machine in &mut after.semantic_module.machines {
        fold_machine(machine, &reserved, &mut elided);
    }
    if !elided.is_empty() {
        after
            .proof_bundle
            .evidence
            .retain(|row| !elided.contains(&row.obligation));
    }
    validate_proof_check_elision(
        &before.semantic_module,
        &after.semantic_module,
        &before.proof_bundle,
        &after.proof_bundle,
    )
    .map_err(PsiOptimizationStageError::InvalidProofCheckElisionRewrite)?;
    if let Some(debug) = after.debug_map.as_mut() {
        debug.semantic = terminal_codec::terminal_psi_identity(&after.semantic_module)
            .map_err(PsiOptimizationStageError::InvalidSemantic)?;
    }
    Ok(after)
}

/// Fold every discharged proof-bearing scalar leaf to its goal-free form.
///
/// Iterating to a fixed point reaches the same elision set a single
/// reverse-postorder pass computes: folding only adds literal bindings and
/// never removes one, and every block in a validated machine is reachable, so
/// the scan order cannot change which leaves fold. The independent verifier
/// replays the relation in graph order.
fn fold_machine(
    machine: &mut TerminalMachine,
    reserved: &BTreeSet<ObligationId>,
    elided: &mut BTreeSet<ObligationId>,
) {
    // Ranking evidence is stated over this machine's exact execution rows;
    // leave it unchanged rather than reason about evidence outside its terms.
    if machine.ranked_scc.is_some() {
        return;
    }
    let mut value_types: BTreeMap<ValueId, ScalarType> = BTreeMap::new();
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
    let mut literals: BTreeMap<ValueId, ScalarLeafLiteral> = BTreeMap::new();
    loop {
        let mut changed = false;
        for block in &mut machine.blocks {
            for operation in &mut block.operations {
                let Some(result) = operation.result.scalar() else {
                    continue;
                };
                match &operation.kind {
                    OperationKind::IntegerConstant { value } => {
                        literals.insert(result.id, ScalarLeafLiteral::Integer(*value));
                        continue;
                    }
                    OperationKind::BooleanConstant { value } => {
                        literals.insert(result.id, ScalarLeafLiteral::Boolean(*value));
                        continue;
                    }
                    _ => {}
                }
                if literals.contains_key(&result.id) {
                    continue;
                }
                // A static reach binder position is semantic call evidence
                // carried by the operation row, never an elision candidate.
                if operation.static_reach_binding.is_some() {
                    continue;
                }
                let Ok(Some(elision)) = terminal_semantics::elidable_proof_bearing_scalar_leaf(
                    operation,
                    &literals,
                    &value_types,
                ) else {
                    continue;
                };
                if reserved.contains(&elision.obligation()) {
                    continue;
                }
                operation.kind = elision.replacement().clone();
                elided.insert(elision.obligation());
                if let Some(literal) = elision.result_literal() {
                    literals.insert(result.id, ScalarLeafLiteral::Integer(literal));
                }
                changed = true;
            }
        }
        if !changed {
            return;
        }
    }
}
