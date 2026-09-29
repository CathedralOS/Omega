//! Propose storage-observation bounds at control-flow joins.
//!
//! A relation obligation on a field-read value can need a fact no path fact
//! carries: the field's own state across a merge or iteration cut. Transporting the
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
    BlockId, IntegerValue, PlaceId, Proposition, PropositionContext, ScalarTerm,
    StructuralPlaceKind, ValueId,
};
use terminal_psi::{ScalarBlockInvariant, TerminalMachine, TerminalModule, Terminator};
use terminal_verifier::{ReconstructedTerminalObligationOwner, ReconstructedTerminalObligationSet};

pub(super) fn candidates(
    module: &TerminalModule,
    questions: &ReconstructedTerminalObligationSet,
    remaining: &mut usize,
    declines: &mut crate::proofs::ProofSearchDeclines,
) -> Vec<ScalarBlockInvariant> {
    let mut candidates = Vec::new();
    for machine in &module.machines {
        let mut proposed = BTreeSet::new();
        let Ok(components) = terminal_verifier::control_cycle_members(machine) else {
            continue;
        };
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
        let header_blocks = machine
            .blocks
            .iter()
            .map(|block| (block.id, block))
            .collect::<BTreeMap<_, _>>();
        // Reverse reachability from a demanded block is machine-level graph
        // structure: obligations on the same block walk the same ancestors.
        // One filtered roster per demand block shares that walk across sites.
        let mut join_headers: BTreeMap<BlockId, Vec<BlockId>> = BTreeMap::new();
        // A header's live-value scope depends only on the machine and header;
        // the same header answers many sites and predicates.
        let mut header_scopes: BTreeMap<BlockId, Option<PropositionContext>> =
            BTreeMap::new();
        // The owning block of a demanded operation never changes within the
        // machine; index it once rather than scanning the block roster per site.
        let operation_blocks = machine
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .operations
                    .iter()
                    .map(move |operation| (operation.id, block))
            })
            .collect::<BTreeMap<_, _>>();
        candidates.extend(arrival_storage_candidates(
            module,
            machine,
            questions,
            &components,
            remaining,
            declines,
        ));
        for site in questions.obligations().iter().filter(|site| {
            matches!(
                site.owner,
                ReconstructedTerminalObligationOwner::Operation { machine: owner, .. }
                | ReconstructedTerminalObligationOwner::CallRequires { machine: owner, .. }
                    if owner == machine.id
            )
        }) {
            let Some(next) = remaining.checked_sub(1) else {
                declines.record(
                    crate::lowering_error::ProofSearchCapacity::ScalarInvariantCandidateWork,
                );
                return candidates;
            };
            *remaining = next;
            let operation = match site.owner {
                ReconstructedTerminalObligationOwner::Operation { operation, .. }
                | ReconstructedTerminalObligationOwner::CallRequires { operation, .. } => operation,
                _ => unreachable!("filtered above"),
            };
            let Some(block) = operation_blocks.get(&operation).copied() else {
                continue;
            };
            let equations = FieldEquations::new(&site.semantic_axioms, &invocation_roots);
            if equations.fields.is_empty() {
                continue;
            }
            let Some(predicates) = transported(&site.obligation.proposition, &equations) else {
                continue;
            };
            let predicates = predicates
                .into_iter()
                .map(inclusive_bound)
                .collect::<Vec<_>>();
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
            // Distinct arrivals may observe the same field through different
            // SSA values. Their bounds disappear at ordinary fact intersection
            // even when every arrival proves the same storage predicate. Loop
            // headers and ordinary joins need the same all-arrival transaction.
            // Walk backwards from this demand so unrelated joins receive no
            // proposal. A candidate is not authority: intervening writes and
            // unsupported arrivals still make its ordinary proof fail.
            let mut exhausted = false;
            let ancestors = join_headers.entry(block.id).or_insert_with(|| {
                let mut ancestors = BTreeSet::new();
                let mut pending = vec![block.id];
                while let Some(predecessor) = pending.pop() {
                    let Some(next) = remaining.checked_sub(1) else {
                        exhausted = true;
                        return Vec::new();
                    };
                    *remaining = next;
                    if ancestors.insert(predecessor)
                        && let Some(sources) = predecessors.get(&predecessor)
                    {
                        pending.extend(sources.iter().copied());
                    }
                }
                ancestors
                    .into_iter()
                    .filter(|member| {
                        *member != machine.entry
                            && predecessors
                                .get(member)
                                .is_some_and(|sources| sources.len() > 1)
                    })
                    .collect()
            });
            if exhausted {
                declines.record(
                    crate::lowering_error::ProofSearchCapacity::ScalarInvariantCandidateWork,
                );
                return candidates;
            }
            for header in ancestors.iter().copied() {
                let Some(header_block) = header_blocks.get(&header).copied() else {
                    continue;
                };
                let scope = match header_scopes.entry(header) {
                    std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                    std::collections::btree_map::Entry::Vacant(entry) => entry.insert(
                        terminal_verifier::scalar_block_invariant_scope(machine, header_block)
                            .ok(),
                    ),
                };
                let Some(scope) = scope.as_ref() else {
                    continue;
                };
                // Guard validity at this header is predicate-independent:
                // validate each transported guard once per (site, header),
                // then drop the guard naming each predicate by comparison.
                let scoped_guards = guards
                    .iter()
                    .filter(|guard| scope.validate(guard).is_ok())
                    .cloned()
                    .collect::<Vec<_>>();
                for predicate in &predicates {
                    // A bound the site's own path guard already states needs
                    // no header invariant; proposing it only adds a question.
                    if guards.contains(predicate) {
                        continue;
                    }
                    // Reconstructed path facts describe when this demand is
                    // reached. Transport their literal comparisons through the
                    // same live read equations, never treating a read's own
                    // defining equation as a guard. The conditional candidate
                    // is weaker than an unconditional bound, but the operation
                    // must still prove its original obligation from it.
                    let guards = scoped_guards
                        .iter()
                        .filter(|guard| *guard != predicate)
                        .cloned()
                        .collect::<Vec<_>>();
                    let conclusion = without_refuted_alternatives(predicate.clone(), &guards);
                    let guarded = match guards.as_slice() {
                        [] => None,
                        [guard] => Some(Proposition::Implication {
                            premise: Box::new(guard.clone()),
                            conclusion: Box::new(conclusion),
                        }),
                        _ => Some(Proposition::Implication {
                            premise: Box::new(Proposition::Conjunction(guards)),
                            conclusion: Box::new(conclusion),
                        }),
                    };
                    // Try the unguarded demand too. Both arrivals may establish
                    // a byte bound independently of a later index guard; tying
                    // that bound only to the index loses it when the index is
                    // assigned after the join. All-arrival checking can drop
                    // this stronger claim while retaining a guarded loop fact.
                    for predicate in std::iter::once(predicate.clone()).chain(guarded) {
                        // The exact invariant telescope rejects block-local
                        // values and dead places regardless of reachability.
                        if scope.validate(&predicate).is_err()
                            || !proposed.insert((header, predicate.clone()))
                        {
                            continue;
                        }
                        let Some(next) = remaining.checked_sub(1) else {
                            declines.record(crate::lowering_error::ProofSearchCapacity::ScalarInvariantCandidateWork);
                            return candidates;
                        };
                        *remaining = next;
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
    }
    candidates
}

/// A demanded header can need a bound established before an inner loop even
/// when that entry is internal to the outer loop's strongly connected component.
/// Seed its arrivals' atomic storage comparisons at that same header, without
/// broadcasting them across the component. The transaction proves establishment and
/// preservation; a branch-local bound or a bound invalidated by a write drops.
pub(super) fn arrival_bounds(
    module: &TerminalModule,
    questions: &ReconstructedTerminalObligationSet,
    remaining: &mut usize,
    declines: &mut crate::proofs::ProofSearchDeclines,
    // A site's seeded bounds are a pure function of its reconstructed
    // obligation and the roster it names; the discovery loop re-queries the
    // module every round while only newly minted invariant questions can add
    // candidates. The caller shares this set so each obligation is queried
    // at most once across every round.
    queried: &mut BTreeSet<semantic_vocabulary::ObligationId>,
) -> Vec<ScalarBlockInvariant> {
    let mut candidates = Vec::new();
    let mut proposed = BTreeSet::new();
    for site in questions.obligations() {
        let ReconstructedTerminalObligationOwner::ScalarBlockInvariant {
            machine: owner,
            header,
            ..
        } = site.owner
        else {
            continue;
        };
        if !queried.insert(site.obligation.id) {
            continue;
        }
        let Some(machine) = module.machines.iter().find(|machine| machine.id == owner) else {
            continue;
        };
        let Some(block) = machine.blocks.iter().find(|block| block.id == header) else {
            continue;
        };
        let Ok(scope) = terminal_verifier::scalar_block_invariant_scope(machine, block) else {
            continue;
        };
        let roots = machine
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
            .collect();
        let equations = FieldEquations::new(&site.semantic_axioms, &roots);
        let mut pending = site.semantic_axioms.iter().collect::<Vec<_>>();
        while let Some(fact) = pending.pop() {
            // Only conjunction leaves are unconditional. Never promote an
            // implication's conclusion or one alternative of a disjunction.
            if let Proposition::Conjunction(members) = fact {
                pending.extend(members);
                continue;
            }
            if !matches!(
                fact,
                Proposition::LessThan(..) | Proposition::LessOrEqual(..)
            ) || !literal_comparison(fact)
            {
                continue;
            }
            let Some(predicate) = transport_relation(fact, &equations).map(inclusive_bound) else {
                continue;
            };
            if scope.validate(&predicate).is_err()
                || !proposed.insert((owner, header, predicate.clone()))
            {
                continue;
            }
            if module.scalar_block_invariants.iter().any(|existing| {
                existing.machine == owner
                    && existing.header == header
                    && (existing.predicate == predicate
                        || matches!(
                            &existing.predicate,
                            Proposition::Conjunction(members) if members.contains(&predicate)
                        ))
            }) {
                continue;
            }
            // The work unit is a new scoped candidate. Do not spend the shared
            // pool rediscovering an already retained bound on every arrival.
            let Some(next) = remaining.checked_sub(1) else {
                declines.record(
                    crate::lowering_error::ProofSearchCapacity::ScalarInvariantCandidateWork,
                );
                return candidates;
            };
            *remaining = next;
            candidates.push(ScalarBlockInvariant {
                machine: owner,
                header,
                predicate,
                arrivals: Vec::new(),
            });
        }
    }
    candidates
}

/// A loop's authored contracts need not repeat initialized storage that stays
/// fixed through the loop. Propose the literal field equations observed on an
/// external arrival at every authored header in that component. These are not
/// declaration assumptions: the shared retention transaction proves each real
/// arrival, and drops a proposal when a write or unknown call invalidates it.
fn arrival_storage_candidates(
    module: &TerminalModule,
    machine: &TerminalMachine,
    questions: &ReconstructedTerminalObligationSet,
    components: &[Vec<BlockId>],
    remaining: &mut usize,
    declines: &mut crate::proofs::ProofSearchDeclines,
) -> Vec<ScalarBlockInvariant> {
    if !module
        .scalar_block_invariants
        .iter()
        .any(|invariant| invariant.machine == machine.id)
    {
        return Vec::new();
    }
    let Ok(validated) = terminal_verifier::validate_module(module) else {
        return Vec::new();
    };
    let Ok(context) = validated.value_context(machine) else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    let mut proposed = BTreeSet::new();
    for site in questions.obligations() {
        let ReconstructedTerminalObligationOwner::ScalarBlockInvariant {
            machine: owner,
            header,
            edge,
        } = site.owner
        else {
            continue;
        };
        if owner != machine.id {
            continue;
        }
        let Some(component) = components
            .iter()
            .find(|component| component.contains(&header))
        else {
            continue;
        };
        let external_arrival = machine.blocks.iter().any(|block| {
            !component.contains(&block.id)
                && match &block.terminator {
                    Terminator::Jump { edge: arrival, .. } => *arrival == edge,
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => when_true.edge == edge || when_false.edge == edge,
                    _ => false,
                }
        });
        if !external_arrival {
            continue;
        }
        for fact in &site.semantic_axioms {
            let Some(next) = remaining.checked_sub(1) else {
                declines.record(
                    crate::lowering_error::ProofSearchCapacity::ScalarInvariantCandidateWork,
                );
                return candidates;
            };
            *remaining = next;
            let Proposition::Equal(left, right) = fact else {
                continue;
            };
            if !matches!(left, ScalarTerm::IntegerField { .. })
                && !matches!(right, ScalarTerm::IntegerField { .. })
            {
                continue;
            }
            let Ok(predicate) = proof_admission::check_value_equality_denotation(
                &context,
                fact,
                site.semantic_axioms
                    .iter()
                    .filter(|fact| matches!(fact, Proposition::Equal(ScalarTerm::Value { .. }, _))),
            ) else {
                continue;
            };
            let Proposition::Equal(left, right) = &predicate else {
                continue;
            };
            if !matches!(
                (left, right),
                (ScalarTerm::IntegerField { .. }, ScalarTerm::Integer { .. })
                    | (ScalarTerm::Integer { .. }, ScalarTerm::IntegerField { .. })
            ) {
                continue;
            }
            for invariant in module.scalar_block_invariants.iter().filter(|invariant| {
                invariant.machine == machine.id && component.contains(&invariant.header)
            }) {
                let Some(next) = remaining.checked_sub(1) else {
                    declines.record(
                        crate::lowering_error::ProofSearchCapacity::ScalarInvariantCandidateWork,
                    );
                    return candidates;
                };
                *remaining = next;
                let Some(block) = machine
                    .blocks
                    .iter()
                    .find(|block| block.id == invariant.header)
                else {
                    continue;
                };
                let Ok(scope) = terminal_verifier::scalar_block_invariant_scope(machine, block)
                else {
                    continue;
                };
                if scope.validate(&predicate).is_err()
                    || !proposed.insert((invariant.header, predicate.clone()))
                {
                    continue;
                }
                candidates.push(ScalarBlockInvariant {
                    machine: machine.id,
                    header: invariant.header,
                    predicate: predicate.clone(),
                    arrivals: Vec::new(),
                });
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
                let field = equations.field(*id).or_else(|| equations.cast_field(*id))?;
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
    let (left, right) = match scalar.as_ref().unwrap_or(proposition) {
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => (endpoint(left)?, endpoint(right)?),
        _ => return None,
    };
    // An exact cast keeps its operand's mathematical value, so a read reached
    // through one compares against the same literal in the read's carrier.
    let (left, right) = same_carrier(left, right)?;
    let proposition = match scalar.as_ref().unwrap_or(proposition) {
        Proposition::Equal(..) => Proposition::Equal(left, right),
        Proposition::LessThan(..) => Proposition::LessThan(left, right),
        _ => Proposition::LessOrEqual(left, right),
    };
    // A relation that names no observed storage is the join/range
    // synthesizers' question, not this one's.
    observed.then_some(proposition)
}

/// State a strict literal bound as its inclusive integer equivalent
/// (`x < 5` as `x <= 4`). The kernel's discreteness rule strengthens an
/// inclusive bound to the strict one an operation asks for, but has no
/// converse, so the inclusive form is the one every consumer can use.
fn inclusive_bound(predicate: Proposition) -> Proposition {
    let adjacent = |literal: &ScalarTerm, step: i128| {
        let (integer_type, value) = literal.integer_value()?;
        let value = match value {
            IntegerValue::Signed(value) => IntegerValue::Signed(value.checked_add(step)?),
            IntegerValue::Unsigned(value) => IntegerValue::Unsigned(
                u128::try_from(i128::try_from(value).ok()?.checked_add(step)?).ok()?,
            ),
        };
        ScalarTerm::integer(integer_type, value).ok()
    };
    match &predicate {
        Proposition::LessThan(left, right @ ScalarTerm::Integer { .. }) => {
            adjacent(right, -1).map(|bound| Proposition::LessOrEqual(left.clone(), bound))
        }
        Proposition::LessThan(left @ ScalarTerm::Integer { .. }, right) => {
            adjacent(left, 1).map(|bound| Proposition::LessOrEqual(bound, right.clone()))
        }
        _ => None,
    }
    .unwrap_or(predicate)
}

/// Restate a transported relation in one carrier: a literal compared with an
/// observation reached through an exact cast takes the observation's carrier
/// when it has the same value there, and otherwise the relation stays out.
fn same_carrier(left: ScalarTerm, right: ScalarTerm) -> Option<(ScalarTerm, ScalarTerm)> {
    if left.scalar_type() == right.scalar_type() {
        return Some((left, right));
    }
    let retyped = |literal: &ScalarTerm, carrier: &ScalarTerm| {
        let semantic_vocabulary::ScalarType::Integer(carrier) = carrier.scalar_type() else {
            return None;
        };
        let (source, value) = literal.integer_value()?;
        let value = source.exact_cast_value_to(carrier, value)?;
        ScalarTerm::integer(carrier, value).ok()
    };
    match (&left, &right) {
        (ScalarTerm::Integer { .. }, _) => Some((retyped(&left, &right)?, right)),
        (_, ScalarTerm::Integer { .. }) => {
            let right = retyped(&right, &left)?;
            Some((left, right))
        }
        _ => None,
    }
}

/// Exact `value = field` equations published by the arrival's reads, rooted at
/// invocation-lived places, and `value = cast(read)` equations whose exact
/// cast or widening keeps that read's value. A conjunction member is a fact
/// like any other; disjunction branches never become unconditional equations.
struct FieldEquations<'input> {
    fields: BTreeMap<ValueId, &'input ScalarTerm>,
    casts: BTreeMap<ValueId, ValueId>,
}

impl<'input> FieldEquations<'input> {
    fn new(axioms: &'input [Proposition], invocation_roots: &BTreeSet<PlaceId>) -> Self {
        let mut fields = BTreeMap::new();
        let mut casts = BTreeMap::new();
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
                        if let (
                            ScalarTerm::Value { id, .. },
                            ScalarTerm::IntegerExactCast { operand, .. }
                            | ScalarTerm::IntegerWiden { operand, .. },
                        ) = (value, field)
                            && let ScalarTerm::Value { id: operand, .. } = operand.as_ref()
                        {
                            casts.entry(*id).or_insert(*operand);
                        }
                    }
                }
                _ => {}
            }
        }
        Self { fields, casts }
    }

    fn field(&self, id: ValueId) -> Option<&'input ScalarTerm> {
        self.fields.get(&id).copied()
    }

    /// The read an exact cast of `id` keeps the value of.
    fn cast_field(&self, id: ValueId) -> Option<&'input ScalarTerm> {
        self.field(*self.casts.get(&id)?)
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
