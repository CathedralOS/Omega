//! Deterministic control-flow edge fact reconstruction.

use std::collections::BTreeMap;

use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ValueId};

use super::super::substitution::{
    AxiomRoster, AxiomRosterMut, FactIndex, proposition_mentions_substituted_value,
    substitute_proposition_places, substitute_proposition_values,
};

mod conditions;
mod discrete;
mod transport;
pub(super) use conditions::condition_fact;
pub(super) use transport::RewrittenSuccessorFact;

/// Bind one successor edge's parameters and restate the established facts
/// that mention its arguments.
///
/// The pushed `parameter = argument` equalities are licensed premise
/// introductions under `fact:successor-parameter-binding`. Each rewritten
/// established fact is discharged by the fixed-shape transport certificate in
/// [`transport`] before it joins the roster; the returned classification marks
/// certified emissions for `fact:successor-path-transport` and leaves rejected
/// certificates under the binding row's license. Production callers discard
/// the classification; only tests read it back.
pub(super) fn bind_successor_axioms<M: AxiomRosterMut>(
    axioms: &mut M,
    target_block: &terminal_psi::Block,
    arguments: &[ValueId],
    value_term: &impl Fn(ValueId) -> ScalarTerm,
    proposition_context: &PropositionContext,
    rewrite_path_facts: bool,
) -> Vec<RewrittenSuccessorFact> {
    let substitutions = target_block
        .parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (*argument, value_term(parameter.id)))
        .collect::<BTreeMap<_, _>>();
    // Facts established before this edge's own pushes live below this
    // position; iterating positions reads them without cloning the roster.
    let established_count = axioms.roster_len();
    // Fingerprint the roster once: every duplicate check and equality
    // citation below is a bucket lookup instead of a roster scan.
    let mut fact_index = FactIndex::new(axioms);
    // Emit edge equalities before rewritten path facts so independently
    // reconstructed axiom indexes are deterministic. A roster never stores
    // one equation in both operand orders — two swapped `parameter =
    // argument` bindings are the same fact, and the pair would register as a
    // cyclic definition in the denotation checker — so a binding whose
    // symmetric twin already stands is not pushed a second time.
    for (parameter, argument) in target_block.parameters.iter().zip(arguments) {
        fact_index.push_unique(
            axioms,
            Proposition::Equal(value_term(parameter.id), value_term(*argument)),
        );
    }
    let mut rewritten_facts = Vec::new();
    if rewrite_path_facts {
        // Each rewrite's transport certificate cites this edge's binding
        // equalities by roster position; the symmetric-twin skip above can
        // leave a pair without its own row, and whichever direction already
        // stands carries the same equation.
        let equality_indices = target_block
            .parameters
            .iter()
            .zip(arguments)
            .filter_map(|(parameter, argument)| {
                fact_index.equation_position(
                    axioms,
                    &value_term(parameter.id),
                    &value_term(*argument),
                )
            })
            .collect::<Vec<_>>();
        for index in 0..established_count {
            if !proposition_mentions_substituted_value(axioms.roster_fact(index), &substitutions) {
                continue;
            }
            let rewritten =
                substitute_proposition_values(axioms.roster_fact(index), &substitutions);
            if fact_index.has_fact(axioms, &rewritten) {
                continue;
            }
            let certified = transport::successor_rewrite_certified(
                proposition_context,
                axioms,
                index,
                &equality_indices,
                &rewritten,
            );
            fact_index.record(axioms.roster_len(), &rewritten);
            axioms.push_fact(rewritten.clone());
            rewritten_facts.push(RewrittenSuccessorFact {
                proposition: rewritten,
                certified,
            });
        }
    }
    rewritten_facts
}

/// Restate established facts whose place roots an edge's structural arguments
/// name, under the target block's bound formals. The edge's structural binding
/// row is the licensed premise these emissions ride on; no fixed-shape
/// transport certificate covers a place rewrite yet, so every emission is
/// reported uncertified under the binding row's license.
pub(super) fn bind_successor_structural_axioms<M: AxiomRosterMut>(
    axioms: &mut M,
    target_block: &terminal_psi::Block,
    structural_arguments: &[terminal_psi::StructuralArgument],
    rewrite_path_facts: bool,
) -> Vec<RewrittenSuccessorFact> {
    if !rewrite_path_facts || target_block.structural_parameters.is_empty() {
        return Vec::new();
    }
    // A projected actual names a value beneath its root, so facts rooted at
    // the declared place do not restate against the bound formal. Only whole
    // actuals join the substitution map.
    let substitutions = target_block
        .structural_parameters
        .iter()
        .zip(structural_arguments)
        .filter(|(_, argument)| argument.path.is_empty())
        .map(|(parameter, argument)| (argument.place, parameter.place))
        .collect::<BTreeMap<_, _>>();
    if substitutions.is_empty() {
        return Vec::new();
    }
    let established_count = axioms.roster_len();
    let mut fact_index = FactIndex::new(axioms);
    let mut rewritten_facts = Vec::new();
    for index in 0..established_count {
        let rewritten = substitute_proposition_places(axioms.roster_fact(index), &substitutions);
        if rewritten != *axioms.roster_fact(index)
            && fact_index.position_of(axioms, &rewritten).is_none()
        {
            fact_index.record(axioms.roster_len(), &rewritten);
            axioms.push_fact(rewritten.clone());
            rewritten_facts.push(RewrittenSuccessorFact {
                proposition: rewritten,
                certified: false,
            });
        }
    }
    rewritten_facts
}

