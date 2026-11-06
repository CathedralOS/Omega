//! Compose symbolic order evidence through existing integer laws. Search may
//! fail, but cannot admit a relation: every successful branch returns an
//! ordinary term over the supplied premise evidence and fixed arithmetic laws.

use super::*;
use crate::mathematical_core::TermArena;
use std::collections::{HashMap, HashSet, VecDeque};

struct OrderQuestion {
    left: TermHandle,
    right: TermHandle,
    proof: Option<TermHandle>,
    depth: usize,
    consumers: Vec<usize>,
    queued: bool,
}

// One predicate owns this graph and borrows its unchanged premises and laws.
// A goal waits for actual proofs of its children, never for an assumption that
// a cycle has failed. Producing a proof wakes the goals that consume it; an
// unresolved cycle cannot wake itself. Completed expansions are shared across
// reconverging paths, including paths through cycles.
pub(super) struct OrderSearch<'premises> {
    premises: &'premises [(TermHandle, TermHandle)],
    laws: &'premises [laws::SourceLaw],
    questions: Vec<OrderQuestion>,
    by_structure: HashMap<(u64, u64), Vec<usize>>,
    // Dependencies are sparse pairs of query positions. Register each edge
    // once instead of rescanning a shared child's consumers on every request.
    dependencies: HashSet<(usize, usize)>,
    pending: VecDeque<usize>,
    current: Option<usize>,
    #[cfg(test)]
    expansions: usize,
    #[cfg(test)]
    comparisons: usize,
}

impl<'premises> OrderSearch<'premises> {
    pub(super) fn new(
        premises: &'premises [(TermHandle, TermHandle)],
        laws: &'premises [laws::SourceLaw],
    ) -> Self {
        Self {
            premises,
            laws,
            questions: Vec::new(),
            by_structure: HashMap::new(),
            dependencies: HashSet::new(),
            pending: VecDeque::new(),
            current: None,
            #[cfg(test)]
            expansions: 0,
            #[cfg(test)]
            comparisons: 0,
        }
    }

    fn enqueue(&mut self, question: usize) {
        let row = &mut self.questions[question];
        if !row.queued && row.proof.is_none() && row.depth < 128 {
            row.queued = true;
            self.pending.push_back(question);
        }
    }

    fn question(
        &mut self,
        arena: &TermArena,
        left: TermHandle,
        right: TermHandle,
    ) -> Result<usize, BoundedDenotationError> {
        let hashes =
            arena
                .structural_hashes(&[left, right])
                .ok_or(BoundedDenotationError::Unsupported(
                    "order goal has an incomplete or cyclic term graph",
                ))?;
        let key = (hashes[0], hashes[1]);
        let mut existing = None;
        if let Some(candidates) = self.by_structure.get(&key) {
            for &position in candidates {
                #[cfg(test)]
                {
                    self.comparisons += 1;
                }
                let question = &self.questions[position];
                if arena.structurally_equal(question.left, left)
                    && arena.structurally_equal(question.right, right)
                {
                    existing = Some(position);
                    break;
                }
            }
        }
        let depth = self
            .current
            .map_or(0, |parent| self.questions[parent].depth + 1);
        let question = if let Some(position) = existing {
            // A shorter path has more of the original depth allowance left.
            // Revisit its dependencies rather than remembering an earlier cut
            // as an unconditional failure.
            if depth < self.questions[position].depth {
                self.questions[position].depth = depth;
                self.enqueue(position);
            }
            position
        } else {
            let position = self.questions.len();
            self.questions.push(OrderQuestion {
                left,
                right,
                proof: None,
                depth,
                consumers: Vec::new(),
                queued: false,
            });
            self.by_structure.entry(key).or_default().push(position);
            self.enqueue(position);
            position
        };
        if let Some(parent) = self.current {
            let row = &mut self.questions[question];
            if row.proof.is_none() && self.dependencies.insert((question, parent)) {
                row.consumers.push(parent);
            }
        }
        Ok(question)
    }

    fn complete(&mut self, question: usize, proof: TermHandle) {
        self.questions[question].proof = Some(proof);
        for consumer in std::mem::take(&mut self.questions[question].consumers) {
            self.enqueue(consumer);
        }
    }
}

#[cfg(test)]
mod tests;

