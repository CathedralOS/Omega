//! Validation of proof check elision rewrites.

use crate::{
    ModuleError, reconstruct_optimizable_crash_obligations,
    reconstruct_optimizable_terminal_obligations, validate_module_for_optimization,
};
use semantic_vocabulary::{MachineId, ObligationId, OperationId, Proposition, ValueId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{ProofBundle, TerminalModule};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofCheckElisionRewriteError {
    InvalidModule(ModuleError),
    ChangedProgramStructure,
    ChangedMachine(MachineId),
    ChangedProofQuestion,
    ChangedProofEvidence,
}

/// Check the exact discharged-goal rewrite, not the producer's literal scan.
///
/// The elision set is re-derived from `before` alone: scanning reachable
/// blocks in reverse postorder meets every dominating literal definition
/// first, so each proof-bearing scalar leaf's canonical goal is decided under
/// the literal bindings its predecessors actually provide. A discharged leaf
/// rewrites in place to the goal-free leaf the shared semantic judgment
/// selects, keeping its identity, result declaration, and position; a decided
/// leaf with no goal-free form stays checked. Ranked machines carry ranking
/// evidence over exact execution positions and pass through unchanged, and a
/// leaf whose obligation a recursive or control-cycle certificate names keeps
/// its question.
///
/// The reconstructed proof question changes exactly at the consumed rows: the
/// discharged operation's obligation disappears and every surviving row's
/// semantic axioms substitute the original result equation with the
/// replacement's — the same element in the same position. The proof bundle's
/// evidence section loses the consumed obligations' rows; recursive
/// certificates, control-cycle certificates, and producer provenance are
/// unchanged.
pub fn validate_proof_check_elision(
    before: &TerminalModule,
    after: &TerminalModule,
    before_bundle: &ProofBundle,
    after_bundle: &ProofBundle,
) -> Result<(), ProofCheckElisionRewriteError> {
    use ProofCheckElisionRewriteError as RewriteError;
    let before_valid =
        validate_module_for_optimization(before).map_err(RewriteError::InvalidModule)?;
    let after_valid =
        validate_module_for_optimization(after).map_err(RewriteError::InvalidModule)?;
    if before.machines.len() != after.machines.len() {
        return Err(RewriteError::ChangedProgramStructure);
    }
    // Obligation identities a recursive or control-cycle certificate names are
    // consumed by that certificate: they cannot disappear with an operation.
    let mut reserved = BTreeSet::new();
    for certificate in before_bundle
        .recursive_components
        .iter()
        .map(|row| &row.certificate)
        .chain(
            before_bundle
                .control_cycles
                .iter()
                .map(|row| &row.certificate),
        )
    {
        for edge in &certificate.edges {
            reserved.insert(edge.obligation);
        }
    }
    let mut expected = before.clone();
    // Each elided leaf's original result equation maps to the replacement's.
    let mut substitutions: BTreeMap<Proposition, Proposition> = BTreeMap::new();
    let mut elided: BTreeMap<(MachineId, OperationId), ObligationId> = BTreeMap::new();
    let mut elided_obligations = BTreeSet::new();
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
                // carried by the operation row, never an elision candidate.
                if operation.static_reach_binding.is_some() {
                    continue;
                }
                let Some(elision) = terminal_semantics::elidable_proof_bearing_scalar_leaf(
                    operation,
                    &literals,
                    &value_types,
                )
                .map_err(|error| {
                    RewriteError::InvalidModule(ModuleError::OperationSemanticSchema(error))
                })?
                else {
                    continue;
                };
                if reserved.contains(&elision.obligation()) {
                    continue;
                }
                let expected_operation =
                    &mut expected_machine.blocks[position].operations[operation_position];
                expected_operation.kind = elision.replacement().clone();
                let replacement_equation = terminal_semantics::goal_free_scalar_leaf_semantics(
                    expected_operation,
                    &value_types,
                )
                .map_err(|error| {
                    RewriteError::InvalidModule(ModuleError::OperationSemanticSchema(error))
                })?
                .ok_or(RewriteError::ChangedMachine(old.id))?
                .result_equation()
                .clone();
                substitutions.insert(
                    elision.semantics().result_equation().clone(),
                    replacement_equation,
                );
                elided.insert((old.id, operation.id), elision.obligation());
                elided_obligations.insert(elision.obligation());
                if let Some(literal) = elision.result_literal() {
                    literals.insert(
                        result.id,
                        terminal_semantics::ScalarLeafLiteral::Integer(literal),
                    );
                }
            }
        }
        if expected_machine != new {
            return Err(RewriteError::ChangedMachine(old.id));
        }
    }
    if &expected != after {
        return Err(RewriteError::ChangedProgramStructure);
    }
    // The complete reconstructed question changes only at consumed rows: an
    // elided leaf's obligation disappears and its result equation is
    // substituted inside every surviving row's axiom snapshot at the exact
    // position reconstruction places it.
    let old_question = reconstruct_optimizable_terminal_obligations(before_valid)
        .map_err(RewriteError::InvalidModule)?;
    let new_question = reconstruct_optimizable_terminal_obligations(after_valid)
        .map_err(RewriteError::InvalidModule)?;
    let mut remaining = new_question.obligations().iter();
    for old_row in old_question.obligations() {
        if let crate::ReconstructedTerminalObligationOwner::Operation { machine, operation } =
            old_row.owner
            && let Some(obligation) = elided.get(&(machine, operation))
        {
            if old_row.obligation.id != *obligation {
                return Err(RewriteError::ChangedProofQuestion);
            }
            continue;
        }
        let Some(new_row) = remaining.next() else {
            return Err(RewriteError::ChangedProofQuestion);
        };
        if new_row.owner != old_row.owner
            || new_row.obligation != old_row.obligation
            || new_row.requirements != old_row.requirements
            || new_row.canonical_certificate != old_row.canonical_certificate
            || new_row.semantic_axioms.len() != old_row.semantic_axioms.len()
        {
            return Err(RewriteError::ChangedProofQuestion);
        }
        for (old_axiom, new_axiom) in old_row
            .semantic_axioms
            .iter()
            .zip(new_row.semantic_axioms.iter())
        {
            if new_axiom == old_axiom {
                continue;
            }
            if substitutions.get(old_axiom) != Some(new_axiom) {
                return Err(RewriteError::ChangedProofQuestion);
            }
        }
    }
    if remaining.next().is_some() {
        return Err(RewriteError::ChangedProofQuestion);
    }
    // Crash obligations are checked the strict way: the supplied roster is
    // byte-preserved by the bundle comparison below, so its certificates only
    // remain answers when the reconstructed crash question is identical. An
    // elision that would substitute inside a crash site's path axioms or a
    // continuation goal is refused rather than transported.
    let old_crash = reconstruct_optimizable_crash_obligations(before_valid)
        .map_err(RewriteError::InvalidModule)?;
    let new_crash = reconstruct_optimizable_crash_obligations(after_valid)
        .map_err(RewriteError::InvalidModule)?;
    if old_crash != new_crash {
        return Err(RewriteError::ChangedProofQuestion);
    }
    // The consumed obligations' evidence rows leave with them; every other
    // bundle section is byte-identical.
    let mut expected_bundle = before_bundle.clone();
    expected_bundle
        .evidence
        .retain(|row| !elided_obligations.contains(&row.obligation));
    if after_bundle != &expected_bundle {
        return Err(RewriteError::ChangedProofEvidence);
    }
    Ok(())
}
