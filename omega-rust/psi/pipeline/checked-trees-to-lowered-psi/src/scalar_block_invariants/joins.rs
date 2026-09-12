//! Propose authored guarantees at acyclic scalar joins.
//!
//! Reconstruction deliberately intersects ordinary path facts. A lost join
//! equation needs checked arrivals, not unioned branch facts. Work backwards
//! from the unchanged guarantee using the destination scope as a stopping
//! boundary for equation transport. This is optional producer search: every
//! resulting predicate is separately proved on every actual arrival, and the
//! original guarantee remains an independent final obligation. No call body is
//! inspected and no source shape selects a compiler path.

use std::collections::{BTreeMap, BTreeSet};

use proof_admission::check_value_equality_denotation;
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ValueId};
use terminal_psi::{ScalarBlockInvariant, TerminalModule, Terminator};
use terminal_verifier::{ReconstructedTerminalObligationOwner, ReconstructedTerminalObligationSet};

pub(super) fn candidates(
    module: &TerminalModule,
    questions: &ReconstructedTerminalObligationSet,
) -> Vec<ScalarBlockInvariant> {
    let mut candidates = Vec::new();
    let mut remaining = 4096usize;
    let Ok(validated) = terminal_verifier::validate_module(module) else {
        return candidates;
    };
    for machine in &module.machines {
        let Ok(context) = validated.value_context(machine) else {
            continue;
        };
        let Ok(components) = terminal_verifier::control_cycle_members(machine) else {
            continue;
        };
        let mut arrivals = BTreeMap::<_, usize>::new();
        let mut structural_targets = BTreeSet::new();
        for block in &machine.blocks {
            match &block.terminator {
                Terminator::Jump { target, .. } => *arrivals.entry(*target).or_default() += 1,
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for successor in [when_true, when_false] {
                        *arrivals.entry(successor.target).or_default() += 1;
                    }
                }
                Terminator::StructuralCase { cases, .. } => {
                    structural_targets.extend(cases.iter().map(|successor| successor.target));
                }
                _ => {}
            }
        }
        for header in &machine.blocks {
            if header.id == machine.entry
                || arrivals.get(&header.id).copied().unwrap_or(0) < 2
                || structural_targets.contains(&header.id)
                || components
                    .iter()
                    .any(|component| component.contains(&header.id))
            {
                continue;
            }
            let Ok(scope) = PropositionContext::from_value_types(
                machine
                    .parameters
                    .iter()
                    .chain(&header.parameters)
                    .map(|value| (value.id, value.scalar_type)),
            ) else {
                continue;
            };
            let allowed = machine
                .parameters
                .iter()
                .chain(&header.parameters)
                .map(|value| value.id)
                .collect::<BTreeSet<_>>();
            for site in questions.obligations().iter().filter(|site| {
                matches!(site.owner, ReconstructedTerminalObligationOwner::ContractEnsures { machine: owner, .. } if owner == machine.id)
            }) {
                let Some(next) = remaining.checked_sub(1) else { return candidates };
                remaining = next;
                let Some(predicate) = scoped_goal(
                    &site.obligation.proposition, &site.semantic_axioms, &allowed, &scope, &context, &mut remaining,
                ) else { continue };
                // Entry-only tautologies do not justify adding a join roster.
                let mut uses_header = false;
                predicate.visit_value_ids(|value| {
                    uses_header |= header.parameters.iter().any(|parameter| parameter.id == value);
                });
                if uses_header {
                    candidates.push(ScalarBlockInvariant {
                        machine: machine.id, header: header.id, predicate, arrivals: Vec::new(),
                    });
                }
            }
        }
    }
    candidates
}

fn scoped_goal(
    goal: &Proposition,
    axioms: &[Proposition],
    allowed: &BTreeSet<ValueId>,
    scope: &PropositionContext,
    context: &PropositionContext,
    remaining: &mut usize,
) -> Option<Proposition> {
    if axioms.len() > *remaining {
        return None;
    }
    let mut pending_facts = axioms.iter().collect::<Vec<_>>();
    let mut definitions = BTreeMap::new();
    let mut reversed = BTreeMap::new();
    while let Some(fact) = pending_facts.pop() {
        *remaining = remaining.checked_sub(1)?;
        match fact {
            Proposition::Conjunction(members) => pending_facts.extend(members),
            Proposition::Equal(left @ ScalarTerm::Value { id, .. }, right) if left != right => {
                // Reverse traversal with replacement retains the first defining
                // equation, matching ordinary value-equality proof production.
                definitions.insert(*id, fact);
                if let ScalarTerm::Value { id: result, .. } = right {
                    reversed.insert(*result, Proposition::Equal(right.clone(), left.clone()));
                }
            }
            _ => {}
        }
    }
    let mut pending = Vec::new();
    if !goal.visit_value_ids(|value| pending.push(value)) {
        return None;
    }
    let mut visited = BTreeSet::new();
    let mut equations = Vec::new();
    while let Some(value) = pending.pop() {
        *remaining = remaining.checked_sub(1)?;
        if allowed.contains(&value) || !visited.insert(value) {
            continue;
        }
        // Canonical call guarantees can spell argument = result. Prefer an
        // actual defining equation, then orient an alias towards its source.
        // This proposes a predicate only; arrival proofs still cite/check the
        // original equations, including explicit symmetry where needed.
        let definition = definitions
            .get(&value)
            .copied()
            .or_else(|| reversed.get(&value))?;
        if !definition.visit_value_ids(|dependency| {
            if dependency != value {
                pending.push(dependency);
            }
        }) {
            return None;
        }
        equations.push(definition);
    }
    if equations.is_empty() {
        return None;
    }
    // The transport validates all participating values before substitution.
    // A narrow final context then rejects any unresolved body-local identity.
    let predicate = check_value_equality_denotation(context, goal, equations).ok()?;
    scope.validate(&predicate).ok()?;
    Some(predicate)
}
