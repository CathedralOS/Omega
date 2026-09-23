//! Ordered affine-definition index recording for certificate production.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use proof_admission::{
    CheckedIntegerAffineForm, IntegerAffineWitness, ProofNode, check_integer_affine_witness,
    integer_affine_truth_bounds, map_integer_affine_bound,
};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ValueId};

use super::candidates;

/// Source-ordered semantic rows that may extend one exact affine value chain.
/// This selects candidates only; the kernel remains authoritative for every
/// prefix and completed proof.
///
/// The index splits in two: `tables` holds every derivation that is a pure
/// function of `(context, semantic_axioms)` — the candidate maps built at
/// construction, frontier words, literal-axiom selections, and kernel-checked
/// word outcomes — and is shared across every index built for the same
/// roster content on this thread, because re-deriving them per goal is where
/// the producer's repeated cost concentrates. `proofs` holds the produced
/// certificates per goal under one exact `(context, assumptions,
/// semantic_axioms)` scope and is shared across every index built under that
/// scope: a produced proof cites the scope's facts by position, so it may be
/// replayed only under the content-equal scope that produced it — which is
/// exactly what the shared store's full-content match guarantees.
pub(crate) struct DefinitionIndex {
    tables: Rc<RefCell<Tables>>,
    proofs: Rc<RefCell<ScopeProofs>>,
}

/// Pure `(context, semantic_axioms)` derivations one index build would
/// otherwise repeat for every goal and every case branch under the same
/// roster. Nothing here reads the assumption roster: `check_word` outcomes
/// carry propositions the caller still proves under its own scope, and the
/// witness check itself consults only the context and the axiom rows.
struct Tables {
    by_input: BTreeMap<ScalarTerm, Vec<usize>>,
    by_output: BTreeMap<ScalarTerm, Vec<usize>>,
    cast_roots: Rc<[ScalarTerm]>,
    cast_definitions: BTreeMap<ScalarTerm, Option<(usize, ScalarTerm)>>,
    words_by_root: BTreeMap<ScalarTerm, Rc<[Vec<usize>]>>,
    words_by_root_and_target: BTreeMap<(ScalarTerm, ScalarTerm), Rc<[Vec<usize>]>>,
    literal_axioms_by_witness:
        BTreeMap<(ScalarTerm, Vec<usize>, ScalarTerm), Option<Vec<Option<usize>>>>,
    /// `check_word` outcomes per `(bound proposition, root, target,
    /// definition word)`: the kernel-checked form and its required evidence
    /// replay without re-running the witness check for every candidate the
    /// word loops meet.
    checked_words:
        BTreeMap<(Proposition, ScalarTerm, ScalarTerm, Vec<usize>), Option<Rc<CheckedWord>>>,
    /// `check_integer_affine_witness` outcomes per witness. The check is a
    /// pure `(context, semantic_axioms)` replay — the kernel consults only
    /// the axiom rows and the shared context — so one checked form serves
    /// every goal and every root bound the same word is offered to. The
    /// innermost key keeps the literal axioms because the form cites them.
    /// `None` records a rejection so it never replays.
    affine_forms: BTreeMap<
        ScalarTerm,
        BTreeMap<
            ScalarTerm,
            BTreeMap<(Vec<usize>, Vec<Option<usize>>), Option<Rc<CheckedIntegerAffineForm>>>,
        >,
    >,
    /// `map_integer_affine_bound` outcomes per `(form, root bound)`. The
    /// first key is the `Rc` address an `affine_forms` entry keeps alive, so
    /// the form identity is stable for the life of this table.
    affine_mapped: BTreeMap<usize, BTreeMap<Proposition, Option<Proposition>>>,
    /// `integer_affine_truth_bounds` outcomes per form, keyed by the same
    /// `Rc` address.
    affine_truth: BTreeMap<usize, Option<Rc<Vec<Proposition>>>>,
}

