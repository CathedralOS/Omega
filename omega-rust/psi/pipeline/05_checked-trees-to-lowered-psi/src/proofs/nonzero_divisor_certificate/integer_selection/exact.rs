//! Exact retained proposition proof custody.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use proof_admission::{
    PrimitiveJudgment, ProofNode, ProofRule, check_value_equality_denotation, decide_primitive,
};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ValueId};

use super::super::integer_evidence::cited_facts;

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
    /// inputs and every roster is matched by full content, never by position
    /// or pointer, so a stale entry can at worst be rebuilt: the produced
    /// proof still passes the kernel unchanged.
    static ROSTERS: RefCell<Vec<Rc<Roster>>> = const { RefCell::new(Vec::new()) };

    /// Distinct proposition contexts this worker has proved under. The index
    /// doubles as the memo token below so hot lookups compare one integer
    /// instead of re-walking the whole value-type table per call.
    static CONTEXTS: RefCell<Vec<PropositionContext>> = const { RefCell::new(Vec::new()) };

    /// Kernel-computed endpoint expansions, one map per context token, keyed
    /// by the exact denotation input `(endpoint, equations)` — a memo of the
    /// deterministic `check_value_equality_denotation` call, nothing more. A
    /// hit replays nothing: the emitted `ValueEqualityTransport` edge still
    /// cites the same equations and the kernel still replays the same
    /// denotation on admission, so the memo can never smuggle a different
    /// result past the receiver.
    static DENOTATIONS: RefCell<
        Vec<BTreeMap<(ScalarTerm, Vec<Proposition>), Option<ScalarTerm>>>,
    > = const { RefCell::new(Vec::new()) };

    /// `ClosedIntegerRelation` answers the kernel already gave, one map per
    /// context token keyed by the ordered endpoint pair — the same
    /// pure-input memo shape as `DENOTATIONS`.
    static DECISIONS: RefCell<Vec<BTreeMap<(ScalarTerm, ScalarTerm), bool>>> =
        const { RefCell::new(Vec::new()) };
}

