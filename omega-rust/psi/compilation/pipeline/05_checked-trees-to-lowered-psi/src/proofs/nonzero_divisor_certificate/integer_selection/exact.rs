//! Exact retained proposition proof custody.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use proof_admission::{
    PrimitiveJudgment, ProofNode, ProofRule, check_value_equality_denotation, decide_primitive,
};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ValueId};

use super::super::integer_evidence::cited_facts;
use super::super::{scope_identity, search_context};

mod order_equalities;

/// Distinct scalar endpoints offered pairwise to the kernel's licensed
/// closed/open integer derivation once the cited chain cannot reach a goal.
/// Transport edges extend the roster by at most one expansion per endpoint,
/// so the bound covers both the cited endpoints and their expansions. The
/// bound keeps the normalization search a producer-side convenience over
/// a small endpoint roster, never an unbounded hunt.
const MAXIMUM_NORMALIZATION_ENDPOINTS: usize = 64;

/// Candidate endpoint pairs the kernel is asked to decide per failed cited
/// chain — a second refusal bound so a dense roster of compounds cannot turn
/// one missing edge into an unbounded search.
const MAXIMUM_NORMALIZATION_PAIRS: usize = 96;

/// Bound discovery before the denotation checker bounds expanded terms.
/// Shared definition DAGs are visited once; exhaustion declines transport.
const MAXIMUM_DEFINITION_WORK: usize = 4096;

/// Indexed rosters kept per worker thread. The bound/order producers re-enter
/// `prove` hundreds of times against one unchanged cited roster within a
/// single obligation; indexing that roster once — citation map, equality
/// adjacency, definition tables — keeps each re-entry near the cost of the
/// goal lookup itself instead of rescanning every cited fact per entry.
const MAXIMUM_ROSTERS: usize = 8;

/// A memo is a convenience, never a completeness gate: past each bound the
/// producer simply recomputes the answer it would have cached.
const MAXIMUM_ROSTER_RESULTS: usize = 8192;
const MAXIMUM_MEMOIZED: usize = 65536;

thread_local! {
    /// Indexed cited-fact rosters this worker is proving under. `prove` is a
    /// pure function of its `(context, goal, assumptions, semantic_axioms)`
    /// inputs and every roster is matched by full content — the context
    /// through its content token — never by position or pointer, so a stale
    /// entry can at worst be rebuilt: the produced proof still passes the
    /// kernel unchanged.
    static ROSTERS: RefCell<Vec<Rc<Roster>>> = const { RefCell::new(Vec::new()) };

    /// Kernel-computed endpoint expansions, one map per context token, keyed
    /// by the exact denotation input `(endpoint, equations)` — a memo of the
    /// deterministic `check_value_equality_denotation` call, nothing more. A
    /// hit replays nothing: the emitted `ValueEqualityTransport` edge still
    /// cites the same equations and the kernel still replays the same
    /// denotation on admission, so the memo can never smuggle a different
    /// result past the receiver.
    static DENOTATIONS: RefCell<Vec<PairMemo<ScalarTerm, Vec<Proposition>, Option<ScalarTerm>>>> =
        const { RefCell::new(Vec::new()) };

    /// `ClosedIntegerRelation` answers the kernel already gave, one map per
    /// context token keyed by the ordered endpoint pair — the same
    /// pure-input memo shape as `DENOTATIONS`.
    static DECISIONS: RefCell<Vec<PairMemo<ScalarTerm, ScalarTerm, bool>>> =
        const { RefCell::new(Vec::new()) };
}

/// A pure-input memo keyed by two parts and nested so a lookup borrows both
/// parts instead of cloning them into a composite key. It stops recording at
/// `MAXIMUM_MEMOIZED` answers, like the single-map memo it replaces.
struct PairMemo<First, Second, Answer> {
    recorded: usize,
    answers: BTreeMap<First, BTreeMap<Second, Answer>>,
}

impl<First: Ord, Second: Ord, Answer> Default for PairMemo<First, Second, Answer> {
    fn default() -> Self {
        Self {
            recorded: 0,
            answers: BTreeMap::new(),
        }
    }
}

impl<First: Ord + Clone, Second: Ord, Answer> PairMemo<First, Second, Answer> {
    fn get<Borrowed: Ord + ?Sized>(&self, first: &First, second: &Borrowed) -> Option<&Answer>
    where
        Second: std::borrow::Borrow<Borrowed>,
    {
        self.answers.get(first)?.get(second)
    }

    fn record(&mut self, first: &First, second: Second, answer: Answer) {
        if self.recorded >= MAXIMUM_MEMOIZED {
            return;
        }
        if self
            .answers
            .entry(first.clone())
            .or_default()
            .insert(second, answer)
            .is_none()
        {
            self.recorded += 1;
        }
    }
}

/// A cited-fact roster resolved once for a fixed scope, then consulted for
/// many goals. Producers that ask a whole loop of equality questions under
/// one unchanged `(assumptions, semantic_axioms)` pair resolve the session
/// once — one full roster match — and then pay only the per-goal lookup
/// instead of re-matching the whole cited roster per question.
pub(super) struct Session {
    roster: Rc<Roster>,
}

