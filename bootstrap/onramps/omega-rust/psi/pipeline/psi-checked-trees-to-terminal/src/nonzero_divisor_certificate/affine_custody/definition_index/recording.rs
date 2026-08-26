//! Ordered affine-definition index recording for certificate production.

use std::collections::BTreeMap;
use std::rc::Rc;

use psi_core::{Proposition, ScalarTerm};
use psi_proof_admission::ProofNode;

use super::candidates;

/// Source-ordered semantic rows that may extend one exact affine value chain.
/// This selects candidates only; the kernel remains authoritative for every
/// prefix and completed proof.
pub(crate) struct DefinitionIndex {
    by_input: BTreeMap<ScalarTerm, Vec<usize>>,
    cast_roots: Vec<ScalarTerm>,
    cast_definitions: BTreeMap<ScalarTerm, Option<(usize, ScalarTerm)>>,
    words_by_root: BTreeMap<ScalarTerm, Rc<[Vec<usize>]>>,
    words_by_root_and_target: BTreeMap<(ScalarTerm, ScalarTerm), Rc<[Vec<usize>]>>,
    literal_axioms_by_witness:
        BTreeMap<(ScalarTerm, Vec<usize>, ScalarTerm), Option<Vec<Option<usize>>>>,
    affine_proofs: BTreeMap<Proposition, Option<ProofNode>>,
}

impl DefinitionIndex {
    pub(crate) fn new(semantic_axioms: &[Proposition]) -> Self {
        let mut by_input = BTreeMap::<ScalarTerm, Vec<usize>>::new();
        candidates::visit(semantic_axioms, |index, input| {
            let candidates = by_input.entry(input.clone()).or_default();
            if candidates.last() != Some(&index) {
                candidates.push(index);
            }
        });
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
            cast_roots,
            cast_definitions,
            words_by_root: BTreeMap::new(),
            words_by_root_and_target: BTreeMap::new(),
            literal_axioms_by_witness: BTreeMap::new(),
            affine_proofs: BTreeMap::new(),
        }
    }

    pub(in crate::nonzero_divisor_certificate::affine_custody) fn candidates_from(
        &self,
        input: &ScalarTerm,
        start: usize,
    ) -> impl Iterator<Item = usize> + '_ {
        let candidates = self.by_input.get(input).map(Vec::as_slice).unwrap_or(&[]);
        let first = candidates.partition_point(|&index| index < start);
        candidates[first..].iter().copied()
    }

    pub(in crate::nonzero_divisor_certificate) fn cast_roots(
        &self,
    ) -> impl Iterator<Item = &ScalarTerm> {
        self.cast_roots.iter()
    }

    pub(in crate::nonzero_divisor_certificate) fn cast_spine(
        &self,
        target: &ScalarTerm,
    ) -> Option<(ScalarTerm, Vec<usize>)> {
        let mut current = target.clone();
        let mut reversed = Vec::new();
        loop {
            let Some(definition) = self.cast_definitions.get(&current) else {
                break;
            };
            if reversed.len() >= self.cast_definitions.len() {
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

    pub(in crate::nonzero_divisor_certificate::affine_custody) fn cached_words(
        &self,
        root: &ScalarTerm,
    ) -> Option<Rc<[Vec<usize>]>> {
        self.words_by_root.get(root).cloned()
    }

    pub(in crate::nonzero_divisor_certificate::affine_custody) fn cache_words(
        &mut self,
        root: &ScalarTerm,
        words: Rc<[Vec<usize>]>,
    ) {
        self.words_by_root.insert(root.clone(), words);
    }

    pub(in crate::nonzero_divisor_certificate::affine_custody) fn cached_words_to_target(
        &self,
        root: &ScalarTerm,
        target: &ScalarTerm,
    ) -> Option<Rc<[Vec<usize>]>> {
        self.words_by_root_and_target
            .get(&(root.clone(), target.clone()))
            .cloned()
    }

    pub(in crate::nonzero_divisor_certificate::affine_custody) fn cache_words_to_target(
        &mut self,
        root: &ScalarTerm,
        target: &ScalarTerm,
        words: Rc<[Vec<usize>]>,
    ) {
        self.words_by_root_and_target
            .insert((root.clone(), target.clone()), words);
    }

    pub(in crate::nonzero_divisor_certificate::affine_custody) fn cached_literal_axioms(
        &self,
        root: &ScalarTerm,
        definition_axioms: &[usize],
        target: &ScalarTerm,
    ) -> Option<Option<Vec<Option<usize>>>> {
        self.literal_axioms_by_witness
            .get(&(root.clone(), definition_axioms.to_vec(), target.clone()))
            .cloned()
    }

    pub(in crate::nonzero_divisor_certificate::affine_custody) fn cache_literal_axioms(
        &mut self,
        root: &ScalarTerm,
        definition_axioms: &[usize],
        target: &ScalarTerm,
        literal_axioms: Option<Vec<Option<usize>>>,
    ) {
        self.literal_axioms_by_witness.insert(
            (root.clone(), definition_axioms.to_vec(), target.clone()),
            literal_axioms,
        );
    }

    pub(in crate::nonzero_divisor_certificate) fn cached_affine_proof(
        &self,
        goal: &Proposition,
    ) -> Option<Option<ProofNode>> {
        self.affine_proofs.get(goal).cloned()
    }

    pub(in crate::nonzero_divisor_certificate) fn begin_affine_proof(
        &mut self,
        goal: &Proposition,
    ) {
        self.affine_proofs.insert(goal.clone(), None);
    }

    pub(in crate::nonzero_divisor_certificate) fn cache_affine_proof(
        &mut self,
        goal: &Proposition,
        proof: Option<ProofNode>,
    ) {
        self.affine_proofs.insert(goal.clone(), proof);
    }
}