impl Tables {
    fn new(semantic_axioms: &[Proposition]) -> Self {
        let mut by_input = BTreeMap::<ScalarTerm, Vec<usize>>::new();
        candidates::visit(semantic_axioms, |index, input| {
            let candidates = by_input.entry(input.clone()).or_default();
            if candidates.last() != Some(&index) {
                candidates.push(index);
            }
        });
        let mut by_output = BTreeMap::<ScalarTerm, Vec<usize>>::new();
        for (index, axiom) in semantic_axioms.iter().enumerate() {
            let Proposition::Equal(left, right) = axiom else {
                continue;
            };
            for output in [left, right]
                .into_iter()
                .filter(|term| matches!(term, ScalarTerm::Value { .. }))
            {
                let definitions = by_output.entry(output.clone()).or_default();
                if definitions.last() != Some(&index) {
                    definitions.push(index);
                }
            }
        }
        let mut cast_roots = Vec::new();
        let mut cast_definitions = BTreeMap::new();
        for (index, axiom) in semantic_axioms.iter().enumerate() {
            let Proposition::Equal(output, ScalarTerm::IntegerExactCast { operand, .. }) = axiom
            else {
                continue;
            };
            cast_roots.push(output.clone());
            cast_definitions
                .entry(output.clone())
                .and_modify(|definition| *definition = None)
                .or_insert_with(|| Some((index, operand.as_ref().clone())));
        }
        Self {
            by_input,
            by_output,
            cast_roots: Rc::from(cast_roots),
            cast_definitions,
            words_by_root: BTreeMap::new(),
            words_by_root_and_target: BTreeMap::new(),
            literal_axioms_by_witness: BTreeMap::new(),
            checked_words: BTreeMap::new(),
            affine_forms: BTreeMap::new(),
            affine_mapped: BTreeMap::new(),
            affine_truth: BTreeMap::new(),
        }
    }

    /// The `check_integer_affine_witness` outcome for one witness, replayed
    /// once per distinct word under this roster rather than once per
    /// `(goal, root bound)` candidate that offers it.
    fn affine_form(
        &mut self,
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        witness: &IntegerAffineWitness,
    ) -> Option<Rc<CheckedIntegerAffineForm>> {
        let key = (
            witness.definition_axioms.clone(),
            witness.literal_axioms.clone(),
        );
        if let Some(form) = self
            .affine_forms
            .get(&witness.root)
            .and_then(|m| m.get(&witness.target))
            .and_then(|m| m.get(&key))
        {
            return form.clone();
        }
        let form = check_integer_affine_witness(context, semantic_axioms, witness)
            .ok()
            .map(Rc::new);
        self.affine_forms
            .entry(witness.root.clone())
            .or_default()
            .entry(witness.target.clone())
            .or_default()
            .insert(key, form.clone());
        form
    }

    /// `map_integer_affine_bound` per `(form, root bound)`, keyed by the
    /// `Rc` address `affine_forms` keeps alive — callers must pass the `Rc`
    /// handed out by `affine_form`, not a fresh clone of an equal form.
    fn affine_mapped(
        &mut self,
        form: &Rc<CheckedIntegerAffineForm>,
        root_bound: &Proposition,
    ) -> Option<Proposition> {
        let key = Rc::as_ptr(form) as usize;
        if let Some(mapped) = self.affine_mapped.get(&key).and_then(|m| m.get(root_bound)) {
            return mapped.clone();
        }
        let mapped = map_integer_affine_bound(form, root_bound).ok();
        self.affine_mapped
            .entry(key)
            .or_default()
            .insert(root_bound.clone(), mapped.clone());
        mapped
    }

    /// `integer_affine_truth_bounds` per form, keyed by the same `Rc`
    /// address.
    fn affine_truth(
        &mut self,
        form: &Rc<CheckedIntegerAffineForm>,
    ) -> Option<Rc<Vec<Proposition>>> {
        let key = Rc::as_ptr(form) as usize;
        if let Some(bounds) = self.affine_truth.get(&key) {
            return bounds.clone();
        }
        let bounds = integer_affine_truth_bounds(form).ok().map(Rc::new);
        self.affine_truth.insert(key, bounds.clone());
        bounds
    }
}