impl Denotation {
    fn source_order_transport(
        &mut self,
        proof: TermHandle,
        left: TermHandle,
        right: TermHandle,
        expected_left: TermHandle,
        expected_right: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let l = self.source_normal_integer(left, 0)?;
        let r = self.source_normal_integer(right, 0)?;
        let el = self.source_normal_integer(expected_left, 0)?;
        let er = self.source_normal_integer(expected_right, 0)?;
        if !self.arena.structurally_equal(l.term, el.term)
            || !self.arena.structurally_equal(r.term, er.term)
        {
            return Ok(None);
        }
        let proof = self.integer_law_application(
            IntegerLaw::LessOrEqualSubstituteLeft,
            &[left, l.term, right, l.equation, proof],
        )?;
        let proof = self.integer_law_application(
            IntegerLaw::LessOrEqualSubstituteRight,
            &[l.term, right, r.term, r.equation, proof],
        )?;
        let integer = self.integer_constant()?;
        let left_equation = self.symmetry(integer, expected_left, el.term, el.equation);
        let right_equation = self.symmetry(integer, expected_right, er.term, er.equation);
        let proof = self.integer_law_application(
            IntegerLaw::LessOrEqualSubstituteLeft,
            &[el.term, expected_left, er.term, left_equation, proof],
        )?;
        self.integer_law_application(
            IntegerLaw::LessOrEqualSubstituteRight,
            &[
                expected_left,
                er.term,
                expected_right,
                right_equation,
                proof,
            ],
        )
        .map(Some)
    }

    fn source_sum_items(
        &mut self,
        term: TermHandle,
        items: &mut Vec<TermHandle>,
        depth: usize,
    ) -> Result<(), BoundedDenotationError> {
        if depth > 128 {
            return Err(BoundedDenotationError::DepthLimitExceeded);
        }
        let add = self.add_operation()?;
        if let Some((_, left, right)) = self
            .source_binary(term)?
            .filter(|(operation, _, _)| *operation == add)
        {
            self.source_sum_items(left, items, depth + 1)?;
            self.source_sum_items(right, items, depth + 1)?;
        } else {
            items.push(term);
        }
        Ok(())
    }
    fn source_sum_from_items(
        &mut self,
        items: &[TermHandle],
    ) -> Result<TermHandle, BoundedDenotationError> {
        let Some((&last, prefix)) = items.split_last() else {
            return self.math_literal(numerics::bignum::BigInt::from_i128(0));
        };
        let mut result = last;
        for &value in prefix.iter().rev() {
            result = self.add_terms(value, result)?;
        }
        Ok(self.source_reduced_sum(result, 0)?.term)
    }