impl Session {
    /// Prefer a literal already connected by cited equalities over expanding
    /// an observation into its field identity. A later read and an earlier
    /// guarded read can share that identity without sharing an SSA value.
    pub(super) fn literal_equality(&self, endpoint: &ScalarTerm) -> Option<ProofNode> {
        if let Some(component) = self.roster.components.get(endpoint)
            && let Some(literal) = self.roster.base_endpoints.iter().find(|candidate| {
                matches!(
                    candidate,
                    ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_)
                ) && candidate.scalar_type() == endpoint.scalar_type()
                    && self.roster.components.get(*candidate) == Some(component)
            })
        {
            return self.prove_equality(endpoint, literal);
        }
        // A cited definition can compute a literal no premise names yet.
        // Evaluate only its checked, closed scalar denotation: an open
        // wrapping operation never becomes mathematical multiplication.
        let (expanded, closure) = self.roster.transport(endpoint)?;
        let (integer_type, value) = expanded.integer_value()?;
        let literal = ScalarTerm::integer(integer_type, value).ok()?;
        if endpoint.scalar_type() != literal.scalar_type()
            || !decide(self.roster.token, &self.roster.context, &expanded, &literal)
        {
            return None;
        }
        Some(ProofNode {
            conclusion: Proposition::Equal(endpoint.clone(), literal.clone()),
            rule: ProofRule::ValueEqualityTransport {
                premise: Box::new(ProofNode {
                    conclusion: Proposition::Equal(expanded, literal),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                }),
                equalities: closure
                    .iter()
                    .map(|&index| self.roster.equal_facts[index].clone())
                    .collect(),
            },
        })
    }

    /// Same contract as `prove` under the session's fixed scope.
    pub(super) fn prove(&self, goal: &Proposition) -> Option<ProofNode> {
        self.roster.prove(goal)
    }

    /// `prove(Equal(left, right))` without building the proposition for the
    /// leaf pairs the roster separates.
    pub(super) fn prove_equality(
        &self,
        left: &ScalarTerm,
        right: &ScalarTerm,
    ) -> Option<ProofNode> {
        if self.roster.separates(left, right) {
            return None;
        }
        self.prove(&Proposition::Equal(left.clone(), right.clone()))
    }

    /// Value terms a cited equality states directly against `endpoint`, each
    /// carrying its `Equal(endpoint, alias)` certificate for the endpoint
    /// substitution to cite. This is the roster's equality adjacency rather
    /// than a cited-fact rescan per endpoint; the certificate still proves
    /// the whole chain, so a partner joined through intermediate hops keeps
    /// its full custody.
    pub(super) fn value_aliases(&self, endpoint: &ScalarTerm) -> Vec<(ScalarTerm, ProofNode)> {
        let mut aliases = Vec::new();
        for &index in self.roster.adjacency.get(endpoint).into_iter().flatten() {
            let Proposition::Equal(left, right) = &self.roster.equal_facts[index].conclusion else {
                continue;
            };
            let alias = if left == endpoint { right } else { left };
            if !matches!(alias, ScalarTerm::Value { .. })
                || aliases.iter().any(|(root, _)| root == alias)
            {
                continue;
            }
            let Some(equality) = self.prove_equality(endpoint, alias) else {
                continue;
            };
            aliases.push((alias.clone(), equality));
        }
        aliases
    }

    /// Memo of `integer_contradiction::prove` under the session's fixed
    /// scope and search generation. Its wrapping endpoint derivations spend
    /// invocation fuel, so a prior search's proof cannot bypass that charge
    /// and an exhausted refusal cannot poison a later search. Goals within
    /// one search share the derivation; the kernel still replays its facts.
    pub(super) fn contradiction(
        &self,
        derive: impl FnOnce() -> Option<ProofNode>,
    ) -> Option<ProofNode> {
        let generation = search_context::generation();
        if let Some((previous_generation, derived)) = &*self.roster.contradiction.borrow()
            && *previous_generation == generation
        {
            return derived.clone();
        }
        let derived = derive();
        *self.roster.contradiction.borrow_mut() = Some((generation, derived.clone()));
        derived
    }
}

/// Resolve the indexed roster for this scope once; the returned session
/// shares it for every goal the surrounding producer asks.
pub(super) fn session(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Session {
    Session {
        roster: roster(context, assumptions, semantic_axioms),
    }
}

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    session(context, assumptions, semantic_axioms).prove(goal)
}