/// Goal-keyed proof answers under one fixed scope. A stored proof cites this
/// scope's assumptions and semantic axioms by position, so the whole store is
/// valid only under the `(context, assumptions, semantic_axioms)` that built
/// it — `shared_proofs` matches all three by full content before handing one
/// out. `None` entries are the producer's own memo of a computed negative
/// and the in-progress marker that turns a hypothetical cycle into `None`
/// rather than recursion.
struct ScopeProofs {
    affine: BTreeMap<Proposition, Option<ProofNode>>,
    wrapping: BTreeMap<Proposition, Option<ProofNode>>,
    bound: BTreeMap<Proposition, Option<ProofNode>>,
    endpoint: BTreeMap<Proposition, Option<ProofNode>>,
    /// Whole `build_with_definitions` answers per goal under this fixed
    /// scope: conjunction and disjunction splits re-ask goals the direct
    /// path already settled, so the composed answer is memoized once.
    cascade: BTreeMap<Proposition, Option<ProofNode>>,
    /// `build_without_implications` answers per `(goal, machine parameter
    /// roster)` — the `ordinary` leaf inside every implication search.
    /// Sibling searches under this exact scope share one derivation instead
    /// of re-running the cascade for every premise they discharge.
    selection: BTreeMap<(Proposition, BTreeSet<ValueId>), Option<ProofNode>>,
    /// Whole `integer_selection::build_with_machine_parameters` answers per
    /// `(goal, machine parameter roster)`: cast completion, value transport,
    /// and predicate conversion re-enter full selection for derived goals
    /// from inside a running search, and without this memo each re-entry
    /// pays a fresh bounded search over the same scope. The parameter roster
    /// is part of the key because a produced proof may cite parameter-custody
    /// roots the kernel checks against exactly this set.
    build: BTreeMap<(Proposition, BTreeSet<ValueId>), Option<ProofNode>>,
    /// Same whole-selection memo for `prove_relaxed_with_machine_parameters`,
    /// whose leaf set adds `derived::prove`, so its answers are not
    /// interchangeable with the canonical `build` answers.
    relaxed: BTreeMap<(Proposition, BTreeSet<ValueId>), Option<ProofNode>>,
}

thread_local! {
    /// Shared `Tables` resolved by full `(context, semantic_axioms)` content,
    /// most-recently-used first — the same content-matched discipline the
    /// exact roster cache uses. Bounded so a long compilation keeps a
    /// bounded working set rather than every roster it ever saw.
    static TABLES: RefCell<Vec<(PropositionContext, Vec<Proposition>, Rc<RefCell<Tables>>)>> =
        const { RefCell::new(Vec::new()) };

    /// Shared `ScopeProofs` resolved by full `(context, assumptions,
    /// semantic_axioms)` content — the same MRU discipline as `TABLES`, one
    /// scope finer because produced proofs cite the assumption roster. The
    /// implication search revisits a handful of assumption scopes per
    /// obligation while case analysis pops back to the enclosing one, so a
    /// small window covers the working set.
    static PROOFS: RefCell<
        Vec<(
            PropositionContext,
            Vec<Proposition>,
            Vec<Proposition>,
            Rc<RefCell<ScopeProofs>>,
        )>,
    > = const { RefCell::new(Vec::new()) };
}

/// Distinct `(context, axioms)` rosters whose shared tables stay live at
/// once. Producers revisit a handful of rosters per obligation, so a small
/// MRU window covers the working set.
const MAXIMUM_TABLES: usize = 8;

/// Distinct scopes whose produced-goal stores stay live at once. Case and
/// implication branches nest a few assumption extensions inside one
/// obligation; matching stays proportional to the live search depth.
const MAXIMUM_SCOPE_PROOFS: usize = 32;

/// The most-recently-used shared tables for `(context, semantic_axioms)`,
/// building them on first sight. Matching is always by full content.
fn shared_tables(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
) -> Rc<RefCell<Tables>> {
    TABLES.with(|cell| {
        let mut tables = cell.borrow_mut();
        let index = match tables.iter().rposition(|(seen_context, seen_axioms, _)| {
            seen_axioms == semantic_axioms && seen_context == context
        }) {
            Some(index) => index,
            None => {
                tables.push((
                    context.clone(),
                    semantic_axioms.to_vec(),
                    Rc::new(RefCell::new(Tables::new(semantic_axioms))),
                ));
                tables.len() - 1
            }
        };
        let entry = tables.remove(index);
        let shared = entry.2.clone();
        tables.push(entry);
        if tables.len() > MAXIMUM_TABLES {
            tables.remove(0);
        }
        shared
    })
}