/// Append one already-established fact to a successor's roster together with
/// its restated form.
///
/// `proposition` itself joins by `push_unique` under whichever row licensed
/// its own introduction. The substituted copy follows it only when absent,
/// discharged by the same fixed-shape transport certificate
/// `bind_successor_axioms` uses; the returned classification marks whether the
/// certificate checker accepted that rewrite.
pub(super) fn append_successor_fact<M: AxiomRosterMut>(
    axioms: &mut M,
    proposition: &Proposition,
    target_block: &terminal_psi::Block,
    arguments: &[ValueId],
    value_term: &impl Fn(ValueId) -> ScalarTerm,
    proposition_context: &PropositionContext,
) -> Vec<RewrittenSuccessorFact> {
    let substitutions = target_block
        .parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (*argument, value_term(parameter.id)))
        .collect::<BTreeMap<_, _>>();
    let mut fact_index = FactIndex::new(axioms);
    fact_index.push_unique(axioms, proposition.clone());
    let rewritten = substitute_proposition_values(proposition, &substitutions);
    append_discrete_fact(axioms, &mut fact_index, proposition);
    append_discrete_fact(axioms, &mut fact_index, &rewritten);
    if fact_index.has_fact(axioms, &rewritten) {
        return Vec::new();
    }
    // The source fact sits at its already-pushed roster position; this edge's
    // binding equalities are the `parameter = argument` axioms the successor
    // binding pushed before any rewrite.
    let source_index = fact_index.position_of(axioms, proposition);
    let equality_indices = target_block
        .parameters
        .iter()
        .zip(arguments)
        .filter_map(|(parameter, argument)| {
            fact_index.equation_position(axioms, &value_term(parameter.id), &value_term(*argument))
        })
        .collect::<Vec<_>>();
    let certified = source_index.is_some_and(|source_index| {
        transport::successor_rewrite_certified(
            proposition_context,
            axioms,
            source_index,
            &equality_indices,
            &rewritten,
        )
    });
    let fact = RewrittenSuccessorFact {
        proposition: rewritten.clone(),
        certified,
    };
    fact_index.record(axioms.roster_len(), &rewritten);
    axioms.push_fact(rewritten);
    vec![fact]
}

/// The roster rows a fixed-shape transport certificate cites, as their own
/// roster in original order.
///
/// Both transport certificates cite only `SemanticAxiom` rows and the
/// `ValueEqualityTransport` relation consults only its cited equations, so
/// checking one against the cited rows re-decides the same derivation from a
/// subset of the premises the full roster offers. Handing the checker the
/// whole roster instead re-validates and re-denotes every uncited row for each
/// emitted fact, which grows with the roster rather than the certificate.
struct CitedRoster {
    rows: Vec<Proposition>,
    positions: BTreeMap<usize, usize>,
}

impl CitedRoster {
    /// `None` when a cited index is outside the roster.
    fn new(
        axioms: &(impl AxiomRoster + ?Sized),
        cited: impl IntoIterator<Item = usize>,
    ) -> Option<Self> {
        let mut indices = cited.into_iter().collect::<Vec<_>>();
        indices.sort_unstable();
        indices.dedup();
        let rows = indices
            .iter()
            .map(|&index| (index < axioms.roster_len()).then(|| axioms.roster_fact(index).clone()))
            .collect::<Option<Vec<_>>>()?;
        let positions = indices
            .into_iter()
            .enumerate()
            .map(|(position, index)| (index, position))
            .collect();
        Some(Self { rows, positions })
    }

    /// The certificate citation of original roster row `index`.
    fn citation(&self, index: usize) -> proof_admission::ProofNode {
        let position = self.positions[&index];
        proof_admission::ProofNode {
            conclusion: self.rows[position].clone(),
            rule: proof_admission::ProofRule::SemanticAxiom { index: position },
        }
    }
}

fn append_discrete_fact<M: AxiomRosterMut>(
    propositions: &mut M,
    fact_index: &mut FactIndex,
    proposition: &Proposition,
) {
    if let Some(discrete) = discrete::strict_bound(proposition) {
        fact_index.push_unique(propositions, discrete);
    }
    if let Some(bounds) = discrete::strict_carrier_bounds(proposition) {
        for bound in bounds {
            fact_index.push_unique(propositions, bound);
        }
    }
}