/// The most-recently-used indexed roster for `scope`, building it on first
/// sight. Matching is always by full content.
fn roster(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Rc<Roster> {
    // The token equals another exactly when the contexts do, so matching it
    // keeps full-content matching without re-walking the value-type table.
    let token = search_context::token(context);
    let scope = scope_identity::token(assumptions, semantic_axioms);
    ROSTERS.with(|cell| {
        let mut rosters = cell.borrow_mut();
        let index = match rosters
            .iter()
            .rposition(|roster| roster.matches(token, scope, assumptions, semantic_axioms))
        {
            Some(index) => index,
            None => {
                rosters.push(Rc::new(Roster::new(
                    context,
                    token,
                    scope,
                    assumptions,
                    semantic_axioms,
                )));
                rosters.len() - 1
            }
        };
        let roster = rosters.remove(index);
        roster.scope.set(scope);
        rosters.push(roster.clone());
        if rosters.len() > MAXIMUM_ROSTERS {
            rosters.remove(0);
        }
        roster
    })
}

/// One cited-fact roster indexed for repeated `prove` entry: the citation map
/// the direct lookup consults, the cited equalities flattened through
/// unconditional conjunctions, an endpoint → equality adjacency for chain
/// traversal, and the `value = term` definition tables transport expands
/// endpoints through. Every field derives from `(context, assumptions,
/// semantic_axioms)` alone, so content-matched reuse is transparent.
struct Roster {
    context: PropositionContext,
    /// This context's content token; roster matching and the
    /// `denotation`/`decide` lookups compare it rather than the whole
    /// value-type table.
    token: usize,
    /// The scope token this roster was last matched under.
    scope: Cell<u64>,
    assumptions: Vec<Proposition>,
    semantic_axioms: Vec<Proposition>,
    /// First citation proof per top-level proposition, keeping cited order.
    cited: BTreeMap<Proposition, ProofNode>,
    /// Cited `Equal` proofs in the same order the flattening walk emits them.
    equal_facts: Vec<ProofNode>,
    /// Endpoint → `equal_facts` indices incident to it, either orientation.
    adjacency: BTreeMap<ScalarTerm, Vec<usize>>,
    /// Sorted distinct endpoints of `equal_facts`.
    base_endpoints: Vec<ScalarTerm>,
    /// `equal_facts` indices whose conclusion is `Equal(Value, _)`.
    definitions: Vec<usize>,
    /// First cited definition per value id — the substitution
    /// `from_semantic_axioms` performs inside denotation — used to gather
    /// exactly the equations an endpoint's expansion can consult.
    first_definitions: BTreeMap<ValueId, usize>,
    /// Every value id named on the left of a cited `Equal(Value, _)`.
    defined: BTreeSet<ValueId>,
    /// Cited-equality graph components: two endpoints share one exactly when
    /// a cited chain joins them — the reachability `chain_through` traverses.
    /// Leaf endpoints can move only along those edges, so leaves in distinct
    /// components reject without a traversal.
    components: BTreeMap<ScalarTerm, usize>,
    /// Endpoint → `(expansion, closure indices)` already computed here, or
    /// `None` when the endpoint has no expansion. The emitted edge cites the
    /// closure subset rather than the whole definition roster, which keeps
    /// each denotation proportional to the endpoint's own reach and lets the
    /// global memo share results across rosters citing the same definitions.
    transports: RefCell<BTreeMap<ScalarTerm, Option<(ScalarTerm, Vec<usize>)>>>,
    /// `prove` answers for this fixed roster — the producer's own memo of a
    /// pure function of `(roster, goal)`. A hit replays nothing: the returned
    /// proof still cites this roster's equations and the kernel still replays
    /// every step on admission.
    results: RefCell<BTreeMap<Proposition, Option<ProofNode>>>,
    /// The search generation and its derived contradiction, including an
    /// exhausted refusal. Fact indexes stay reusable across searches; this
    /// fuel-sensitive derivation belongs to one invocation.
    contradiction: RefCell<Option<(u64, Option<ProofNode>)>>,
}

impl Roster {
    /// Full-content matching: equal scope tokens imply equal content, and
    /// content decides whenever the tokens differ.
    fn matches(
        &self,
        token: usize,
        scope: u64,
        assumptions: &[Proposition],
        semantic_axioms: &[Proposition],
    ) -> bool {
        self.token == token
            && (self.scope.get() == scope
                || (self.assumptions.as_slice() == assumptions
                    && self.semantic_axioms.as_slice() == semantic_axioms))
    }

    fn new(
        context: &PropositionContext,
        token: usize,
        scope: u64,
        assumptions: &[Proposition],
        semantic_axioms: &[Proposition],
    ) -> Self {
        let mut cited = BTreeMap::new();
        let mut pending_facts = Vec::new();
        for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
            cited
                .entry(fact.clone())
                .or_insert_with(|| citation.proof(fact));
            pending_facts.push(citation.proof(fact));
        }
        let mut equal_facts = Vec::new();
        let mut order_facts = Vec::new();
        while let Some(proof) = pending_facts.pop() {
            match &proof.conclusion {
                Proposition::Conjunction(conjuncts) => {
                    for (conjunct, conclusion) in conjuncts.iter().enumerate() {
                        pending_facts.push(ProofNode {
                            conclusion: conclusion.clone(),
                            rule: ProofRule::ConjunctionElimination {
                                conjunction: Box::new(proof.clone()),
                                conjunct,
                            },
                        });
                    }
                }
                Proposition::Equal(_, _) => equal_facts.push(proof),
                Proposition::LessOrEqual(_, _) => order_facts.push(proof),
                _ => {}
            }
        }
        equal_facts.extend(order_equalities::derive(context, order_facts));
        let mut adjacency = BTreeMap::<ScalarTerm, Vec<usize>>::new();
        let mut base_endpoints = Vec::new();
        for (index, fact) in equal_facts.iter().enumerate() {
            let Proposition::Equal(source, destination) = &fact.conclusion else {
                unreachable!("equal_facts holds only Equal conclusions")
            };
            adjacency.entry(source.clone()).or_default().push(index);
            if destination != source {
                adjacency
                    .entry(destination.clone())
                    .or_default()
                    .push(index);
            }
            base_endpoints.push(source.clone());
            base_endpoints.push(destination.clone());
        }
        base_endpoints.sort();
        base_endpoints.dedup();
        let mut definitions = Vec::new();
        let mut first_definitions = BTreeMap::new();
        let mut defined = BTreeSet::new();
        for (index, fact) in equal_facts.iter().enumerate() {
            let Proposition::Equal(left @ ScalarTerm::Value { id, .. }, right) = &fact.conclusion
            else {
                continue;
            };
            defined.insert(*id);
            definitions.push(index);
            // `from_semantic_axioms` skips self-equations and keeps the first
            // definition per id; the closure mirror must choose identically.
            if left != right {
                first_definitions.entry(*id).or_insert(index);
            }
        }
        // Components of the cited-equality graph, labelled once so a leaf
        // endpoint pair in distinct components rejects without traversing.
        let mut components = BTreeMap::<ScalarTerm, usize>::new();
        let mut component = 0usize;
        for start in &base_endpoints {
            if components.contains_key(start) {
                continue;
            }
            let mut pending = vec![start.clone()];
            components.insert(start.clone(), component);
            while let Some(current) = pending.pop() {
                for &index in adjacency.get(&current).into_iter().flatten() {
                    let Proposition::Equal(source, destination) = &equal_facts[index].conclusion
                    else {
                        continue;
                    };
                    for next in [source, destination] {
                        if !components.contains_key(next) {
                            components.insert(next.clone(), component);
                            pending.push(next.clone());
                        }
                    }
                }
            }
            component += 1;
        }
        Self {
            token,
            scope: Cell::new(scope),
            context: context.clone(),
            assumptions: assumptions.to_vec(),
            semantic_axioms: semantic_axioms.to_vec(),
            cited,
            equal_facts,
            adjacency,
            base_endpoints,
            definitions,
            first_definitions,
            defined,
            components,
            transports: RefCell::new(BTreeMap::new()),
            results: RefCell::new(BTreeMap::new()),
            contradiction: RefCell::new(None),
        }
    }

    /// The `prove` answer for `goal` under this roster, memoized per goal:
    /// the result is a pure function of the roster's content and the goal,
    /// and a hit still returns a certificate the kernel replays verbatim.
    fn prove(&self, goal: &Proposition) -> Option<ProofNode> {
        if let Proposition::Equal(left, right) = goal
            && self.separates(left, right)
        {
            return None;
        }
        if let Some(result) = self.results.borrow().get(goal) {
            return result.clone();
        }
        let result = self
            .cited
            .get(goal)
            .cloned()
            .or_else(|| self.equality_chain(goal))
            .or_else(|| self.compound_citation(goal));
        if self.results.borrow().len() < MAXIMUM_ROSTER_RESULTS {
            self.results
                .borrow_mut()
                .insert(goal.clone(), result.clone());
        }
        result
    }

    /// Whether no cited chain can join two distinct leaves: a leaf moves only
    /// along cited equality edges, and a cited equality between the two
    /// would share their component. The chain search answers these pairs
    /// with no certificate, and so does this check, before the answer memo
    /// or a proposition is touched.
    fn separates(&self, left: &ScalarTerm, right: &ScalarTerm) -> bool {
        if left == right || compound(left) || compound(right) {
            return false;
        }
        match (self.components.get(left), self.components.get(right)) {
            (Some(left), Some(right)) => left != right,
            _ => true,
        }
    }

    /// Preserve a cited connective as a whole. From `a < 0 || a > 0` and
    /// equal observations of `a` and `b`, neither sign is individually known.
    /// Splitting the goal first needlessly searches guard combinations. The
    /// existing transport rule instead replays the original compound fact
    /// and only the unconditional definitions its endpoints actually use.
    /// Atomic arithmetic keeps its existing ordered-custody producers.
    fn compound_citation(&self, goal: &Proposition) -> Option<ProofNode> {
        if self.definitions.is_empty()
            || !matches!(
                goal,
                Proposition::Conjunction(_)
                    | Proposition::Disjunction(_)
                    | Proposition::Implication { .. }
            )
        {
            return None;
        }
        let mut goal_budget = MAXIMUM_DEFINITION_WORK;
        let mut goal_values = Vec::new();
        goal.visit_value_ids_bounded(&mut goal_budget, |value| goal_values.push(value))?;
        for (premise, proof) in &self.cited {
            if std::mem::discriminant(premise) != std::mem::discriminant(goal) {
                continue;
            }
            let mut remaining = goal_budget;
            let mut pending = goal_values.clone();
            if premise
                .visit_value_ids_bounded(&mut remaining, |value| pending.push(value))
                .is_none()
            {
                continue;
            }
            let Some(closure) = self.closure_from_values(pending, remaining) else {
                continue;
            };
            let equalities = closure
                .iter()
                .map(|&index| self.equal_facts[index].clone())
                .collect::<Vec<_>>();
            let denotation = |proposition| {
                check_value_equality_denotation(
                    &self.context,
                    proposition,
                    equalities.iter().map(|equality| &equality.conclusion),
                )
                .ok()
            };
            let Some(original) = denotation(premise) else {
                continue;
            };
            if denotation(goal).as_ref() == Some(&original) {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::ValueEqualityTransport {
                        premise: Box::new(proof.clone()),
                        equalities,
                    },
                });
            }
        }
        None
    }

    /// Follow explicitly cited equalities, proving every reversed edge. A
    /// machine result aliases its returned value, whose defining operation
    /// supplies the literal equation. No missing equation or implicit
    /// symmetry is assumed. When no cited chain reaches the goal, the
    /// kernel's licensed closed/open-term integer derivation may still
    /// certify an edge between two endpoints —
    /// `(acc + 1) + (remaining - 1) == acc + remaining` — which then composes
    /// like any cited fact; the kernel re-decides the same judgment on
    /// replay, so a wrong update or a stale endpoint still cannot pass.
    fn equality_chain(&self, goal: &Proposition) -> Option<ProofNode> {
        let Proposition::Equal(left, right) = goal else {
            return None;
        };
        if left == right {
            return Some(ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
            });
        }
        // A leaf endpoint carries no hidden arithmetic for transport or
        // normalization to expose: it can move only along cited equality
        // edges, so leaves in distinct cited components — including an
        // endpoint no cited equality names at all — can never meet.
        if !compound(left)
            && !compound(right)
            && self.components.get(left) != self.components.get(right)
        {
            return None;
        }
        self.chain_through(&[], left, right).or_else(|| {
            // The derivation fallback only pays for goals a cited chain could
            // never close: a leaf-to-leaf goal has no hidden arithmetic for
            // transport or normalization to expose, while the bound/order
            // producers re-enter this producer thousands of times for leaf
            // endpoint substitutions that must stay cheap.
            if !compound(left) && !compound(right) {
                return None;
            }
            // An update wraps cited names inside fresh compounds the cited
            // chain cannot name: `(acc + 1) + (remaining - 1)` holds the cited
            // `acc` and `remaining` only through the successor's own defining
            // equations. Transport certifies each endpoint's cited-definition
            // expansion before the kernel is asked to decide the exposed
            // arithmetic.
            let mut extra = self.transport_edges(left, right);
            extra.extend(self.normalization_edges(left, right, &extra));
            self.chain_through(&extra, left, right)
        })
    }

    /// Breadth-first traversal over the cited equalities plus `extra` edges:
    /// directed `Equal` edges, each traversed in either orientation with
    /// explicit symmetry certificates, and every hop composed by transitivity
    /// back to `left`.
    ///
    /// The traversal records each reached endpoint's parent hop and builds a
    /// certificate only for the path that reaches `right`: the same
    /// transitivity nesting the hop-by-hop composition produced, without
    /// cloning a growing prefix proof into every queued endpoint.
    fn chain_through(
        &self,
        extra: &[ProofNode],
        left: &ScalarTerm,
        right: &ScalarTerm,
    ) -> Option<ProofNode> {
        let mut extra_adjacency = BTreeMap::<&ScalarTerm, Vec<usize>>::new();
        for (index, edge) in extra.iter().enumerate() {
            let Proposition::Equal(source, destination) = &edge.conclusion else {
                continue;
            };
            extra_adjacency.entry(source).or_default().push(index);
            if destination != source {
                extra_adjacency.entry(destination).or_default().push(index);
            }
        }
        // `hops[i]` reaches `endpoints[i]` from `endpoints[parent]` along an
        // edge, reversed when the edge's destination is the parent. The
        // start endpoint has no hop.
        let mut endpoints = vec![left];
        let mut hops: Vec<Option<ChainHop<'_>>> = vec![None];
        let mut reached = BTreeSet::new();
        reached.insert(left);
        let mut index = 0;
        while index < endpoints.len() {
            let current = endpoints[index];
            let parent = index;
            index += 1;
            let incident = self
                .adjacency
                .get(current)
                .into_iter()
                .flatten()
                .map(|&index| &self.equal_facts[index])
                .chain(
                    extra_adjacency
                        .get(current)
                        .into_iter()
                        .flatten()
                        .map(|&index| &extra[index]),
                );
            for edge in incident {
                let Proposition::Equal(source, destination) = &edge.conclusion else {
                    continue;
                };
                let (destination, reversed) = if source == current {
                    (destination, false)
                } else if destination == current {
                    (source, true)
                } else {
                    continue;
                };
                if reached.contains(destination) {
                    continue;
                }
                let hop = ChainHop {
                    parent,
                    edge,
                    reversed,
                };
                if destination == right {
                    return Some(chain_proof(left, &endpoints, &hops, destination, hop));
                }
                reached.insert(destination);
                endpoints.push(destination);
                hops.push(Some(hop));
            }
        }
        None
    }

    /// Small invocation-owned component roster for the bounded normalization
    /// universe. Relabeling at most 64 entries avoids rebuilding a second
    /// adjacency index; all members are exact terms from the current scope.
    fn normalization_components(
        &self,
        endpoints: &[&ScalarTerm],
        transport: &[ProofNode],
    ) -> Vec<usize> {
        let mut components = (0..endpoints.len()).collect::<Vec<_>>();
        for edge in self.equal_facts.iter().chain(transport) {
            let Proposition::Equal(left, right) = &edge.conclusion else {
                continue;
            };
            let (Ok(left), Ok(right)) = (
                endpoints.binary_search(&left),
                endpoints.binary_search(&right),
            ) else {
                continue;
            };
            let source = components[left];
            let destination = components[right];
            if source != destination {
                for component in &mut components {
                    if *component == destination {
                        *component = source;
                    }
                }
            }
        }
        components
    }

    /// The sorted endpoint universe `endpoints()` used to compute per call:
    /// `base_endpoints` merged with the goal endpoints and any extra terms.
    /// Borrowed rather than cloned: callers read the endpoints in order and
    /// clone only those an emitted edge names.
    fn universe<'a>(
        &'a self,
        left: &'a ScalarTerm,
        right: &'a ScalarTerm,
        extra: &'a [ScalarTerm],
    ) -> Vec<&'a ScalarTerm> {
        let mut universe = self.base_endpoints.iter().collect::<Vec<_>>();
        for endpoint in [left, right].into_iter().chain(extra.iter()) {
            if let Err(position) = universe.binary_search(&endpoint) {
                universe.insert(position, endpoint);
            }
        }
        universe
    }

    /// Cited `value = term` definitions let transport expose the compounds an
    /// endpoint wraps around still-cited names: `v_succ` carrying
    /// `v_succ == add(v, 1)` inside `add(v_succ, w_succ)` expands to
    /// `add(add(v, 1), w_succ)` through `ValueEqualityTransport`, which the
    /// kernel rechecks under the same bounded denotation owner. The premise
    /// is reflexivity of the unexpanded endpoint, so the edge asserts exactly
    /// the cited equations and nothing more.
    ///
    /// The emitted edge cites only the definitions the endpoint's expansion
    /// actually consults — the transitive closure of the value ids inside it —
    /// rather than every cited definition. The kernel substitutes only the
    /// first definition of each id, so the closure is the complete input the
    /// denotation reads; replaying it yields the same expansion while keeping
    /// each call proportional to the endpoint's own reach instead of the
    /// whole roster.
    fn transport_edges(&self, left: &ScalarTerm, right: &ScalarTerm) -> Vec<ProofNode> {
        if self.definitions.is_empty() {
            return Vec::new();
        }
        let mut edges = Vec::new();
        for endpoint in self.universe(left, right, &[]) {
            // A leaf's expansion is its own cited edge; only a compound hides
            // cited names inside a constructor the chain cannot traverse.
            if !compound(endpoint) {
                continue;
            }
            // An endpoint holding no defined value cannot expand, so skip it
            // before paying the closure walk.
            if !endpoint.any_value_id(|value| self.defined.contains(&value)) {
                continue;
            }
            let Some((expanded, closure)) = self.transport(endpoint) else {
                continue;
            };
            if expanded == *endpoint {
                continue;
            }
            edges.push(ProofNode {
                conclusion: Proposition::Equal(endpoint.clone(), expanded),
                rule: ProofRule::ValueEqualityTransport {
                    premise: Box::new(ProofNode {
                        conclusion: Proposition::Equal(endpoint.clone(), endpoint.clone()),
                        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                    }),
                    equalities: closure
                        .iter()
                        .map(|&index| self.equal_facts[index].clone())
                        .collect(),
                },
            });
        }
        edges
    }

    /// The kernel-validated expansion of `endpoint` under its definition
    /// closure, memoized per endpoint inside this roster.
    fn transport(&self, endpoint: &ScalarTerm) -> Option<(ScalarTerm, Vec<usize>)> {
        if let Some(result) = self.transports.borrow().get(endpoint) {
            return result.clone();
        }
        let closure = self.closure_definitions(endpoint)?;
        let mut equations = closure
            .iter()
            .map(|&index| self.equal_facts[index].conclusion.clone())
            .collect::<Vec<_>>();
        // Canonical order keeps the memo key independent of the roster's
        // citation order; `from_semantic_axioms` keeps the first definition
        // per id either way, and the closure holds at most one per id.
        equations.sort();
        let expanded = denotation(self.token, &self.context, endpoint, &equations);
        let result = expanded.map(|expanded| (expanded, closure));
        self.transports
            .borrow_mut()
            .insert(endpoint.clone(), result.clone());
        result
    }

    /// `equal_facts` indices of the definitions `check_value_equality_
    /// denotation` would substitute inside `endpoint`: the first definition
    /// of every value id the endpoint reaches transitively through those
    /// definitions. Ids with no cited definition substitute to themselves
    /// and contribute nothing.
    fn closure_definitions(&self, endpoint: &ScalarTerm) -> Option<Vec<usize>> {
        let mut remaining = MAXIMUM_DEFINITION_WORK;
        let mut pending = Vec::new();
        if !endpoint.visit_value_ids(|id| {
            let Some(rest) = remaining.checked_sub(1) else {
                return false;
            };
            remaining = rest;
            pending.push(id);
            true
        }) {
            return None;
        }
        self.closure_from_values(pending, remaining)
    }

    fn closure_from_values(
        &self,
        mut pending: Vec<ValueId>,
        mut remaining: usize,
    ) -> Option<Vec<usize>> {
        let mut collected = BTreeSet::new();
        while let Some(id) = pending.pop() {
            remaining = remaining.checked_sub(1)?;
            let Some(&definition) = self.first_definitions.get(&id) else {
                continue;
            };
            if !collected.insert(definition) {
                continue;
            }
            // Keep every selected equation, including cycles: collection is
            // only a finite dependency slice. The kernel still refuses cyclic
            // substitution. Shared DAG predecessors need traversal only once.
            self.equal_facts[definition]
                .conclusion
                .visit_value_ids_bounded(&mut remaining, |dependency| {
                    if dependency != id {
                        pending.push(dependency);
                    }
                })?;
        }
        Some(collected.into_iter().collect())
    }

    /// Edges `PrimitiveJudgment::ClosedIntegerRelation` itself decides between
    /// the goal endpoints and every distinct cited endpoint. Each emitted
    /// edge carries the primitive as its own proof, so custody stays explicit
    /// and the receiver re-derives rather than trusts the producer's choice.
    fn normalization_edges(
        &self,
        left: &ScalarTerm,
        right: &ScalarTerm,
        transport: &[ProofNode],
    ) -> Vec<ProofNode> {
        // The roster includes each transported expansion; the pair bound
        // below — not this count — is what bounds kernel derivations.
        let expansions = transport
            .iter()
            .filter_map(|edge| match &edge.conclusion {
                Proposition::Equal(_, expanded) => Some(expanded.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let endpoints = self.universe(left, right, &expansions);
        if endpoints.len() > MAXIMUM_NORMALIZATION_ENDPOINTS {
            return Vec::new();
        }
        // Only a pair holding at least one compound endpoint can be a new
        // decision: distinct atoms and literals were already closed-evaluated
        // or stay distinct, so leaf pairs never certify an edge the cited
        // roster lacked. The filter also keeps the kernel's own derivation
        // cheap — it runs once per candidate pair, not per endpoint.
        // Compound pairs go first: a transported expansion differs from its
        // cited target only in shape, while a leaf-to-leaf question is the
        // cited chain's own job, so sorted order must not let leaf pairs
        // exhaust the candidate bound before the deciding pair is asked.
        let compounds = endpoints
            .iter()
            .enumerate()
            .filter(|(_, endpoint)| compound(endpoint))
            .map(|(position, _)| position)
            .collect::<Vec<_>>();
        let mut pairs = Vec::new();
        for (first, &source) in compounds.iter().enumerate() {
            for &destination in &compounds[first + 1..] {
                pairs.push((source, destination));
            }
        }
        for &source in &compounds {
            for (destination, endpoint) in endpoints.iter().enumerate() {
                if !compound(endpoint) {
                    pairs.push((source, destination));
                }
            }
        }
        // A normalization edge is useful first when it joins the components
        // containing the actual goal endpoints. Incidental equations can
        // otherwise spend the pair budget before that bridge is considered.
        // Components include only cited or independently checked transport
        // edges: this ordering contributes no new equality authority.
        let components = self.normalization_components(&endpoints, transport);
        let left_component = components[endpoints
            .binary_search(&left)
            .expect("goal endpoint is in the universe")];
        let right_component = components[endpoints
            .binary_search(&right)
            .expect("goal endpoint is in the universe")];
        pairs.sort_by_key(|&(source, destination)| {
            let source = components[source];
            let destination = components[destination];
            if (source == left_component && destination == right_component)
                || (source == right_component && destination == left_component)
            {
                0
            } else if [source, destination]
                .iter()
                .any(|component| *component == left_component || *component == right_component)
            {
                1
            } else {
                2
            }
        });
        let pairs = pairs
            .into_iter()
            .take(MAXIMUM_NORMALIZATION_PAIRS)
            .collect::<Vec<_>>();
        let decisions = decide_pairs(
            self.token,
            &self.context,
            pairs
                .iter()
                .map(|&(source, destination)| (endpoints[source], endpoints[destination])),
        );
        let mut edges = Vec::new();
        for (&(source, destination), decided) in pairs.iter().zip(decisions) {
            if decided {
                edges.push(ProofNode {
                    conclusion: Proposition::Equal(
                        endpoints[source].clone(),
                        endpoints[destination].clone(),
                    ),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                });
            }
        }
        edges
    }
}

/// One breadth-first hop of `Roster::chain_through`.
#[derive(Clone, Copy)]
struct ChainHop<'edge> {
    parent: usize,
    edge: &'edge ProofNode,
    reversed: bool,
}

impl ChainHop<'_> {
    /// The hop's own certificate: the cited edge, or its symmetry when the
    /// traversal crossed it from destination to source.
    fn proof(self) -> ProofNode {
        if !self.reversed {
            return self.edge.clone();
        }
        let Proposition::Equal(source, destination) = &self.edge.conclusion else {
            unreachable!("chain edges are equalities")
        };
        ProofNode {
            conclusion: Proposition::Equal(destination.clone(), source.clone()),
            rule: ProofRule::EqualitySymmetry {
                equality: Box::new(self.edge.clone()),
            },
        }
    }
}

