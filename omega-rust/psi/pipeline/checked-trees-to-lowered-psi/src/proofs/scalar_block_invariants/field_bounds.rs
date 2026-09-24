//! Propose storage-observation bounds at cyclic headers.
//!
//! A relation obligation on a field-read value can need a fact no path fact
//! carries: the field's own state across the iteration cut. Transporting the
//! bound through the read's exact equation produces a candidate invariant on
//! the observed leaf, scoped to places alive for the whole invocation.
//! Retain selected field guards as premises: an exit backedge may invalidate
//! the bound after disabling the path that needs it. These are proposals only
//! — every actual arrival still proves the predicate
//! before the module grants authority, so a non-inductive guess (for example
//! a bound the cycle's last iteration violates before the guard exits) is
//! dropped by that check rather than weakening the question.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{
    BlockId, IntegerValue, PlaceId, Proposition, ScalarTerm, StructuralPlaceKind, ValueId,
};
use terminal_psi::{ScalarBlockInvariant, TerminalMachine, TerminalModule, Terminator};
use terminal_verifier::{ReconstructedTerminalObligationOwner, ReconstructedTerminalObligationSet};

pub(super) fn candidates(
    module: &TerminalModule,
    questions: &ReconstructedTerminalObligationSet,
    remaining: &mut usize,
) -> Vec<ScalarBlockInvariant> {
    let mut candidates = Vec::new();
    let mut proposed = BTreeSet::new();
    for machine in &module.machines {
        let Ok(components) = terminal_verifier::control_cycle_members(machine) else {
            continue;
        };
        if components.is_empty() {
            continue;
        }
        // An invariant may observe only places alive at its header's
        // arrivals: machine parameters and block parameters. Operation
        // results, literals, and other produced places can never qualify, so
        // this cheap pre-filter keeps them out of the equation table; the
        // exact telescope check below still decides per header whether a
        // block parameter belongs to it.
        let invocation_roots = machine
            .structural_places
            .iter()
            .filter(|place| {
                matches!(
                    place.kind,
                    StructuralPlaceKind::Parameter { .. }
                        | StructuralPlaceKind::BlockParameter { .. }
                )
            })
            .map(|place| place.id)
            .collect::<BTreeSet<_>>();
        if invocation_roots.is_empty() {
            continue;
        }
        let predecessors = predecessors(machine);
        for site in questions.obligations().iter().filter(|site| {
            matches!(
                site.owner,
                ReconstructedTerminalObligationOwner::Operation { machine: owner, .. }
                | ReconstructedTerminalObligationOwner::CallRequires { machine: owner, .. }
                    if owner == machine.id
            )
        }) {
            let Some(next) = remaining.checked_sub(1) else {
                return candidates;
            };
            *remaining = next;
            let operation = match site.owner {
                ReconstructedTerminalObligationOwner::Operation { operation, .. }
                | ReconstructedTerminalObligationOwner::CallRequires { operation, .. } => operation,
                _ => unreachable!("filtered above"),
            };
            let Some(block) = machine.blocks.iter().find(|block| {
                block
                    .operations
                    .iter()
                    .any(|candidate| candidate.id == operation)
            }) else {
                continue;
            };
            let Some(component) = components
                .iter()
                .find(|component| component.contains(&block.id))
            else {
                // Acyclic demands are the join synthesizer's scope.
                continue;
            };
            let equations = FieldEquations::new(&site.semantic_axioms, &invocation_roots);
            if equations.fields.is_empty() {
                continue;
            }
            let Some(predicates) = transported(&site.obligation.proposition, &equations) else {
                continue;
            };
            // Transport the site's retained field comparisons once, then
            // select the subset scoped to each header and distinct from the
            // proposed conclusion. These can include derived path bounds;
            // none become trusted simply because they are candidate premises.
            let guards = site
                .semantic_axioms
                .iter()
                .filter(|fact| literal_comparison(fact))
                .filter_map(|fact| transport_relation(fact, &equations))
                .collect::<BTreeSet<_>>();
            // A cyclic header is entered from outside its component. Members
            // reachable only through the cycle itself carry the same facts
            // forward; the externally entered member is where the iteration
            // hypothesis must hold on every actual arrival.
            let externally_entered = |member: &BlockId| {
                predecessors
                    .get(member)
                    .is_some_and(|sources| sources.iter().any(|source| !component.contains(source)))
            };
            // Nested cycles share one component but have their own iteration
            // entries: reconstruction cuts incoming facts at each feedback
            // target, so a bound the inner backedge invalidates can only be
            // inductive at the inner header dominating this operation.
            let Ok(iteration_entries) =
                terminal_verifier::dominating_control_cycle_entries(machine, block.id)
            else {
                continue;
            };
            for header in component.iter().copied().filter(|member| {
                *member != machine.entry
                    && (externally_entered(member) || iteration_entries.contains(member))
            }) {
                let Some(header_block) = machine.blocks.iter().find(|block| block.id == header)
                else {
                    continue;
                };
                let Ok(scope) =
                    terminal_verifier::scalar_block_invariant_scope(machine, header_block)
                else {
                    continue;
                };
                for predicate in &predicates {
                    let Some(next) = remaining.checked_sub(1) else {
                        return candidates;
                    };
                    *remaining = next;
                    // Reconstructed path facts describe when this demand is
                    // reached. Transport their literal comparisons through the
                    // same live read equations, never treating a read's own
                    // defining equation as a guard. The conditional candidate
                    // is weaker than an unconditional bound, but the operation
                    // must still prove its original obligation from it.
                    let guards = guards
                        .iter()
                        .filter(|guard| *guard != predicate && scope.validate(guard).is_ok())
                        .cloned()
                        .collect::<Vec<_>>();
                    let conclusion = without_refuted_alternatives(predicate.clone(), &guards);
                    let predicate = match guards.as_slice() {
                        [] => conclusion,
                        [guard] => Proposition::Implication {
                            premise: Box::new(guard.clone()),
                            conclusion: Box::new(conclusion),
                        },
                        _ => Proposition::Implication {
                            premise: Box::new(Proposition::Conjunction(guards)),
                            conclusion: Box::new(conclusion),
                        },
                    };
                    // The exact invariant telescope decides scope: a predicate
                    // naming a block-local value or a dead place cannot become
                    // an assertion even though its site was reachable.
                    if scope.validate(&predicate).is_err()
                        || !proposed.insert((header, predicate.clone()))
                    {
                        continue;
                    }
                    candidates.push(ScalarBlockInvariant {
                        machine: machine.id,
                        header,
                        predicate,
                        arrivals: Vec::new(),
                    });
                }
            }
        }
    }
    candidates
}

