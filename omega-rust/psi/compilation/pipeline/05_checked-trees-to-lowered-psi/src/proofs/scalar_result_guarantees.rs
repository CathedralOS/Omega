//! Export demanded scalar result relations through ordinary proved guarantees.
//!
//! A call preserves its result carrier, but the carrier alone loses relations
//! to actual parameters. Follow pending proof demands to scalar callees and
//! propose the parameter-only denotation of their emitted return values. The
//! operation semantic tables own arithmetic policy; this pass never evaluates
//! source syntax or specializes a body for one caller. Every normal return must
//! propose the same relation, and ordinary ContractEnsures reconstruction and
//! proof admission must establish it before it becomes a caller fact.

use std::collections::{BTreeMap, BTreeSet};

use lowered_psi::LoweredPsi;
use proof_admission::{PredicateDenotationError, check_value_equality_denotation};
use semantic_vocabulary::{
    MachineId, ObligationId, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::{ContractClause, OperationKind, TerminalMachine, Terminator};
use terminal_verifier::{ReconstructedTerminalObligationOwner, ReconstructedTerminalObligationSet};

use super::ProofSearchDeclines;
use super::nonzero_divisor_certificate::{
    produce_checked_canonical_integer_proof, produce_relaxed_integer_proof,
};
use crate::lowering_error::{LoweringError, ProofSearchCapacity};

#[cfg(test)]
mod tests;

pub(super) fn retain_provable(
    lowered: &mut LoweredPsi,
    declines: &mut ProofSearchDeclines,
) -> Result<(), LoweringError> {
    let original = terminal_verifier::reconstruct_terminal_obligations(&lowered.semantic_module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    let mut remaining = ProofSearchCapacity::ScalarResultGuaranteeWork.limit() as usize;
    let demanded = demanded_callees(lowered, &original, &mut remaining);
    let validated = terminal_verifier::validate_module(&lowered.semantic_module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    let mut candidates = Vec::new();
    for machine in &lowered.semantic_module.machines {
        if remaining == 0 {
            break;
        }
        if !demanded.contains(&machine.id) {
            continue;
        }
        let context = validated
            .value_context(machine)
            .map_err(LoweringError::InvalidTerminalModule)?;
        if let Some(proposition) = result_relation(machine, &context, &mut remaining)
            && !machine
                .contract
                .ensures
                .iter()
                .any(|clause| clause.proposition == proposition)
        {
            candidates.push((machine.id, proposition));
        }
    }
    if remaining == 0 {
        declines.record(ProofSearchCapacity::ScalarResultGuaranteeWork);
    }
    if candidates.is_empty() {
        return Ok(());
    }
    let mut next = terminal_verifier::maximum_registered_obligation_id(&lowered.semantic_module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    let mut added = Vec::new();
    for (owner, proposition) in candidates {
        let Some(fresh) = next.checked_add(1).and_then(ObligationId::new) else {
            declines.record(ProofSearchCapacity::TerminalObligationIdentities);
            break;
        };
        next = fresh.get();
        let machine = lowered
            .semantic_module
            .machines
            .iter_mut()
            .find(|machine| machine.id == owner)
            .ok_or(LoweringError::Unsupported(
                "result guarantee lost its owner",
            ))?;
        machine.contract.ensures.push(ContractClause {
            proposition,
            obligation: fresh,
        });
        added.push(fresh);
    }
    // The batch is optional. Preserve every authored clause and supplied proof
    // if its new contract facts cannot all be admitted together. Finalization
    // reconstructs and emits evidence again against the retained module.
    // A valid module may have consumers that forbid additional guarantees
    // (for example, dynamic realizations). Such a proposal also declines.
    let proved = matches!(
        proves_and_preserves_evidence(lowered, &original, &added),
        Ok(true)
    );
    if !proved {
        for machine in &mut lowered.semantic_module.machines {
            machine
                .contract
                .ensures
                .retain(|clause| !added.contains(&clause.obligation));
        }
    }
    Ok(())
}

/// Value identities are machine-scoped. Each site's directed definition slice
/// stays in its owner; a call result selects a callee rather than crossing into
/// the callee's unrelated value namespace.
fn demanded_callees(
    lowered: &LoweredPsi,
    original: &ReconstructedTerminalObligationSet,
    remaining: &mut usize,
) -> BTreeSet<MachineId> {
    let supplied = lowered
        .proof_bundle
        .evidence
        .iter()
        .map(|proof| proof.obligation)
        .collect::<BTreeSet<_>>();
    let mut demanded = BTreeSet::new();
    for machine in &lowered.semantic_module.machines {
        let mut calls = BTreeMap::new();
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            if !charge(remaining, 1) {
                return demanded;
            }
            if let OperationKind::Call { callee, .. }
            | OperationKind::CallStructuralScalar { callee, .. } = operation.kind
                && let Some(result) = operation.result.scalar_ref()
            {
                calls.insert(result.id, callee);
            }
        }
        if calls.is_empty() {
            continue;
        }
        for site in original.obligations().iter().filter(|site| {
            site.owner.machine() == machine.id && !supplied.contains(&site.obligation.id)
        }) {
            let mut pending = Vec::new();
            if site
                .obligation
                .proposition
                .visit_value_ids_bounded(remaining, |value| pending.push(value))
                .is_none()
            {
                return demanded;
            }
            // Each reconstructed equation stays in this site's exact context.
            // Index it once rather than rescan the whole roster per dependency.
            let mut definitions: BTreeMap<ValueId, Vec<&Proposition>> = BTreeMap::new();
            let mut field_definitions: BTreeMap<&ScalarTerm, Vec<&Proposition>> = BTreeMap::new();
            for equation in &site.semantic_axioms {
                if !charge(remaining, 1) {
                    return demanded;
                }
                if let Proposition::Equal(ScalarTerm::Value { id, .. }, _) = equation {
                    definitions.entry(*id).or_default().push(equation);
                } else if let Proposition::Equal(
                    field @ (ScalarTerm::IntegerField { .. } | ScalarTerm::BooleanField { .. }),
                    _,
                ) = equation
                {
                    field_definitions.entry(field).or_default().push(equation);
                }
            }
            // A stored call result can be demanded by a later field read.
            // Bridge that observation through only this site's surviving store
            // equations. Reconstruction owns mutation invalidation; matching the
            // complete field term preserves root, path and carrier identity.
            // This selects a proof candidate, never establishes a field fact.
            for equation in &site.semantic_axioms {
                if !charge(remaining, 1) {
                    return demanded;
                }
                if let Proposition::Equal(ScalarTerm::Value { id, .. }, observed) = equation
                    && let Some(stores) = field_definitions.get(observed)
                {
                    if !charge(remaining, stores.len()) {
                        return demanded;
                    }
                    definitions.entry(*id).or_default().extend(stores);
                }
            }
            let mut visited = BTreeSet::new();
            while let Some(value) = pending.pop() {
                if !charge(remaining, 1) {
                    return demanded;
                }
                if !visited.insert(value) {
                    continue;
                }
                if let Some(callee) = calls.get(&value) {
                    demanded.insert(*callee);
                    continue;
                }
                for equation in definitions.get(&value).into_iter().flatten() {
                    if equation
                        .visit_value_ids_bounded(remaining, |dependency| {
                            if dependency != value {
                                pending.push(dependency);
                            }
                        })
                        .is_none()
                    {
                        return demanded;
                    }
                }
            }
        }
    }
    demanded
}

fn charge(remaining: &mut usize, count: usize) -> bool {
    match remaining.checked_sub(count) {
        Some(rest) => {
            *remaining = rest;
            true
        }
        None => {
            *remaining = 0;
            false
        }
    }
}

fn result_relation(
    machine: &TerminalMachine,
    context: &PropositionContext,
    remaining: &mut usize,
) -> Option<Proposition> {
    let result = machine.result.scalar_ref()?;
    if !charge(remaining, machine.parameters.len())
        || !charge(remaining, machine.contract.erased_scalar_formals.len())
    {
        return None;
    }
    for block in &machine.blocks {
        if !charge(remaining, 1)
            || !charge(remaining, block.parameters.len())
            || !charge(remaining, block.erased_scalar_formals.len())
            || !charge(remaining, block.operations.len())
        {
            return None;
        }
    }
    let scope = PropositionContext::from_value_types(
        machine
            .parameters
            .iter()
            .chain(std::iter::once(result))
            .map(|value| (value.id, value.scalar_type)),
    )
    .ok()?;
    let value_types: BTreeMap<ValueId, ScalarType> = machine
        .parameters
        .iter()
        .chain(machine.contract.erased_scalar_formals.iter())
        .chain(std::iter::once(result))
        .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.erased_scalar_formals),
        )
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| operation.result.scalar_ref()),
        )
        .map(|value| (value.id, value.scalar_type))
        .collect();
    let mut equations = Vec::new();
    for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
        *remaining = remaining.checked_sub(1)?;
        if let Some(meaning) =
            terminal_semantics::goal_free_scalar_leaf_semantics(operation, &value_types).ok()?
        {
            equations.push(meaning.result_equation().clone());
        } else if let Some(meaning) =
            terminal_semantics::proof_bearing_scalar_leaf_semantics(operation, &value_types).ok()?
        {
            equations.push(meaning.result_equation().clone());
        }
    }
    let mut common = None;
    for block in &machine.blocks {
        let Terminator::Return { value, .. } = block.terminator else {
            continue;
        };
        *remaining = remaining.checked_sub(1)?;
        let relation = Proposition::Equal(
            ScalarTerm::value(result.id, result.scalar_type),
            ScalarTerm::value(value, *value_types.get(&value)?),
        );
        relation.visit_value_ids_bounded(remaining, |_| {})?;
        for equation in &equations {
            equation.visit_value_ids_bounded(remaining, |_| {})?;
        }
        // Do not add the result-to-return equation to the substitution roster:
        // the declared result is a boundary variable, not a body definition.
        let relation = if equations.is_empty() {
            relation
        } else {
            match check_value_equality_denotation(context, &relation, equations.iter()) {
                Ok(relation) => relation,
                Err(PredicateDenotationError::ResourceLimitExceeded) => {
                    *remaining = 0;
                    return None;
                }
                Err(_) => return None,
            }
        };
        // Expansion has the transport kernel's own finite bound. Charge its
        // complete output against the shared pool before retaining or cloning it.
        relation.visit_value_ids_bounded(remaining, |_| {})?;
        scope.validate(&relation).ok()?;
        let Proposition::Equal(left, right) = relation else {
            return None;
        };
        // Generated clauses obey the same wire ordering as authored contracts.
        let relation = super::contract_predicates::canonical_equality(left, right).ok()?;
        if common
            .as_ref()
            .is_some_and(|previous| previous != &relation)
        {
            return None;
        }
        common = Some(relation);
    }
    common
}