/// Compose the path ending in `last` (which reaches `destination`) back to
/// `left`: the first hop stands alone and every later hop joins its prefix
/// through transitivity, exactly as the traversal visited them.
fn chain_proof(
    left: &ScalarTerm,
    endpoints: &[&ScalarTerm],
    hops: &[Option<ChainHop<'_>>],
    destination: &ScalarTerm,
    last: ChainHop<'_>,
) -> ProofNode {
    let mut path = vec![(destination, last)];
    let mut parent = last.parent;
    while let Some(hop) = hops[parent] {
        path.push((endpoints[parent], hop));
        parent = hop.parent;
    }
    let mut proof: Option<ProofNode> = None;
    for (reached, hop) in path.into_iter().rev() {
        let next = hop.proof();
        proof = Some(match proof {
            None => next,
            Some(prefix) => ProofNode {
                conclusion: Proposition::Equal(left.clone(), reached.clone()),
                rule: ProofRule::EqualityTransitivity {
                    left_equals_middle: Box::new(prefix),
                    middle_equals_right: Box::new(next),
                },
            },
        });
    }
    proof.expect("a reached endpoint has at least one hop")
}

/// The kernel's `check_value_equality_denotation` memoized on its exact
/// input `(context token, endpoint, equations)`: a hit is the same `Ok`/`Err`
/// the call would return, and the caller still emits an edge citing the same
/// equations for the kernel to replay.
fn denotation(
    token: usize,
    context: &PropositionContext,
    endpoint: &ScalarTerm,
    equations: &[Proposition],
) -> Option<ScalarTerm> {
    DENOTATIONS.with(|cell| {
        let mut cell = cell.borrow_mut();
        while cell.len() <= token {
            cell.push(PairMemo::default());
        }
        let map = &mut cell[token];
        if let Some(result) = map.get(endpoint, equations) {
            return result.clone();
        }
        let premise = Proposition::Equal(endpoint.clone(), endpoint.clone());
        let result = check_value_equality_denotation(context, &premise, equations.iter())
            .ok()
            .and_then(|denoted| match denoted {
                Proposition::Equal(expanded, _) => Some(expanded),
                _ => None,
            });
        map.record(endpoint, equations.to_vec(), result.clone());
        result
    })
}

/// The kernel's `decide_primitive` for `ClosedIntegerRelation` memoized on
/// its exact input `(context token, ordered endpoint pair)`.
fn decide(
    token: usize,
    context: &PropositionContext,
    source: &ScalarTerm,
    destination: &ScalarTerm,
) -> bool {
    DECISIONS.with(|cell| {
        let mut cell = cell.borrow_mut();
        while cell.len() <= token {
            cell.push(PairMemo::default());
        }
        let map = &mut cell[token];
        if let Some(&result) = map.get(source, destination) {
            return result;
        }
        let conclusion = Proposition::Equal(source.clone(), destination.clone());
        let result = decide_primitive(
            context,
            &conclusion,
            PrimitiveJudgment::ClosedIntegerRelation,
        )
        .is_ok();
        map.record(source, destination.clone(), result);
        result
    })
}

/// `decide` over a run of endpoint pairs under one memo borrow. Consecutive
/// pairs sharing a source look that source up once. The pairs are distinct,
/// so a group's lookups cannot observe its own new answers, and misses are
/// decided and recorded in pair order: answers and memo contents match
/// asking `decide` pair by pair in the same order.
fn decide_pairs<'a>(
    token: usize,
    context: &PropositionContext,
    pairs: impl Iterator<Item = (&'a ScalarTerm, &'a ScalarTerm)>,
) -> Vec<bool> {
    let pairs = pairs.collect::<Vec<_>>();
    DECISIONS.with(|cell| {
        let mut cell = cell.borrow_mut();
        while cell.len() <= token {
            cell.push(PairMemo::default());
        }
        let map = &mut cell[token];
        let mut decisions = Vec::with_capacity(pairs.len());
        for group in pairs.chunk_by(|left, right| std::ptr::eq(left.0, right.0)) {
            let source = group[0].0;
            let known = {
                let answers = map.answers.get(source);
                group
                    .iter()
                    .map(|(_, destination)| {
                        answers.and_then(|answers| answers.get(*destination).copied())
                    })
                    .collect::<Vec<_>>()
            };
            for (&(_, destination), known) in group.iter().zip(known) {
                let result = match known {
                    Some(result) => result,
                    None => {
                        let conclusion = Proposition::Equal(source.clone(), destination.clone());
                        let result = decide_primitive(
                            context,
                            &conclusion,
                            PrimitiveJudgment::ClosedIntegerRelation,
                        )
                        .is_ok();
                        map.record(source, destination.clone(), result);
                        result
                    }
                };
                decisions.push(result);
            }
        }
        decisions
    })
}

/// A term carrying structure the kernel's derivation can decide — anything
/// beyond a leaf value, literal or Boolean atom.
fn compound(term: &ScalarTerm) -> bool {
    !matches!(
        term,
        ScalarTerm::Value { .. } | ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_)
    )
}