/// A carrier question often states every way an operation can be defined
/// (`d <= -2 || 1 <= d || ...` for a remainder's divisor), while the retained
/// guards already fix which alternatives an arrival can take. Drop the
/// alternatives a guard's literal bound contradicts: under the premise the
/// proposal is unchanged, and the surviving alternative is the atomic bound
/// the operation's own proof consumes. A disjunction the guards refute
/// entirely stays as proposed for the ordinary arrival check to decide.
fn without_refuted_alternatives(predicate: Proposition, guards: &[Proposition]) -> Proposition {
    let Proposition::Disjunction(alternatives) = predicate else {
        return predicate;
    };
    let mut surviving = alternatives
        .iter()
        .filter(|alternative| !refuted(alternative, guards))
        .cloned()
        .collect::<Vec<_>>();
    match surviving.len() {
        0 => Proposition::Disjunction(alternatives),
        1 => surviving.pop().expect("one surviving alternative"),
        _ => Proposition::Disjunction(surviving),
    }
}

fn refuted(alternative: &Proposition, guards: &[Proposition]) -> bool {
    if let Proposition::Conjunction(members) = alternative {
        return members.iter().any(|member| refuted(member, guards));
    }
    let Some((field, bound)) = literal_bound(alternative) else {
        return false;
    };
    guards
        .iter()
        .filter_map(literal_bound)
        .any(|(guarded, guard)| {
            guarded == field
                && match (bound, guard) {
                    (LiteralBound::AtMost(upper), LiteralBound::AtLeast(lower))
                    | (LiteralBound::AtLeast(lower), LiteralBound::AtMost(upper)) => upper < lower,
                    _ => false,
                }
        })
}

#[derive(Clone, Copy)]
enum LiteralBound {
    AtLeast(i128),
    AtMost(i128),
}

/// An observed field compared with a literal, as one inclusive bound.
fn literal_bound(relation: &Proposition) -> Option<(&ScalarTerm, LiteralBound)> {
    let (left, right, strict) = match relation {
        Proposition::LessOrEqual(left, right) => (left, right, false),
        Proposition::LessThan(left, right) => (left, right, true),
        _ => return None,
    };
    let literal = |term: &ScalarTerm| match term.integer_value()?.1 {
        IntegerValue::Signed(value) => Some(value),
        IntegerValue::Unsigned(value) => i128::try_from(value).ok(),
    };
    let step = i128::from(strict);
    match (left, right) {
        (field @ ScalarTerm::IntegerField { .. }, bound) => Some((
            field,
            LiteralBound::AtMost(literal(bound)?.checked_sub(step)?),
        )),
        (bound, field @ ScalarTerm::IntegerField { .. }) => Some((
            field,
            LiteralBound::AtLeast(literal(bound)?.checked_add(step)?),
        )),
        _ => None,
    }
}

fn literal_comparison(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => [left, right]
            .iter()
            .any(|term| matches!(term, ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. })),
        _ => false,
    }
}

