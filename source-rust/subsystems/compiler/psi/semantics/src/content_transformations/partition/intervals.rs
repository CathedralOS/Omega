//! Compare a half-open interval with the disjoint union of its pieces.
use semantic_vocabulary::IntegerMathTerm as Term;
use std::collections::{BTreeMap, BTreeSet};
use std::{cell::Cell, cmp::Ordering, rc::Rc};

pub(super) type Interval = (Term, Term);

#[derive(Clone, Default)]
pub(super) struct Bounds {
    comparisons: Rc<Cell<usize>>,
    nonnegative: BTreeSet<Term>,
    orders: BTreeSet<(Term, Term)>,
    equal: BTreeMap<Term, Term>,
}

impl Bounds {
    pub(super) fn len(&self) -> usize {
        self.nonnegative.len() + self.orders.len() + self.equal.len()
    }

    pub(super) fn nonnegative(&mut self, term: Term) {
        self.nonnegative.insert(term);
    }

    pub(super) fn order(&mut self, lower: Term, upper: Term) {
        if lower == upper {
            return;
        }
        if self.orders.contains(&(upper.clone(), lower.clone())) {
            // Two opposite bounds identify the atoms on this branch. Give
            // the class a stable representative before ring normalization.
            let atom = |t: &Term| {
                matches!(
                    t,
                    Term::MathValue { .. } | Term::Embed(_) | Term::IntegerLiteral(_)
                )
            };
            if atom(&lower) && atom(&upper) {
                let a = self.representative(&lower).clone();
                let b = self.representative(&upper).clone();
                if a != b {
                    let (smaller, larger) = if a < b { (a, b) } else { (b, a) };
                    self.equal.insert(larger, smaller);
                }
            }
        }
        self.orders.insert((lower, upper));
    }

    fn representative<'a>(&'a self, term: &'a Term) -> &'a Term {
        let mut current = term;
        while let Some(next) = self.equal.get(current) {
            current = next;
        }
        current
    }

    fn normalized(&self, term: &Term) -> Term {
        let pair =
            |a: &Term, b: &Term| (Box::new(self.normalized(a)), Box::new(self.normalized(b)));
        match term {
            Term::Add(a, b) => {
                let (a, b) = pair(a, b);
                Term::Add(a, b)
            }
            Term::Subtract(a, b) => {
                let (a, b) = pair(a, b);
                Term::Subtract(a, b)
            }
            Term::Multiply(a, b) => {
                let (a, b) = pair(a, b);
                Term::Multiply(a, b)
            }
            other => self.representative(other).clone(),
        }
    }

    fn ring_compare(&self, a: &Term, b: &Term) -> Result<Option<Ordering>, &'static str> {
        let work = self.comparisons.get();
        if work >= 65_536 {
            return Err("interval partition exceeds its comparison budget");
        }
        self.comparisons.set(work + 1);
        proof_admission::compare_integer_math_terms(&self.normalized(a), &self.normalized(b))
            .map_err(|_| "interval endpoint comparison exceeds its proof budget")
    }

    pub(super) fn compare(&self, a: &Term, b: &Term) -> Result<Option<Ordering>, &'static str> {
        if let Some(order) = self.ring_compare(a, b)? {
            return Ok(Some(order));
        }
        // Equalities between compound terms also hold after adding or
        // subtracting the same expression from both sides (e.g. endpoints).
        let difference = Term::Subtract(Box::new(a.clone()), Box::new(b.clone()));
        for (left, right) in &self.orders {
            if self.orders.contains(&(right.clone(), left.clone())) {
                let equal_to_zero = Term::Subtract(Box::new(left.clone()), Box::new(right.clone()));
                if self.ring_compare(&difference, &equal_to_zero)? == Some(Ordering::Equal) {
                    return Ok(Some(Ordering::Equal));
                }
            }
        }
        Ok(None)
    }

    fn ordered(&self, a: &Term, b: &Term) -> Result<bool, &'static str> {
        if let Some(order) = self.compare(a, b)? {
            return Ok(order != Ordering::Greater);
        }
        let difference = Term::Subtract(Box::new(b.clone()), Box::new(a.clone()));
        for nonnegative in &self.nonnegative {
            if self.compare(&difference, nonnegative)? == Some(Ordering::Equal) {
                return Ok(true);
            }
        }
        for (lower, upper) in &self.orders {
            let known = Term::Subtract(Box::new(upper.clone()), Box::new(lower.clone()));
            if self.compare(&difference, &known)? == Some(Ordering::Equal) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn partition(
        &self,
        (start, end): &Interval,
        pieces: &[Interval],
    ) -> Result<(), &'static str> {
        if !self.ordered(start, end)? {
            return Err("whole interval endpoints are not ordered");
        }
        let mut nonempty = Vec::new();
        for (a, b) in pieces {
            if !self.ordered(a, b)? {
                return Err("piece endpoints are not ordered");
            }
            // Empty intervals have no location. This also covers an empty
            // suffix whose mathematical endpoint is not a representable address.
            if self.compare(a, b)? != Some(Ordering::Equal) {
                nonempty.push((a, b));
            }
        }
        let mut cursor = start;
        while !nonempty.is_empty() {
            let mut next = None;
            for (index, (a, _)) in nonempty.iter().enumerate() {
                if self.compare(cursor, a)? == Some(Ordering::Equal)
                    && next.replace(index).is_some()
                {
                    return Err("interval pieces do not have a unique disjoint continuation");
                }
            }
            let index = next.ok_or("interval pieces overlap or leave an unproved gap")?;
            cursor = nonempty.swap_remove(index).1;
        }
        if self.compare(cursor, end)? != Some(Ordering::Equal) {
            // Adjacency above makes the sum telescope. This second form also
            // proves joins whose authored length adds independent inputs.
            let whole_length = Term::Subtract(Box::new(end.clone()), Box::new(start.clone()));
            let mut length = Term::literal(semantic_vocabulary::IntegerValue::Unsigned(0));
            for (base, limit) in pieces {
                let width = Term::Subtract(Box::new(limit.clone()), Box::new(base.clone()));
                length = Term::Add(Box::new(length), Box::new(width));
                closed_predicates::check_integer_math_term_size(&length, &length)
                    .map_err(|_| "interval length exceeds its proof budget")?;
            }
            if self.compare(&length, &whole_length)? != Some(Ordering::Equal) {
                return Err("interval pieces do not reach the whole endpoint");
            }
        }
        Ok(())
    }
}