/// The most-recently-used produced-goal store for `(context, assumptions,
/// semantic_axioms)`, building it on first sight. Matching is always by full
/// content: a memoized proof cites this scope's facts by position, and a
/// content-equal scope replays it unchanged.
fn shared_proofs(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Rc<RefCell<ScopeProofs>> {
    PROOFS.with(|cell| {
        let mut proofs = cell.borrow_mut();
        let index =
            match proofs
                .iter()
                .rposition(|(seen_context, seen_assumptions, seen_axioms, _)| {
                    // Slice length is the cheap discriminator; the whole
                    // proposition context compares last and only on real
                    // candidates.
                    seen_axioms == semantic_axioms
                        && seen_assumptions == assumptions
                        && seen_context == context
                }) {
                Some(index) => index,
                None => {
                    proofs.push((
                        context.clone(),
                        assumptions.to_vec(),
                        semantic_axioms.to_vec(),
                        Rc::new(RefCell::new(ScopeProofs {
                            affine: BTreeMap::new(),
                            wrapping: BTreeMap::new(),
                            bound: BTreeMap::new(),
                            endpoint: BTreeMap::new(),
                            cascade: BTreeMap::new(),
                            selection: BTreeMap::new(),
                            build: BTreeMap::new(),
                            relaxed: BTreeMap::new(),
                        })),
                    ));
                    proofs.len() - 1
                }
            };
        let entry = proofs.remove(index);
        let shared = entry.3.clone();
        proofs.push(entry);
        if proofs.len() > MAXIMUM_SCOPE_PROOFS {
            proofs.remove(0);
        }
        shared
    })
}

/// The `check_word` outcome for one `(bound, root, target, word)`: the
/// kernel-checked affine form, the evidence propositions the mapped bound
/// requires, and the literal axioms the witness names — everything needed to
/// rebuild the caller's witness and mapped proof.
pub(crate) struct CheckedWord {
    pub(in crate::proofs::nonzero_divisor_certificate) form: CheckedIntegerAffineForm,
    pub(in crate::proofs::nonzero_divisor_certificate) evidence: Vec<Proposition>,
    pub(in crate::proofs::nonzero_divisor_certificate) literal_axioms: Vec<Option<usize>>,
}

impl DefinitionIndex {
    /// An index over this scope: the caller's `assumptions` are part of the
    /// identity because produced proofs cite them by position, so the shared
    /// produced-goal store only ever hands back entries built under this
    /// exact scope.
    pub(crate) fn new(
        context: &PropositionContext,
        assumptions: &[Proposition],
        semantic_axioms: &[Proposition],
    ) -> Self {
        Self {
            tables: shared_tables(context, semantic_axioms),
            proofs: shared_proofs(context, assumptions, semantic_axioms),
        }
    }

    pub(in crate::proofs::nonzero_divisor_certificate::affine_custody) fn candidates_from(
        &self,
        input: &ScalarTerm,
        start: usize,
    ) -> impl Iterator<Item = usize> {
        // A chain term resumes forward through a definition that uses it as
        // an operand, or backward through a wrapping-add row that defines it;
        // merge both source-ordered lists so the frontier sees every
        // candidate. The kernel replay stays authoritative for each prefix.
        let tables = self.tables.borrow();
        let inputs = tables.by_input.get(input).map(Vec::as_slice).unwrap_or(&[]);
        let outputs = tables
            .by_output
            .get(input)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut merged = inputs
            .iter()
            .chain(outputs)
            .copied()
            .filter(|&index| index >= start)
            .collect::<Vec<_>>();
        merged.sort_unstable();
        merged.dedup();
        merged.into_iter()
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn output_definitions_before(
        &self,
        output: &ScalarTerm,
        before: usize,
    ) -> Vec<usize> {
        let tables = self.tables.borrow();
        let definitions = tables
            .by_output
            .get(output)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let end = definitions.partition_point(|&index| index < before);
        definitions[..end].to_vec()
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn cast_roots(&self) -> Rc<[ScalarTerm]> {
        self.tables.borrow().cast_roots.clone()
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn cast_spine(
        &self,
        target: &ScalarTerm,
    ) -> Option<(ScalarTerm, Vec<usize>)> {
        let tables = self.tables.borrow();
        let mut current = target.clone();
        let mut reversed = Vec::new();
        while let Some(definition) = tables.cast_definitions.get(&current) {
            if reversed.len() >= tables.cast_definitions.len() {
                return None;
            }
            let (index, operand) = definition.as_ref()?;
            if reversed.contains(index) {
                return None;
            }
            reversed.push(*index);
            current = operand.clone();
        }
        reversed.reverse();
        (!reversed.is_empty() && reversed.windows(2).all(|pair| pair[0] < pair[1]))
            .then_some((current, reversed))
    }

    pub(in crate::proofs::nonzero_divisor_certificate::affine_custody) fn cached_words(
        &self,
        root: &ScalarTerm,
    ) -> Option<Rc<[Vec<usize>]>> {
        self.tables.borrow().words_by_root.get(root).cloned()
    }

    pub(in crate::proofs::nonzero_divisor_certificate::affine_custody) fn cache_words(
        &self,
        root: &ScalarTerm,
        words: Rc<[Vec<usize>]>,
    ) {
        self.tables
            .borrow_mut()
            .words_by_root
            .insert(root.clone(), words);
    }

    pub(in crate::proofs::nonzero_divisor_certificate::affine_custody) fn cached_words_to_target(
        &self,
        root: &ScalarTerm,
        target: &ScalarTerm,
    ) -> Option<Rc<[Vec<usize>]>> {
        self.tables
            .borrow()
            .words_by_root_and_target
            .get(&(root.clone(), target.clone()))
            .cloned()
    }

    pub(in crate::proofs::nonzero_divisor_certificate::affine_custody) fn cache_words_to_target(
        &self,
        root: &ScalarTerm,
        target: &ScalarTerm,
        words: Rc<[Vec<usize>]>,
    ) {
        self.tables
            .borrow_mut()
            .words_by_root_and_target
            .insert((root.clone(), target.clone()), words);
    }

    pub(in crate::proofs::nonzero_divisor_certificate::affine_custody) fn cached_literal_axioms(
        &self,
        root: &ScalarTerm,
        definition_axioms: &[usize],
        target: &ScalarTerm,
    ) -> Option<Option<Vec<Option<usize>>>> {
        self.tables
            .borrow()
            .literal_axioms_by_witness
            .get(&(root.clone(), definition_axioms.to_vec(), target.clone()))
            .cloned()
    }

    pub(in crate::proofs::nonzero_divisor_certificate::affine_custody) fn cache_literal_axioms(
        &self,
        root: &ScalarTerm,
        definition_axioms: &[usize],
        target: &ScalarTerm,
        literal_axioms: Option<Vec<Option<usize>>>,
    ) {
        self.tables.borrow_mut().literal_axioms_by_witness.insert(
            (root.clone(), definition_axioms.to_vec(), target.clone()),
            literal_axioms,
        );
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn cached_affine_proof(
        &self,
        goal: &Proposition,
    ) -> Option<Option<ProofNode>> {
        self.proofs.borrow().affine.get(goal).cloned()
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn begin_affine_proof(
        &mut self,
        goal: &Proposition,
    ) {
        self.proofs.borrow_mut().affine.insert(goal.clone(), None);
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn cache_affine_proof(
        &mut self,
        goal: &Proposition,
        proof: Option<ProofNode>,
    ) {
        self.proofs.borrow_mut().affine.insert(goal.clone(), proof);
    }

    // The wrapping leg shares the candidate index but not the affine proof
    // cache: `bound::prove` runs the exact affine selection first, and a
    // negative entry there must not suppress the wrapping conjunction leg for
    // the same goal. The in-progress marker still breaks re-entrant cycles
    // reached through `prove_candidate_endpoint` -> `shift::prove` ->
    // `bound::prove` -> `wrapping::prove` for the same goal.
    pub(in crate::proofs::nonzero_divisor_certificate) fn cached_wrapping_proof(
        &self,
        goal: &Proposition,
    ) -> Option<Option<ProofNode>> {
        self.proofs.borrow().wrapping.get(goal).cloned()
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn begin_wrapping_proof(
        &mut self,
        goal: &Proposition,
    ) {
        self.proofs.borrow_mut().wrapping.insert(goal.clone(), None);
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn cache_wrapping_proof(
        &mut self,
        goal: &Proposition,
        proof: Option<ProofNode>,
    ) {
        self.proofs
            .borrow_mut()
            .wrapping
            .insert(goal.clone(), proof);
    }

    /// The bound cascade answers one `(goal, scope)` question; under this
    /// index the scope is fixed, so the goal alone keys the memo. The
    /// in-progress marker breaks re-entrant cycles reached through
    /// `wrapping::prove` -> `map_checked_word` -> `prove_candidate_endpoint`
    /// -> `shift::prove` -> `bound::prove` for the same goal.
    pub(in crate::proofs::nonzero_divisor_certificate) fn cached_bound_proof(
        &self,
        goal: &Proposition,
    ) -> Option<Option<ProofNode>> {
        self.proofs.borrow().bound.get(goal).cloned()
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn begin_bound_proof(
        &mut self,
        goal: &Proposition,
    ) {
        self.proofs.borrow_mut().bound.insert(goal.clone(), None);
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn cache_bound_proof(
        &mut self,
        goal: &Proposition,
        proof: Option<ProofNode>,
    ) {
        self.proofs.borrow_mut().bound.insert(goal.clone(), proof);
    }

    /// `build_with_definitions` answers per goal — the top of the producer
    /// cascade, memoized so a conjunction/disjunction split never re-runs
    /// the whole cascade for a goal already settled under this scope.
    pub(in crate::proofs::nonzero_divisor_certificate) fn cached_cascade_proof(
        &self,
        goal: &Proposition,
    ) -> Option<Option<ProofNode>> {
        self.proofs.borrow().cascade.get(goal).cloned()
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn begin_cascade_proof(
        &mut self,
        goal: &Proposition,
    ) {
        self.proofs.borrow_mut().cascade.insert(goal.clone(), None);
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn cache_cascade_proof(
        &mut self,
        goal: &Proposition,
        proof: Option<ProofNode>,
    ) {
        self.proofs.borrow_mut().cascade.insert(goal.clone(), proof);
    }

    /// The relaxed candidate-endpoint cascade has its own producer order, so
    /// it keeps its own memo rather than sharing `bound_proofs`.
    pub(in crate::proofs::nonzero_divisor_certificate) fn cached_endpoint_proof(
        &self,
        goal: &Proposition,
    ) -> Option<Option<ProofNode>> {
        self.proofs.borrow().endpoint.get(goal).cloned()
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn begin_endpoint_proof(
        &mut self,
        goal: &Proposition,
    ) {
        self.proofs.borrow_mut().endpoint.insert(goal.clone(), None);
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn cache_endpoint_proof(
        &mut self,
        goal: &Proposition,
        proof: Option<ProofNode>,
    ) {
        self.proofs
            .borrow_mut()
            .endpoint
            .insert(goal.clone(), proof);
    }

    /// `build_without_implications` answers per `(goal, machine parameter
    /// roster)` — the `ordinary` leaf inside every implication search, shared
    /// across sibling searches under this exact scope. The in-progress marker
    /// breaks re-entrant cycles reached through `case_analysis` branches that
    /// re-enter selection for a goal already being computed.
    pub(in crate::proofs::nonzero_divisor_certificate) fn cached_selection_proof(
        &self,
        goal: &Proposition,
        machine_parameter_values: &BTreeSet<ValueId>,
    ) -> Option<Option<ProofNode>> {
        self.proofs
            .borrow()
            .selection
            .get(&(goal.clone(), machine_parameter_values.clone()))
            .cloned()
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn begin_selection_proof(
        &mut self,
        goal: &Proposition,
        machine_parameter_values: &BTreeSet<ValueId>,
    ) {
        self.proofs
            .borrow_mut()
            .selection
            .insert((goal.clone(), machine_parameter_values.clone()), None);
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn cache_selection_proof(
        &mut self,
        goal: &Proposition,
        machine_parameter_values: &BTreeSet<ValueId>,
        proof: Option<ProofNode>,
    ) {
        self.proofs
            .borrow_mut()
            .selection
            .insert((goal.clone(), machine_parameter_values.clone()), proof);
    }

    /// Whole `integer_selection::build_with_machine_parameters` answers per
    /// `(goal, machine parameter roster)`: cast completion, value transport,
    /// and predicate conversion re-enter full selection for derived goals
    /// from inside a running search. The in-progress marker turns a cyclic
    /// re-entry for the same goal into `None` rather than recursion.
    pub(in crate::proofs::nonzero_divisor_certificate) fn cached_build_proof(
        &self,
        goal: &Proposition,
        machine_parameter_values: &BTreeSet<ValueId>,
    ) -> Option<Option<ProofNode>> {
        self.proofs
            .borrow()
            .build
            .get(&(goal.clone(), machine_parameter_values.clone()))
            .cloned()
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn begin_build_proof(
        &mut self,
        goal: &Proposition,
        machine_parameter_values: &BTreeSet<ValueId>,
    ) {
        self.proofs
            .borrow_mut()
            .build
            .insert((goal.clone(), machine_parameter_values.clone()), None);
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn cache_build_proof(
        &mut self,
        goal: &Proposition,
        machine_parameter_values: &BTreeSet<ValueId>,
        proof: Option<ProofNode>,
    ) {
        self.proofs
            .borrow_mut()
            .build
            .insert((goal.clone(), machine_parameter_values.clone()), proof);
    }

    /// Same whole-selection memo for the relaxed selection whose leaf set
    /// adds `derived::prove`; its answers are not interchangeable with the
    /// canonical `build` answers, so it keeps a separate map.
    pub(in crate::proofs::nonzero_divisor_certificate) fn cached_relaxed_proof(
        &self,
        goal: &Proposition,
        machine_parameter_values: &BTreeSet<ValueId>,
    ) -> Option<Option<ProofNode>> {
        self.proofs
            .borrow()
            .relaxed
            .get(&(goal.clone(), machine_parameter_values.clone()))
            .cloned()
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn begin_relaxed_proof(
        &mut self,
        goal: &Proposition,
        machine_parameter_values: &BTreeSet<ValueId>,
    ) {
        self.proofs
            .borrow_mut()
            .relaxed
            .insert((goal.clone(), machine_parameter_values.clone()), None);
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn cache_relaxed_proof(
        &mut self,
        goal: &Proposition,
        machine_parameter_values: &BTreeSet<ValueId>,
        proof: Option<ProofNode>,
    ) {
        self.proofs
            .borrow_mut()
            .relaxed
            .insert((goal.clone(), machine_parameter_values.clone()), proof);
    }

    /// One `check_word` outcome per `(bound, root, target, word)` — the
    /// contradiction leg pool and the goal-directed path meet the same
    /// candidates, so the witness check computes once per word under this
    /// roster. `None` records a checked-out failure so it does not recompute.
    pub(in crate::proofs::nonzero_divisor_certificate) fn cached_checked_word(
        &self,
        bound: &Proposition,
        root: &ScalarTerm,
        target: &ScalarTerm,
        definition_axioms: &[usize],
    ) -> Option<Option<Rc<CheckedWord>>> {
        self.tables
            .borrow()
            .checked_words
            .get(&(
                bound.clone(),
                root.clone(),
                target.clone(),
                definition_axioms.to_vec(),
            ))
            .cloned()
    }

    pub(in crate::proofs::nonzero_divisor_certificate) fn cache_checked_word(
        &self,
        bound: &Proposition,
        root: &ScalarTerm,
        target: &ScalarTerm,
        definition_axioms: &[usize],
        checked: Option<Rc<CheckedWord>>,
    ) {
        self.tables.borrow_mut().checked_words.insert(
            (
                bound.clone(),
                root.clone(),
                target.clone(),
                definition_axioms.to_vec(),
            ),
            checked,
        );
    }

    /// The `check_integer_affine_witness` outcome for this witness under the
    /// shared `(context, semantic_axioms)` tables — one kernel replay per
    /// distinct word, not per candidate that offers it.
    pub(in crate::proofs::nonzero_divisor_certificate) fn affine_form(
        &self,
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        witness: &IntegerAffineWitness,
    ) -> Option<Rc<CheckedIntegerAffineForm>> {
        self.tables
            .borrow_mut()
            .affine_form(context, semantic_axioms, witness)
    }

    /// The `map_integer_affine_bound` outcome for this `(form, root bound)`.
    /// `form` must be the `Rc` `affine_form` handed out: the memo keys on
    /// the allocation the shared table keeps alive.
    pub(in crate::proofs::nonzero_divisor_certificate) fn affine_mapped(
        &self,
        form: &Rc<CheckedIntegerAffineForm>,
        root_bound: &Proposition,
    ) -> Option<Proposition> {
        self.tables.borrow_mut().affine_mapped(form, root_bound)
    }

    /// The `integer_affine_truth_bounds` outcome for this form.
    pub(in crate::proofs::nonzero_divisor_certificate) fn affine_truth(
        &self,
        form: &Rc<CheckedIntegerAffineForm>,
    ) -> Option<Rc<Vec<Proposition>>> {
        self.tables.borrow_mut().affine_truth(form)
    }
}