/// Transport one goal's value endpoints through the arrival's exact field-read
/// equations. Each member of a conjunction is an independent claim; any other
/// connective transports whole, because a weaker remainder would change the
/// proposed question.
fn transported(goal: &Proposition, equations: &FieldEquations<'_>) -> Option<Vec<Proposition>> {
    match goal {
        Proposition::Conjunction(members) => Some(
            members
                .iter()
                .filter_map(|member| transport_claim(member, equations))
                .collect(),
        ),
        _ => transport_claim(goal, equations).map(|predicate| vec![predicate]),
    }
}

/// A disjunction transports only when every alternative names observed
/// storage, and a conjunction inside one only when every member does.
fn transport_claim(claim: &Proposition, equations: &FieldEquations<'_>) -> Option<Proposition> {
    match claim {
        Proposition::Conjunction(members) => members
            .iter()
            .map(|member| transport_claim(member, equations))
            .collect::<Option<Vec<_>>>()
            .map(Proposition::Conjunction),
        Proposition::Disjunction(members) => members
            .iter()
            .map(|member| transport_claim(member, equations))
            .collect::<Option<Vec<_>>>()
            .map(Proposition::Disjunction),
        _ => transport_relation(claim, equations),
    }
}

fn transport_relation(
    proposition: &Proposition,
    equations: &FieldEquations<'_>,
) -> Option<Proposition> {
    let mut observed = false;
    let mut endpoint = |term: &ScalarTerm| -> Option<ScalarTerm> {
        Some(match term {
            ScalarTerm::Value { id, .. } => {
                let field = equations.field(*id)?;
                observed = true;
                field.clone()
            }
            ScalarTerm::BooleanField { .. } | ScalarTerm::IntegerField { .. } => {
                observed = true;
                term.clone()
            }
            // Literals and existing field observations are already in scope;
            // any other term is retained for the telescope check to reject.
            _ => term.clone(),
        })
    };
    // Operation questions state carrier bounds in mathematical integers; one
    // value against literals is the same typed relation over its carrier.
    let scalar = crate::proofs::contract_predicates::scalar_relation_for_math(proposition);
    let proposition = match scalar.as_ref().unwrap_or(proposition) {
        Proposition::Equal(left, right) => Proposition::Equal(endpoint(left)?, endpoint(right)?),
        Proposition::LessThan(left, right) => {
            Proposition::LessThan(endpoint(left)?, endpoint(right)?)
        }
        Proposition::LessOrEqual(left, right) => {
            Proposition::LessOrEqual(endpoint(left)?, endpoint(right)?)
        }
        _ => return None,
    };
    // A relation that names no observed storage is the join/range
    // synthesizers' question, not this one's.
    observed.then_some(proposition)
}

/// Exact `value = field` equations published by the arrival's reads, rooted at
/// invocation-lived places. A conjunction member is a fact like any other;
/// disjunction branches never become unconditional equations.
struct FieldEquations<'input> {
    fields: BTreeMap<ValueId, &'input ScalarTerm>,
}

impl<'input> FieldEquations<'input> {
    fn new(axioms: &'input [Proposition], invocation_roots: &BTreeSet<PlaceId>) -> Self {
        let mut fields = BTreeMap::new();
        let mut pending = axioms.iter().collect::<Vec<_>>();
        while let Some(fact) = pending.pop() {
            match fact {
                Proposition::Conjunction(members) => pending.extend(members),
                Proposition::Equal(left, right) => {
                    for (value, field) in [(left, right), (right, left)] {
                        if let (
                            ScalarTerm::Value { id, .. },
                            ScalarTerm::BooleanField { root, .. }
                            | ScalarTerm::IntegerField { root, .. },
                        ) = (value, field)
                            && invocation_roots.contains(root)
                        {
                            fields.entry(*id).or_insert(field);
                        }
                    }
                }
                _ => {}
            }
        }
        Self { fields }
    }

    fn field(&self, id: ValueId) -> Option<&'input ScalarTerm> {
        self.fields.get(&id).copied()
    }
}

fn predecessors(machine: &TerminalMachine) -> BTreeMap<BlockId, Vec<BlockId>> {
    let mut predecessors = BTreeMap::<BlockId, Vec<BlockId>>::new();
    let mut push = |source: BlockId, target: BlockId| {
        predecessors.entry(target).or_default().push(source);
    };
    for block in &machine.blocks {
        match &block.terminator {
            Terminator::Jump { target, .. } => push(block.id, *target),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                push(block.id, when_true.target);
                push(block.id, when_false.target);
            }
            Terminator::StructuralCase { cases, .. } => {
                for successor in cases {
                    push(block.id, successor.target);
                }
            }
            _ => {}
        }
    }
    predecessors
}