    pub(super) fn source_composed_order(
        &mut self,
        goal: TermHandle,
        search: &mut OrderSearch<'_>,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let Some(IntegerRelation::LessOrEqual { left, right }) = self.integer_relation(goal) else {
            return self.closed_source_predicate(goal, 0);
        };
        self.source_order_search(left, right, search)
    }

    pub(super) fn source_strict_order(
        &mut self,
        left: TermHandle,
        right: TermHandle,
        search: &mut OrderSearch<'_>,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let zero = self.math_literal(numerics::bignum::BigInt::from_i128(0))?;
        let one = self.math_literal(numerics::bignum::BigInt::from_i128(1))?;
        let successor = self.add_terms(left, one)?;
        let Some(bound) = self.source_order_search(successor, right, search)? else {
            return Ok(None);
        };
        let integer = self.integer_constant()?;
        let reflexive = self.arena.insert(Term::Refl {
            ty: integer,
            value: successor,
        });
        let inclusive = self.integer_law_application(
            IntegerLaw::EqualityToLessOrEqual,
            &[successor, successor, reflexive],
        )?;
        let before = self.subtract_terms(successor, one)?;
        let lower = self.source_subtract_cancel_add_left(left, successor, one, inclusive)?;
        let after = self.subtract_terms(successor, zero)?;
        let positive = self.order_term_application(true, zero, one)?;
        let Some(positive) = self.closed_source_predicate(positive, 0)? else {
            return Ok(None);
        };
        let strict = self.source_subtract_antitone(successor, zero, one, positive)?;
        let zero_identity = self.source_subtract_zero(successor)?;
        let strict = self.integer_law_application(
            IntegerLaw::LessThanSubstituteRight,
            &[before, after, successor, zero_identity, strict],
        )?;
        let step = self.integer_law_application(
            IntegerLaw::LessOrEqualLessThanTransitivity,
            &[left, before, successor, lower, strict],
        )?;
        self.integer_law_application(
            IntegerLaw::LessThanLessOrEqualTransitivity,
            &[left, successor, right, step, bound],
        )
        .map(Some)
    }

    fn source_order_search(
        &mut self,
        left: TermHandle,
        right: TermHandle,
        search: &mut OrderSearch<'_>,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let root = search.question(&self.arena, left, right)?;
        while search.questions[root].proof.is_none() {
            let Some(question) = search.pending.pop_front() else {
                break;
            };
            search.questions[question].queued = false;
            if search.questions[question].proof.is_some() {
                continue;
            }
            let left = search.questions[question].left;
            let right = search.questions[question].right;
            search.current = Some(question);
            #[cfg(test)]
            {
                search.expansions += 1;
            }
            let result = self.expand_source_order(left, right, search);
            search.current = None;
            if let Some(proof) = result? {
                search.complete(question, proof);
            }
        }
        Ok(search.questions[root].proof)
    }

    fn request_source_order(
        &self,
        left: TermHandle,
        right: TermHandle,
        search: &mut OrderSearch<'_>,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let question = search.question(&self.arena, left, right)?;
        Ok(search.questions[question].proof)
    }

    fn expand_source_order(
        &mut self,
        left: TermHandle,
        right: TermHandle,
        search: &mut OrderSearch<'_>,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let premises = search.premises;
        let laws = search.laws;
        let goal = self.order_term_application(false, left, right)?;
        if let Some(proof) = self.closed_source_predicate(goal, 0)? {
            return Ok(Some(proof));
        }
        if self.arena.structurally_equal(left, right) {
            let integer = self.integer_constant()?;
            let equation = self.arena.insert(Term::Refl {
                ty: integer,
                value: left,
            });
            return self
                .integer_law_application(
                    IntegerLaw::EqualityToLessOrEqual,
                    &[left, right, equation],
                )
                .map(Some);
        }
        for &(proposition, evidence) in premises {
            if self.arena.structurally_equal(goal, proposition) {
                return Ok(Some(evidence));
            }
        }
        for law in laws {
            let Some(application) = self.source_law_application_plan(law, goal)? else {
                continue;
            };
            let mut proofs = Vec::new();
            let mut complete = true;
            for &condition in &application.conditions {
                let Some(IntegerRelation::LessOrEqual { left, right }) =
                    self.integer_relation(condition)
                else {
                    complete = false;
                    break;
                };
                let Some(proof) = self.request_source_order(left, right, search)? else {
                    complete = false;
                    break;
                };
                proofs.push(proof);
            }
            if !complete {
                continue;
            }
            let mut evidence = application.evidence;
            for argument in application.arguments {
                let value = match argument {
                    laws::LawArgument::Value(position) => application.values[position],
                    laws::LawArgument::Condition(position) => proofs[position],
                };
                evidence = self.source_apply(evidence, value);
            }
            return Ok(Some(evidence));
        }
        // Removing the same sum on both endpoints proposes a smaller goal.
        // The resulting proof is added back and transported to the original
        // endpoints; no cancellation verdict is trusted as evidence.
        let mut left_items = Vec::new();
        let mut right_items = Vec::new();
        self.source_sum_items(left, &mut left_items, 0)?;
        self.source_sum_items(right, &mut right_items, 0)?;
        let mut common = Vec::new();
        left_items.retain(|&item| {
            if let Some(position) = right_items
                .iter()
                .position(|&other| self.arena.structurally_equal(item, other))
            {
                right_items.remove(position);
                common.push(item);
                false
            } else {
                true
            }
        });
        if !common.is_empty() {
            let a = self.source_sum_from_items(&left_items)?;
            let b = self.source_sum_from_items(&right_items)?;
            if let Some(proof) = self.request_source_order(a, b, search)? {
                let amount = self.source_sum_from_items(&common)?;
                let proof = self.add_order_translation(a, b, amount, proof)?;
                let l = self.add_terms(a, amount)?;
                let r = self.add_terms(b, amount)?;
                if let Some(proof) = self.source_order_transport(proof, l, r, left, right)? {
                    return Ok(Some(proof));
                }
            }
        }
        // Follow an actual supplied order, retaining its evidence and using
        // transitivity only after the remainder has independently derived.
        for &(proposition, evidence) in premises {
            let Some(IntegerRelation::LessOrEqual {
                left: start,
                right: middle,
            }) = self.integer_relation(proposition)
            else {
                continue;
            };
            if !self.arena.structurally_equal(start, left)
                || self.arena.structurally_equal(start, middle)
            {
                continue;
            }
            if let Some(rest) = self.request_source_order(middle, right, search)? {
                return self
                    .integer_law_application(
                        IntegerLaw::LessOrEqualTransitivity,
                        &[left, middle, right, evidence, rest],
                    )
                    .map(Some);
            }
        }
        let add = self.add_operation()?;
        let l = self
            .source_binary(left)?
            .filter(|(operation, _, _)| *operation == add);
        let r = self
            .source_binary(right)?
            .filter(|(operation, _, _)| *operation == add);
        let zero = self.math_literal(numerics::bignum::BigInt::from_i128(0))?;
        let candidates = match (l, r) {
            (Some((_, a, c)), Some((_, b, d))) => vec![(a, c, b, d)],
            (None, Some((_, b, d))) => vec![(left, zero, b, d), (zero, left, b, d)],
            (Some((_, a, c)), None) => vec![(a, c, right, zero), (a, c, zero, right)],
            (None, None) => Vec::new(),
        };
        for (a, c, b, d) in candidates {
            let Some(first) = self.request_source_order(a, b, search)? else {
                continue;
            };
            let Some(second) = self.request_source_order(c, d, search)? else {
                continue;
            };
            let proof = self.add_order_both(a, b, c, d, first, second)?;
            let l = self.add_terms(a, c)?;
            let r = self.add_terms(b, d)?;
            if let Some(proof) = self.source_order_transport(proof, l, r, left, right)? {
                return Ok(Some(proof));
            }
        }
        Ok(None)
    }
}