fn proves_and_preserves_evidence(
    lowered: &LoweredPsi,
    original: &ReconstructedTerminalObligationSet,
    added: &[ObligationId],
) -> Result<bool, LoweringError> {
    let validated = terminal_verifier::validate_module(&lowered.semantic_module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    let proposed = terminal_verifier::reconstruct_execution_terminal_obligations(validated)
        .map_err(LoweringError::InvalidTerminalModule)?;
    for obligation in added {
        let Some(site) = proposed.obligations().iter().find(|site| {
            site.obligation.id == *obligation
                && matches!(
                    site.owner,
                    ReconstructedTerminalObligationOwner::ContractEnsures { .. }
                )
        }) else {
            return Ok(false);
        };
        let machine = validated
            .machine(site.owner.machine())
            .ok_or(LoweringError::Unsupported(
                "result guarantee lost its proof owner",
            ))?;
        let context = validated
            .value_context(machine)
            .map_err(LoweringError::InvalidTerminalModule)?;
        let parameters = machine.parameters.iter().map(|value| value.id).collect();
        if produce_checked_canonical_integer_proof(
            &context,
            &site.obligation.proposition,
            &site.requirements,
            &site.semantic_axioms,
            &parameters,
        )
        .or_else(|| {
            produce_relaxed_integer_proof(
                &context,
                &site.obligation.proposition,
                &site.requirements,
                &site.semantic_axioms,
                &parameters,
            )
        })
        .is_none()
        {
            return Ok(false);
        }
    }
    super::retained_evidence::replays(validated, original, &proposed, &lowered.proof_bundle)
}