/// The memo token for `context`, allocating a slot on first sight. Compared
/// by content once per roster build; afterwards only the token travels.
fn context_token(context: &PropositionContext) -> usize {
    CONTEXTS.with(|cell| {
        let mut cell = cell.borrow_mut();
        match cell.iter().position(|existing| existing == context) {
            Some(token) => token,
            None => {
                cell.push(context.clone());
                cell.len() - 1
            }
        }
    })
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
    /// Same contract as `prove` under the session's fixed scope.
    pub(super) fn prove(&self, goal: &Proposition) -> Option<ProofNode> {
        self.roster.prove(goal)
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
            let Some(equality) = self.prove(&Proposition::Equal(endpoint.clone(), alias.clone()))
            else {
                continue;
            };
            aliases.push((alias.clone(), equality));
        }
        aliases
    }

    /// Memo of `integer_contradiction::prove` under the session's fixed
    /// scope: the derived contradiction depends on `(context, assumptions,
    /// semantic_axioms)` alone — its `DefinitionIndex` argument is itself a
    /// pure memo — so every goal `prove_contradiction` visits under this
    /// scope shares one derivation instead of re-running the legs for each.
    /// The returned proof still cites this scope's facts for the kernel to
    /// replay.
    pub(super) fn contradiction(
        &self,
        derive: impl FnOnce() -> Option<ProofNode>,
    ) -> Option<ProofNode> {
        if let Some(derived) = &*self.roster.contradiction.borrow() {
            return derived.clone();
        }
        let derived = derive();
        *self.roster.contradiction.borrow_mut() = Some(derived.clone());
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
    ROSTERS.with(|cell| {
        let mut rosters = cell.borrow_mut();
        let index = match rosters
            .iter()
            .rposition(|roster| roster.matches(context, assumptions, semantic_axioms))
        {
            Some(index) => index,
            None => {
                rosters.push(Rc::new(Roster::new(context, assumptions, semantic_axioms)));
                rosters.len() - 1
            }
        };
        let roster = rosters.remove(index);
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
    /// This context's slot in `CONTEXTS`; `denotation`/`decide` lookups
    /// compare the token rather than the whole value-type table.
    token: usize,
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
    /// The derived integer contradiction for this scope, or `None` once
    /// derived — `integer_contradiction::prove` is a pure function of this
    /// roster's scope, so `Session::contradiction` computes it at most once.
    contradiction: RefCell<Option<Option<ProofNode>>>,
}

impl Roster {
    fn matches(
        &self,
        context: &PropositionContext,
        assumptions: &[Proposition],
        semantic_axioms: &[Proposition],
    ) -> bool {
        self.assumptions.as_slice() == assumptions
            && self.semantic_axioms.as_slice() == semantic_axioms
            && self.context == *context
    }

    fn new(
        context: &PropositionContext,
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
                _ => {}
            }
        }
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
            token: context_token(context),
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
        if let Some(result) = self.results.borrow().get(goal) {
            return result.clone();
        }
        let result = self
            .cited
            .get(goal)
            .cloned()
            .or_else(|| self.equality_chain(goal));
        if self.results.borrow().len() < MAXIMUM_ROSTER_RESULTS {
            self.results
                .borrow_mut()
                .insert(goal.clone(), result.clone());
        }
        result
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
    fn chain_through(
        &self,
        extra: &[ProofNode],
        left: &ScalarTerm,
        right: &ScalarTerm,
    ) -> Option<ProofNode> {
        let mut extra_adjacency = BTreeMap::<ScalarTerm, Vec<usize>>::new();
        for (index, edge) in extra.iter().enumerate() {
            let Proposition::Equal(source, destination) = &edge.conclusion else {
                continue;
            };
            extra_adjacency
                .entry(source.clone())
                .or_default()
                .push(index);
            if destination != source {
                extra_adjacency
                    .entry(destination.clone())
                    .or_default()
                    .push(index);
            }
        }
        let mut pending = vec![(left.clone(), None::<ProofNode>)];
        let mut reached = BTreeSet::new();
        reached.insert(left.clone());
        let mut index = 0;
        while index < pending.len() {
            let (current, prefix) = pending[index].clone();
            index += 1;
            let incident = self
                .adjacency
                .get(&current)
                .into_iter()
                .flatten()
                .map(|&index| &self.equal_facts[index])
                .chain(
                    extra_adjacency
                        .get(&current)
                        .into_iter()
                        .flatten()
                        .map(|&index| &extra[index]),
                );
            for next in incident {
                let Proposition::Equal(source, destination) = &next.conclusion else {
                    continue;
                };
                let (destination, next) = if source == &current {
                    (destination, next.clone())
                } else if destination == &current {
                    (
                        source,
                        ProofNode {
                            conclusion: Proposition::Equal(destination.clone(), source.clone()),
                            rule: ProofRule::EqualitySymmetry {
                                equality: Box::new(next.clone()),
                            },
                        },
                    )
                } else {
                    continue;
                };
                if reached.contains(destination) {
                    continue;
                }
                let proof = if let Some(prefix) = prefix.clone() {
                    ProofNode {
                        conclusion: Proposition::Equal(left.clone(), destination.clone()),
                        rule: ProofRule::EqualityTransitivity {
                            left_equals_middle: Box::new(prefix),
                            middle_equals_right: Box::new(next),
                        },
                    }
                } else {
                    next
                };
                if destination == right {
                    return Some(proof);
                }
                reached.insert(destination.clone());
                pending.push((destination.clone(), Some(proof)));
            }
        }
        None
    }

    /// The sorted endpoint universe `endpoints()` used to compute per call:
    /// `base_endpoints` merged with the goal endpoints and any extra terms.
    fn universe(
        &self,
        left: &ScalarTerm,
        right: &ScalarTerm,
        extra: &[ScalarTerm],
    ) -> Vec<ScalarTerm> {
        let mut universe = self.base_endpoints.clone();
        for endpoint in [left, right].into_iter().chain(extra.iter()) {
            if let Err(position) = universe.binary_search(endpoint) {
                universe.insert(position, endpoint.clone());
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
            if !compound(&endpoint) {
                continue;
            }
            // An endpoint holding no defined value cannot expand, so skip it
            // before paying the closure walk.
            if !endpoint.any_value_id(|value| self.defined.contains(&value)) {
                continue;
            }
            let Some((expanded, closure)) = self.transport(&endpoint) else {
                continue;
            };
            if expanded == endpoint {
                continue;
            }
            edges.push(ProofNode {
                conclusion: Proposition::Equal(endpoint.clone(), expanded),
                rule: ProofRule::ValueEqualityTransport {
                    premise: Box::new(ProofNode {
                        conclusion: Proposition::Equal(endpoint.clone(), endpoint),
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
        let closure = self.closure_definitions(endpoint);
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
    fn closure_definitions(&self, endpoint: &ScalarTerm) -> Vec<usize> {
        let mut active = BTreeSet::new();
        let mut collected = BTreeSet::new();
        self.collect_definitions(endpoint, &mut active, &mut collected);
        collected.into_iter().collect()
    }

    fn collect_definitions(
        &self,
        term: &ScalarTerm,
        active: &mut BTreeSet<ValueId>,
        collected: &mut BTreeSet<usize>,
    ) {
        let mut members = Vec::new();
        term.visit_value_ids(|id| {
            members.push(id);
            true
        });
        for id in members {
            let Some(&definition) = self.first_definitions.get(&id) else {
                continue;
            };
            collected.insert(definition);
            // `active` mirrors the kernel's substitution stack: an id already
            // on the path means a cyclic definition, and the denotation call
            // rejects the same expansion the kernel would.
            if !active.insert(id) {
                continue;
            }
            let Proposition::Equal(_, defined) = &self.equal_facts[definition].conclusion else {
                unreachable!("definition indices name Equal(Value, _) facts")
            };
            self.collect_definitions(defined, active, collected);
            active.remove(&id);
        }
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
        let mut edges = Vec::new();
        for (source, destination) in pairs.into_iter().take(MAXIMUM_NORMALIZATION_PAIRS) {
            if decide(
                self.token,
                &self.context,
                &endpoints[source],
                &endpoints[destination],
            ) {
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
            cell.push(BTreeMap::new());
        }
        let map = &mut cell[token];
        let key = (endpoint.clone(), equations.to_vec());
        if let Some(result) = map.get(&key) {
            return result.clone();
        }
        let premise = Proposition::Equal(endpoint.clone(), endpoint.clone());
        let result = check_value_equality_denotation(context, &premise, equations.iter())
            .ok()
            .and_then(|denoted| match denoted {
                Proposition::Equal(expanded, _) => Some(expanded),
                _ => None,
            });
        if map.len() < MAXIMUM_MEMOIZED {
            map.insert(key, result.clone());
        }
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
            cell.push(BTreeMap::new());
        }
        let map = &mut cell[token];
        let key = (source.clone(), destination.clone());
        if let Some(&result) = map.get(&key) {
            return result;
        }
        let conclusion = Proposition::Equal(source.clone(), destination.clone());
        let result = decide_primitive(
            context,
            &conclusion,
            PrimitiveJudgment::ClosedIntegerRelation,
        )
        .is_ok();
        if map.len() < MAXIMUM_MEMOIZED {
            map.insert(key, result);
        }
        result
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

#[cfg(test)]
mod tests {
    use super::{PrimitiveJudgment, ProofRule, Proposition, prove};
    use proof_admission::check_certificate;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, PropositionContext, ScalarTerm, ScalarType, ValueId,
    };

    fn context() -> PropositionContext {
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap());
        PropositionContext::from_value_types(
            (1..=4).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
        )
        .unwrap()
    }

    fn equality(left: u64, right: u64) -> Proposition {
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap());
        Proposition::Equal(
            ScalarTerm::value(ValueId::new(left).unwrap(), scalar_type),
            ScalarTerm::value(ValueId::new(right).unwrap(), scalar_type),
        )
    }

    #[test]
    fn directed_equality_chain_replays_each_exact_citation() {
        let goal = equality(1, 4);
        let assumptions = [Proposition::Conjunction(vec![
            equality(2, 3),
            equality(3, 4),
        ])];
        let semantic_axioms = [equality(1, 2)];
        let proof = prove(&context(), &goal, &assumptions, &semantic_axioms)
            .expect("three cited equalities");
        assert!(matches!(proof.rule, ProofRule::EqualityTransitivity { .. }));
        check_certificate(&context(), &goal, &assumptions, &semantic_axioms, &proof)
            .expect("kernel replays the directed chain");

        assert!(check_certificate(&context(), &goal, &assumptions, &[], &proof).is_err());
        assert!(
            check_certificate(
                &context(),
                &goal,
                &[equality(2, 4)],
                &semantic_axioms,
                &proof
            )
            .is_err(),
            "substituted hypothesis shape cannot satisfy the existing citation"
        );
    }

    #[test]
    fn missing_equality_edges_are_not_invented() {
        let goal = equality(1, 4);
        for semantic_axioms in [
            vec![equality(1, 2), equality(3, 4)],
            vec![equality(2, 1), equality(3, 4)],
            vec![equality(1, 2), equality(3, 2)],
        ] {
            assert!(
                prove(&context(), &goal, &[], &semantic_axioms).is_none(),
                "{semantic_axioms:?}"
            );
        }
    }

    #[test]
    fn reversed_edges_have_explicit_symmetry_certificates() {
        let goal = equality(1, 4);
        let axioms = [equality(2, 1), equality(2, 3), equality(4, 3)];
        let proof = prove(&context(), &goal, &[], &axioms).expect("two reversed edges");
        check_certificate(&context(), &goal, &[], &axioms, &proof).unwrap();
        let mut changed = axioms.clone();
        changed[0] = equality(1, 2);
        assert!(
            check_certificate(&context(), &goal, &[], &changed, &proof).is_err(),
            "a symmetric proposition does not replace the cited proof node"
        );
    }

    #[test]
    fn equality_cycles_terminate_without_hiding_a_reachable_target() {
        let goal = equality(1, 4);
        let mut semantic_axioms = vec![
            equality(1, 2),
            equality(2, 1),
            equality(2, 3),
            equality(3, 2),
        ];
        assert!(
            prove(&context(), &goal, &[], &semantic_axioms).is_none(),
            "cycle is not a missing exit"
        );
        semantic_axioms.push(equality(3, 4));
        let proof =
            prove(&context(), &goal, &[], &semantic_axioms).expect("reachable target after cycle");
        check_certificate(&context(), &goal, &[], &semantic_axioms, &proof).unwrap();
    }

    #[test]
    fn reflexivity_needs_no_citation_and_non_equality_is_not_a_chain() {
        let goal = equality(1, 1);
        let proof = prove(&context(), &goal, &[], &[]).expect("reflexivity");
        check_certificate(&context(), &goal, &[], &[], &proof).unwrap();
        let Proposition::Equal(left, right) = equality(1, 4) else {
            unreachable!()
        };
        assert!(
            prove(
                &context(),
                &Proposition::LessOrEqual(left, right),
                &[],
                &[equality(1, 4)]
            )
            .is_none()
        );
    }

    fn integer_type() -> IntegerType {
        IntegerType::new(IntegerSign::Unsigned, 16).unwrap()
    }

    fn add(left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
        ScalarTerm::exact_integer_add(integer_type(), left, right).unwrap()
    }

    fn subtract(left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
        ScalarTerm::exact_integer_subtract(integer_type(), left, right).unwrap()
    }

    fn term(identity: u64) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer_type()),
        )
    }

    fn one() -> ScalarTerm {
        ScalarTerm::integer(integer_type(), IntegerValue::Unsigned(1)).unwrap()
    }

    /// `(acc + 1) + (remaining - 1)`: the arrival the cyclic guarantee must
    /// preserve against `acc + remaining`.
    fn updated_sum() -> ScalarTerm {
        add(add(term(1), one()), subtract(term(2), one()))
    }

    fn conserved_sum() -> ScalarTerm {
        add(term(1), term(2))
    }

    #[test]
    fn normalization_certifies_the_accumulator_recurrence_edge() {
        let goal = Proposition::Equal(updated_sum(), conserved_sum());
        let proof = prove(&context(), &goal, &[], &[]).expect("certified normalization edge");
        // Endpoint order is canonical, so the single hop is either the
        // primitive itself or its explicit symmetry certificate.
        match &proof.rule {
            ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation) => {}
            ProofRule::EqualitySymmetry { equality } => assert!(matches!(
                equality.rule,
                ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
            )),
            _ => panic!("normalization edge must carry the licensed primitive"),
        }
        check_certificate(&context(), &goal, &[], &[], &proof).unwrap();
    }

    #[test]
    fn normalization_edges_compose_through_cited_equalities() {
        let goal = Proposition::Equal(updated_sum(), term(3));
        let axioms = [Proposition::Equal(term(3), conserved_sum())];
        let proof = prove(&context(), &goal, &[], &axioms)
            .expect("normalization edge then reversed citation");
        assert!(matches!(proof.rule, ProofRule::EqualityTransitivity { .. }));
        check_certificate(&context(), &goal, &[], &axioms, &proof).unwrap();
        assert!(
            check_certificate(&context(), &goal, &[], &[], &proof).is_err(),
            "dropping the cited hop invalidates the chain even though the \
             normalization edge needs no citation"
        );
    }

    #[test]
    fn successor_endpoint_transports_through_cited_definitions() {
        // The latch arrival shape: the goal names only the successor values,
        // whose cited definitions hide `acc`/`remaining` inside compounds.
        // `add(v17, v15)` must reach the cited `add(v4, v3)` hypothesis:
        // transport expands `v17 == add(v7, v16)` and `v15 == sub(v6, v14)`
        // through `v7 == v4`, `v6 == v3`, `v16 == 1`, `v14 == 1`.
        let goal = Proposition::Equal(add(term(2), term(1)), add(term(17), term(15)));
        let axioms = [
            // The header invariant hypothesis over its parameters.
            Proposition::Equal(add(term(2), term(1)), add(term(4), term(3))),
            Proposition::Equal(term(6), term(3)),
            Proposition::Equal(term(7), term(4)),
            Proposition::Equal(term(14), one()),
            Proposition::Equal(term(15), subtract(term(6), term(14))),
            Proposition::Equal(term(16), one()),
            Proposition::Equal(term(17), add(term(7), term(16))),
        ];
        // context() registers values 1..=4 only; widen it for this roster.
        let scalar_type = ScalarType::Integer(integer_type());
        let context = PropositionContext::from_value_types(
            (1..=17).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
        )
        .unwrap();
        let proof = prove(&context, &goal, &[], &axioms)
            .expect("transport then cited hypothesis closes the arrival");
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        assert!(
            prove(&context, &goal, &[], &axioms[..axioms.len() - 1]).is_none(),
            "dropping the `v17` definition leaves an unprovable gap"
        );
    }

    #[test]
    fn a_wrong_update_is_decided_not_equal() {
        // `acc` forwarded unchanged: `acc + (remaining - 1)` is strictly less
        // than `acc + remaining`, so the kernel refuses the edge outright —
        // it is a decided non-equality, not an undecided search.
        let forwarded = add(term(1), subtract(term(2), one()));
        let goal = Proposition::Equal(forwarded, conserved_sum());
        assert!(prove(&context(), &goal, &[], &[]).is_none());
        // Off-by-one drift stays refused even when the update is a compound.
        let drifted = add(term(1), add(term(2), one()));
        assert!(
            prove(
                &context(),
                &Proposition::Equal(updated_sum(), drifted),
                &[],
                &[]
            )
            .is_none()
        );
    }
}
