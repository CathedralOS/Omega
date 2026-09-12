//! Propose authored guarantees at acyclic scalar joins.
//!
//! Reconstruction deliberately intersects ordinary path facts. A lost join
//! equation needs checked arrivals, not unioned branch facts. Work backwards
//! from the unchanged guarantee using the destination scope as a stopping
//! boundary for equation transport. This is optional producer search: every
//! resulting predicate is separately proved on every actual arrival, and the
//! original guarantee remains an independent final obligation. No call body is
//! inspected and no source shape selects a compiler path.
//!
//! Downstream arrivals supply conditional demands for upstream joins. Preserve
//! their established Boolean polarities through call/copy equations; ordinary
//! aliases are not path guards. Never wrap a demand already covered by a header
//! conjunct, or repeatedly add guards around its own imported assertion. Values
//! absent from a header's scope must resolve through established arrival facts;
//! a later block cannot supply its not-yet-defined parameters to an earlier edge.

use std::collections::{BTreeMap, BTreeSet};

use proof_admission::{check_predicate_denotations, check_value_equality_denotation};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ValueId};
use terminal_psi::{ScalarBlockInvariant, TerminalModule, Terminator};
use terminal_verifier::{ReconstructedTerminalObligationOwner, ReconstructedTerminalObligationSet};

pub(super) fn candidates(
    module: &TerminalModule,
    questions: &ReconstructedTerminalObligationSet,
    remaining: &mut usize,
) -> Vec<ScalarBlockInvariant> {
    let mut candidates = Vec::new();
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
            let mut imported = module
                .scalar_block_invariants
                .iter()
                .filter(|invariant| {
                    invariant.machine == machine.id && invariant.header == header.id
                })
                .map(|invariant| &invariant.predicate)
                .collect::<Vec<_>>();
            let mut cursor = 0;
            while cursor < imported.len() {
                let Some(next) = remaining.checked_sub(1) else {
                    return candidates;
                };
                *remaining = next;
                if let Proposition::Conjunction(members) = imported[cursor] {
                    if members.len() > *remaining {
                        return candidates;
                    }
                    imported.extend(members);
                }
                cursor += 1;
            }
            for site in questions
                .obligations()
                .iter()
                .filter(|site| match site.owner {
                    ReconstructedTerminalObligationOwner::ContractEnsures {
                        machine: owner,
                        ..
                    } => owner == machine.id,
                    ReconstructedTerminalObligationOwner::ScalarBlockInvariant {
                        machine: owner,
                        header: destination,
                        ..
                    } => owner == machine.id && destination != header.id,
                    _ => false,
                })
            {
                let Some(next) = remaining.checked_sub(1) else {
                    return candidates;
                };
                *remaining = next;
                let Some(equations) = ArrivalEquations::new(&site.semantic_axioms, remaining)
                else {
                    continue;
                };
                let Some(mut predicate) = scoped_goal(
                    &site.obligation.proposition,
                    &equations,
                    &allowed,
                    &scope,
                    &context,
                    None,
                    remaining,
                ) else {
                    continue;
                };
                let Ok(canonical) = super::canonical_predicate(predicate) else {
                    continue;
                };
                predicate = canonical;
                if imported.contains(&&predicate) {
                    continue;
                }
                if matches!(
                    site.owner,
                    ReconstructedTerminalObligationOwner::ScalarBlockInvariant { .. }
                ) {
                    let mut guards = site
                        .semantic_axioms
                        .iter()
                        // Retain established Boolean polarities for this path.
                        // Copy equations and imported assertions describe values,
                        // not whether the downstream arrival is taken.
                        .filter(|fact| {
                            matches!(fact, Proposition::Equal(_, ScalarTerm::Boolean(_)))
                        })
                        // A call or copy between joins can carry the selected
                        // condition. Transport guards through the same exact
                        // arrival equations as the goal, stopping at this scope.
                        .filter_map(|fact| {
                            scoped_goal(
                                fact,
                                &equations,
                                &allowed,
                                &scope,
                                &context,
                                Some(fact),
                                remaining,
                            )
                        })
                        .filter_map(|fact| super::canonical_predicate(fact).ok())
                        .filter(|fact| !imported.contains(&fact) && uses_header(fact, header))
                        .collect::<Vec<_>>();
                    if !guards.is_empty() {
                        // The downstream arrival is conditional. Demand its goal
                        // only under the already-established facts expressible at
                        // this earlier join, not unconditionally on every path.
                        let guarded = Proposition::Implication {
                            premise: Box::new(if guards.len() == 1 {
                                guards.remove(0)
                            } else {
                                Proposition::Conjunction(guards)
                            }),
                            conclusion: Box::new(predicate),
                        };
                        let Ok(normalized) =
                            check_predicate_denotations(&scope, &guarded, &[], &[])
                        else {
                            continue;
                        };
                        predicate = normalized.goal().clone();
                    }
                }
                let Ok(predicate) = super::canonical_predicate(predicate) else {
                    continue;
                };
                // Entry-only tautologies do not justify adding a join roster.
                if uses_header(&predicate, header) && !imported.contains(&&predicate) {
                    candidates.push(ScalarBlockInvariant {
                        machine: machine.id,
                        header: header.id,
                        predicate,
                        arrivals: Vec::new(),
                    });
                }
            }
        }
    }
    candidates
}

fn uses_header(predicate: &Proposition, header: &terminal_psi::Block) -> bool {
    let mut uses_header = false;
    predicate.visit_value_ids(|value| {
        uses_header |= header
            .parameters
            .iter()
            .any(|parameter| parameter.id == value);
    });
    uses_header
}

fn scoped_goal(
    goal: &Proposition,
    arrival: &ArrivalEquations<'_>,
    allowed: &BTreeSet<ValueId>,
    scope: &PropositionContext,
    context: &PropositionContext,
    excluded: Option<&Proposition>,
    remaining: &mut usize,
) -> Option<Proposition> {
    *remaining = remaining.checked_sub(1)?;
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
        let definition = arrival
            .definitions
            .get(&value)
            .copied()
            // A guard's selected truth is not its defining computation. Using
            // that very fact as a definition would turn every transported guard
            // into Truth instead of carrying its call/copy alias to the header.
            .filter(|definition| Some(*definition) != excluded)
            .or_else(|| arrival.reversed.get(&value))?;
        if !definition.visit_value_ids(|dependency| {
            if dependency != value {
                pending.push(dependency);
            }
        }) {
            return None;
        }
        equations.push(definition);
    }
    // The transport validates all participating values before substitution.
    // A narrow final context then rejects any unresolved body-local identity.
    // An arrival already in scope needs no equations, but still passes through
    // the same bounded denotation/normalization rather than an unchecked clone.
    let predicate = if equations.is_empty() {
        check_predicate_denotations(context, goal, &[], &[])
            .ok()?
            .goal()
            .clone()
    } else {
        check_value_equality_denotation(context, goal, equations).ok()?
    };
    scope.validate(&predicate).ok()?;
    Some(predicate)
}

/// The goal and its path guards share one arrival's defining equations. Build
/// their index once; neither conditional implications nor alternative branches
/// become unconditional definitions.
struct ArrivalEquations<'input> {
    definitions: BTreeMap<ValueId, &'input Proposition>,
    reversed: BTreeMap<ValueId, Proposition>,
}

impl<'input> ArrivalEquations<'input> {
    fn new(axioms: &'input [Proposition], remaining: &mut usize) -> Option<Self> {
        if axioms.len() > *remaining {
            return None;
        }
        let mut pending = axioms.iter().collect::<Vec<_>>();
        let mut definitions = BTreeMap::new();
        let mut reversed = BTreeMap::new();
        while let Some(fact) = pending.pop() {
            *remaining = remaining.checked_sub(1)?;
            match fact {
                Proposition::Conjunction(members) => {
                    if members.len() > *remaining {
                        return None;
                    }
                    pending.extend(members);
                }
                Proposition::Equal(left @ ScalarTerm::Value { id, .. }, right) if left != right => {
                    // Reverse traversal with replacement retains the first
                    // defining equation, matching value-equality proof production.
                    definitions.insert(*id, fact);
                    if let ScalarTerm::Value { id: result, .. } = right {
                        reversed.insert(*result, Proposition::Equal(right.clone(), left.clone()));
                    }
                }
                _ => {}
            }
        }
        Some(Self {
            definitions,
            reversed,
        })
    }
}
