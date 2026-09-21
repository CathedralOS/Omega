//! Reusable proof lemmas and quantified/sequence-style facts.
//!
//! Bounds, lengths, and window transformations show up in almost every
//! slice/text proof. Re-deriving the same arithmetic at every check site is
//! both slow and error prone, so this module captures the recurring shapes as
//! small, composable lemmas the prover can apply directly.
//!
//! The facts here are deliberately narrow first-order shapes. They are not a
//! general theorem prover; they are the handful of length/bounds/window
//! relationships that the checker keeps needing, plus a single
//! "for all `i` in range, `P(i)`" quantified shape so text/slice invariants can
//! be stated without pretending proof binders are runtime loops (see
//! `wiki/language_guide/chapter_10_compile_time_proofs.md`).

/// A reusable proof lemma over slice/sequence lengths, index bounds, and
/// window (sub-slice) transformations.
///
/// Each variant encodes a relationship the prover may *assume* once its
/// premises are established, so individual check sites do not re-derive the
/// arithmetic. Lemmas are intentionally side-effect free and comparable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofLemma {
    /// `index < length` implies `index` is a valid offset into the collection.
    ///
    /// The canonical bounds lemma: once a value is known to be strictly less
    /// than the collection length (and non-negative, which `usize` guarantees),
    /// indexing at that offset is in bounds.
    IndexInBounds,
    /// A non-empty collection has `length >= 1`, so offset `0` is in bounds.
    NonEmptyHasFirst,
    /// The length of a window `items[start..end]` is `end - start` whenever
    /// `start <= end <= length`.
    WindowLength,
    /// A window `items[start..end]` is contained in `items` whenever
    /// `start <= end <= length`; every in-bounds offset of the window is an
    /// in-bounds offset of the parent.
    WindowSubrange,
    /// A tail window `items[1..]` is strictly shorter than `items` whenever
    /// `items` is non-empty. This is the well-founded measure that backs
    /// `terminates by items -> Slice::Length` ranking proofs.
    TailLengthDecreases,
}

impl ProofLemma {
    /// The logical foundation this lemma's justification stands on.
    ///
    /// The classicality audit (`wiki/spec/proofs/classicality.md`) covers the
    /// obligation-side lemma library beside the certificate rules: an
    /// exhaustive `match` keeps the boundary enforced at compile time, so a
    /// new lemma does not compile until it is classified here.
    ///
    /// Every lemma today decides a closed integer relation over exact
    /// collection bounds — `index < length`, `length >= 1`, or
    /// `start <= end <= length` — the same decidable fixed-integer fragment
    /// the certificate rules' checked witnesses use. A lemma generalized to
    /// an undecidable relation is a disguised classical step and must be
    /// reclassified before it lands.
    pub const fn foundation(self) -> proof_admission::ProofRuleFoundation {
        match self {
            Self::IndexInBounds
            | Self::NonEmptyHasFirst
            | Self::WindowLength
            | Self::WindowSubrange
            | Self::TailLengthDecreases => {
                proof_admission::ProofRuleFoundation::ConstructiveDecidable
            }
        }
    }
}

impl ProofLemma {
    /// All lemmas, in a stable order, for registries and exhaustive iteration.
    pub const ALL: [ProofLemma; 5] = [
        ProofLemma::IndexInBounds,
        ProofLemma::NonEmptyHasFirst,
        ProofLemma::WindowLength,
        ProofLemma::WindowSubrange,
        ProofLemma::TailLengthDecreases,
    ];

    /// Stable identifier used in diagnostics and registry records.
    pub fn name(self) -> &'static str {
        match self {
            ProofLemma::IndexInBounds => "index_in_bounds",
            ProofLemma::NonEmptyHasFirst => "non_empty_has_first",
            ProofLemma::WindowLength => "window_length",
            ProofLemma::WindowSubrange => "window_subrange",
            ProofLemma::TailLengthDecreases => "tail_length_decreases",
        }
    }

    /// The premises that must already hold before the lemma may be applied.
    pub fn premises(self) -> &'static [LemmaFact] {
        match self {
            ProofLemma::IndexInBounds => &[LemmaFact::IndexLessThanLength],
            ProofLemma::NonEmptyHasFirst => &[LemmaFact::NonEmpty],
            ProofLemma::WindowLength | ProofLemma::WindowSubrange => {
                &[LemmaFact::WindowWithinLength]
            }
            ProofLemma::TailLengthDecreases => &[LemmaFact::NonEmpty],
        }
    }

    /// The fact the lemma establishes once its premises hold.
    pub fn conclusion(self) -> LemmaFact {
        match self {
            ProofLemma::IndexInBounds | ProofLemma::NonEmptyHasFirst => LemmaFact::InBounds,
            ProofLemma::WindowLength => LemmaFact::WindowLengthIsExtent,
            ProofLemma::WindowSubrange => LemmaFact::WindowWithinParent,
            ProofLemma::TailLengthDecreases => LemmaFact::TailStrictlyShorter,
        }
    }
}

impl ProofLemma {
    /// Apply this lemma to a set of already-established premises. Returns the
    /// concluded [`LemmaFact`] when *every* premise the lemma requires is
    /// present, otherwise `None`.
    ///
    /// This is the single entry point check sites use to discharge a recurring
    /// length/bounds/window fact through a *named* lemma instead of re-deriving
    /// the arithmetic locally. Routing through here keeps the reasoning auditable
    /// (the lemma name appears in evidence/diagnostics) and consistent across
    /// every call site.
    pub fn apply(self, established: &[LemmaFact]) -> Option<LemmaFact> {
        self.premises()
            .iter()
            .all(|premise| established.contains(premise))
            .then(|| self.conclusion())
    }

    /// The first lemma (in stable [`ProofLemma::ALL`] order) whose premises are
    /// all satisfied by `established` and that concludes `goal`. This lets a
    /// check site ask "is there a named lemma that discharges this fact from what
    /// I already know?" without hard-coding which lemma applies.
    pub fn discharging(goal: LemmaFact, established: &[LemmaFact]) -> Option<ProofLemma> {
        ProofLemma::ALL
            .into_iter()
            .find(|lemma| lemma.conclusion() == goal && lemma.apply(established) == Some(goal))
    }
}

/// A first-order fact a lemma can consume as a premise or produce as a
/// conclusion. These are the recurring length/bounds/window relationships,
/// kept abstract over which concrete collection/index they describe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LemmaFact {
    /// `index < length` for some collection.
    IndexLessThanLength,
    /// `length >= 1`.
    NonEmpty,
    /// `start <= end <= length` for a window `[start..end]`.
    WindowWithinLength,
    /// The index is a valid in-bounds offset.
    InBounds,
    /// `(end - start)` is the window's length.
    WindowLengthIsExtent,
    /// Every offset of the window is an offset of the parent collection.
    WindowWithinParent,
    /// The tail window is strictly shorter than its parent.
    TailStrictlyShorter,
}

/// A `for all i in start..end, P(i)` style proof fact.
///
/// This is the narrow, sequence-oriented quantified shape the checker needs for
/// text/slice invariants such as "every byte in this window is ASCII" or "every
/// earlier index holds a value <= every later index". The binder is proof-only:
/// it never lowers to a runtime loop. `predicate` names the per-element
/// predicate (for example a domain or a comparison), and the range is the
/// half-open index space the predicate is asserted to hold over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForAllInRangeFact {
    /// Name of the per-index predicate `P` (e.g. a domain or comparison name).
    pub predicate: String,
    /// Inclusive lower bound of the quantified index range.
    pub start: QuantifiedBound,
    /// Exclusive upper bound of the quantified index range.
    pub end: QuantifiedBound,
}

impl ForAllInRangeFact {
    /// The foundation this fact's one elimination step stands on.
    ///
    /// `proves_element` discharges a per-element goal only by deciding
    /// bounded range membership — concrete literal bounds, or the symbolic
    /// full-extent `0..length` paired with an index already proven in
    /// bounds — and a vacuous range discharges no concrete element. That is
    /// a decision inside the decidable fragment; extending it to an
    /// undecidable membership relation is a reclassification under the
    /// classicality audit, not a local edit.
    pub const ELIMINATION_FOUNDATION: proof_admission::ProofRuleFoundation =
        proof_admission::ProofRuleFoundation::ConstructiveDecidable;

    /// Build a `for all i in start..end, P(i)` fact.
    pub fn new(predicate: impl Into<String>, start: QuantifiedBound, end: QuantifiedBound) -> Self {
        Self {
            predicate: predicate.into(),
            start,
            end,
        }
    }

    /// A fact quantified over the full extent `0..length` of a collection.
    pub fn over_full_extent(predicate: impl Into<String>) -> Self {
        Self::new(predicate, QuantifiedBound::Zero, QuantifiedBound::Length)
    }

    /// Whether this quantified fact discharges "element at `index` satisfies
    /// `predicate`". It does so when the predicate matches and `index` provably
    /// lies inside the quantified range. A vacuous range proves nothing (there
    /// is no such element), so it never discharges a concrete element goal.
    ///
    /// This is the consumer side of the sequence-style invariant: a guard or
    /// contract that established "every element in 0..len is `P`" lets an
    /// element access at a proven-in-range index be treated as `P` without a
    /// per-element runtime check.
    pub fn proves_element(&self, predicate: &str, index: ElementIndex) -> bool {
        if self.predicate != predicate || self.is_vacuous() {
            return false;
        }
        self.contains_index(index)
    }

    /// Whether `index` provably lies within `[start, end)`. Symbolic bounds are
    /// treated conservatively: an index is only proven in-range when it can be
    /// compared against concrete literal bounds, or when it carries its own
    /// in-bounds witness (`literal < length` discharges a `length` upper bound).
    fn contains_index(&self, index: ElementIndex) -> bool {
        let literal = match index {
            // A symbolic (in-bounds) index is covered only by a full-extent
            // quantifier, whose range is exactly the collection's valid
            // offsets; a narrower range gives it no literal position.
            ElementIndex::InBounds => {
                return matches!(
                    (self.start, self.end),
                    (QuantifiedBound::Zero, QuantifiedBound::Length)
                );
            }
            ElementIndex::Literal(offset) | ElementIndex::LiteralInBounds(offset) => offset,
        };

        let lower_ok = match self.start {
            QuantifiedBound::Zero => literal >= 0,
            QuantifiedBound::Literal(start) => literal >= start,
            QuantifiedBound::Length => false,
        };
        let upper_ok = match self.end {
            QuantifiedBound::Zero => false,
            QuantifiedBound::Literal(end) => literal < end,
            // A `length` upper bound covers a literal only when the literal
            // carries the `literal < length` witness a bare literal lacks.
            QuantifiedBound::Length => matches!(index, ElementIndex::LiteralInBounds(_)),
        };
        lower_ok && upper_ok
    }

    /// Whether the quantified range is provably empty, in which case the fact
    /// holds vacuously. Bounds that can be compared symbolically are compared;
    /// a bound that cannot be ordered is treated as possibly non-empty.
    pub fn is_vacuous(&self) -> bool {
        match (self.start, self.end) {
            // Nothing lies at or after the collection's own extent.
            (QuantifiedBound::Length, _) => true,
            // A zero-ended range is empty unless its start is a malformed
            // negative literal.
            (_, QuantifiedBound::Zero) => {
                !matches!(self.start, QuantifiedBound::Literal(start) if start < 0)
            }
            (QuantifiedBound::Literal(start), QuantifiedBound::Literal(end)) => start >= end,
            (QuantifiedBound::Zero, QuantifiedBound::Literal(end)) => end <= 0,
            _ => false,
        }
    }
}

/// An index into a quantified range, used when discharging a per-element goal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementIndex {
    /// A concrete literal offset.
    Literal(i64),
    /// A symbolic index already proven to be a valid in-bounds offset (for
    /// example via [`ProofLemma::IndexInBounds`]).
    InBounds,
    /// A concrete literal offset already proven to be a valid in-bounds
    /// offset — the literal was witnessed `literal < length` (for example via
    /// [`ProofLemma::IndexInBounds`]), so a `length`-bounded quantifier covers
    /// it where a bare [`ElementIndex::Literal`] cannot.
    LiteralInBounds(i64),
}

/// A bound of a quantified index range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantifiedBound {
    /// The literal `0`.
    Zero,
    /// A concrete literal offset.
    Literal(i64),
    /// The (symbolic) length of the collection.
    Length,
}

#[cfg(test)]
mod tests {
    use super::{ElementIndex, ForAllInRangeFact, LemmaFact, ProofLemma, QuantifiedBound};

    #[test]
    fn no_lemma_stands_on_a_classical_foundation() {
        for lemma in ProofLemma::ALL {
            assert_ne!(
                lemma.foundation(),
                proof_admission::ProofRuleFoundation::Classical,
                "{lemma:?} must not silently rest on classical reasoning"
            );
        }
    }

    #[test]
    fn every_lemma_decides_a_decidable_relation_today() {
        // The audit's completeness check beside the exhaustive `match` in
        // `foundation`: a lemma that is genuinely constructive (structural
        // inference needing no decision) or a trusted admission changes
        // this pin deliberately, in the same edit that classifies it.
        for lemma in ProofLemma::ALL {
            assert_eq!(
                lemma.foundation(),
                proof_admission::ProofRuleFoundation::ConstructiveDecidable,
                "{lemma:?} foundation drifted off the decidable fragment"
            );
        }
    }

    #[test]
    fn quantified_elimination_stays_inside_the_decidable_fragment() {
        assert_eq!(
            ForAllInRangeFact::ELIMINATION_FOUNDATION,
            proof_admission::ProofRuleFoundation::ConstructiveDecidable
        );
    }

    #[test]
    fn lemmas_have_distinct_stable_names() {
        let mut names: Vec<&str> = ProofLemma::ALL.iter().map(|lemma| lemma.name()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "lemma names must be distinct");
    }

    #[test]
    fn bounds_lemmas_conclude_in_bounds() {
        assert_eq!(ProofLemma::IndexInBounds.conclusion(), LemmaFact::InBounds);
        assert_eq!(
            ProofLemma::IndexInBounds.premises(),
            [LemmaFact::IndexLessThanLength].as_slice()
        );
        assert_eq!(
            ProofLemma::NonEmptyHasFirst.conclusion(),
            LemmaFact::InBounds
        );
    }

    #[test]
    fn window_lemmas_share_within_length_premise() {
        assert_eq!(
            ProofLemma::WindowLength.premises(),
            [LemmaFact::WindowWithinLength].as_slice()
        );
        assert_eq!(
            ProofLemma::WindowSubrange.premises(),
            [LemmaFact::WindowWithinLength].as_slice()
        );
        assert_eq!(
            ProofLemma::WindowSubrange.conclusion(),
            LemmaFact::WindowWithinParent
        );
    }

    #[test]
    fn apply_requires_all_premises() {
        // Premise present: bounds lemma fires.
        assert_eq!(
            ProofLemma::IndexInBounds.apply(&[LemmaFact::IndexLessThanLength]),
            Some(LemmaFact::InBounds)
        );
        // Premise missing: lemma does not fire.
        assert_eq!(ProofLemma::IndexInBounds.apply(&[]), None);
        assert_eq!(
            ProofLemma::IndexInBounds.apply(&[LemmaFact::NonEmpty]),
            None
        );
    }

    #[test]
    fn discharging_finds_named_lemma_for_goal() {
        // `InBounds` can be discharged from `NonEmpty` via NonEmptyHasFirst, or
        // from `IndexLessThanLength` via IndexInBounds. With only NonEmpty
        // established, the non-empty lemma is the one that discharges it.
        assert_eq!(
            ProofLemma::discharging(LemmaFact::InBounds, &[LemmaFact::NonEmpty]),
            Some(ProofLemma::NonEmptyHasFirst)
        );
        assert_eq!(
            ProofLemma::discharging(LemmaFact::InBounds, &[LemmaFact::IndexLessThanLength]),
            Some(ProofLemma::IndexInBounds)
        );
        // No premise for the goal: nothing discharges it.
        assert_eq!(
            ProofLemma::discharging(LemmaFact::InBounds, &[LemmaFact::WindowWithinLength]),
            None
        );
    }

    #[test]
    fn tail_length_lemma_backs_termination_measure() {
        assert_eq!(
            ProofLemma::TailLengthDecreases.premises(),
            [LemmaFact::NonEmpty].as_slice()
        );
        assert_eq!(
            ProofLemma::TailLengthDecreases.conclusion(),
            LemmaFact::TailStrictlyShorter
        );
    }

    #[test]
    fn for_all_full_extent_quantifies_over_length() {
        let fact = ForAllInRangeFact::over_full_extent("Ascii");
        assert_eq!(fact.start, QuantifiedBound::Zero);
        assert_eq!(fact.end, QuantifiedBound::Length);
        assert_eq!(fact.predicate, "Ascii");
        assert!(!fact.is_vacuous());
    }

    #[test]
    fn empty_literal_range_is_vacuous() {
        let fact = ForAllInRangeFact::new(
            "Positive",
            QuantifiedBound::Literal(3),
            QuantifiedBound::Literal(3),
        );
        assert!(fact.is_vacuous());

        let non_empty = ForAllInRangeFact::new(
            "Positive",
            QuantifiedBound::Literal(0),
            QuantifiedBound::Literal(2),
        );
        assert!(!non_empty.is_vacuous());
    }

    #[test]
    fn zero_to_length_is_not_assumed_vacuous() {
        let fact = ForAllInRangeFact::new("Ascii", QuantifiedBound::Zero, QuantifiedBound::Length);
        assert!(!fact.is_vacuous());
    }

    #[test]
    fn full_extent_fact_proves_in_bounds_element() {
        let fact = ForAllInRangeFact::over_full_extent("Ascii");
        // A symbolic in-bounds index is covered by the full extent.
        assert!(fact.proves_element("Ascii", ElementIndex::InBounds));
        // Wrong predicate is not discharged.
        assert!(!fact.proves_element("Digit", ElementIndex::InBounds));
    }

    #[test]
    fn literal_range_proves_literal_element_in_range() {
        let fact = ForAllInRangeFact::new(
            "Positive",
            QuantifiedBound::Literal(1),
            QuantifiedBound::Literal(4),
        );
        assert!(fact.proves_element("Positive", ElementIndex::Literal(1)));
        assert!(fact.proves_element("Positive", ElementIndex::Literal(3)));
        // Out of range below and at/above the exclusive end.
        assert!(!fact.proves_element("Positive", ElementIndex::Literal(0)));
        assert!(!fact.proves_element("Positive", ElementIndex::Literal(4)));
        // A bare literal index is not discharged by a symbolic upper bound.
        let symbolic = ForAllInRangeFact::over_full_extent("Positive");
        assert!(!symbolic.proves_element("Positive", ElementIndex::Literal(2)));
        // The same literal carrying its in-bounds witness is discharged.
        assert!(symbolic.proves_element("Positive", ElementIndex::LiteralInBounds(2)));
    }

    #[test]
    fn literal_in_bounds_discharges_length_bounded_ranges() {
        // `Literal(s)..Length`: the witness supplies `literal < length`, so the
        // literal only owes the lower bound.
        let tail = ForAllInRangeFact::new(
            "Checked",
            QuantifiedBound::Literal(3),
            QuantifiedBound::Length,
        );
        assert!(tail.proves_element("Checked", ElementIndex::LiteralInBounds(3)));
        assert!(tail.proves_element("Checked", ElementIndex::LiteralInBounds(9)));
        assert!(!tail.proves_element("Checked", ElementIndex::LiteralInBounds(2)));
        // A bare literal still fails the symbolic upper bound.
        assert!(!tail.proves_element("Checked", ElementIndex::Literal(9)));
        // And a symbolic index without a literal position is not discharged by
        // a narrower start than the full extent.
        assert!(!tail.proves_element("Checked", ElementIndex::InBounds));

        // `Zero..Literal(e)`: the witness is unused; the literal owes `i < e`.
        let head = ForAllInRangeFact::new(
            "Checked",
            QuantifiedBound::Zero,
            QuantifiedBound::Literal(5),
        );
        assert!(head.proves_element("Checked", ElementIndex::LiteralInBounds(4)));
        assert!(!head.proves_element("Checked", ElementIndex::LiteralInBounds(5)));

        // Literal-bounded ranges behave exactly as for `Literal`.
        let middle = ForAllInRangeFact::new(
            "Checked",
            QuantifiedBound::Literal(2),
            QuantifiedBound::Literal(9),
        );
        assert!(middle.proves_element("Checked", ElementIndex::LiteralInBounds(2)));
        assert!(middle.proves_element("Checked", ElementIndex::LiteralInBounds(8)));
        assert!(!middle.proves_element("Checked", ElementIndex::LiteralInBounds(9)));
    }

    #[test]
    fn symbolic_vacuity_covers_extent_and_zero_bounds() {
        // `length..` is empty: nothing lies at or after the extent.
        assert!(
            ForAllInRangeFact::new("P", QuantifiedBound::Length, QuantifiedBound::Length)
                .is_vacuous()
        );
        assert!(
            ForAllInRangeFact::new("P", QuantifiedBound::Length, QuantifiedBound::Literal(4))
                .is_vacuous()
        );
        assert!(
            ForAllInRangeFact::new("P", QuantifiedBound::Length, QuantifiedBound::Zero)
                .is_vacuous()
        );
        // `..0` is empty for every provably non-negative start.
        assert!(
            ForAllInRangeFact::new("P", QuantifiedBound::Literal(3), QuantifiedBound::Zero)
                .is_vacuous()
        );
        assert!(
            ForAllInRangeFact::new("P", QuantifiedBound::Zero, QuantifiedBound::Zero).is_vacuous()
        );
        // `0..e` with `e <= 0` is empty; `0..length` stays unproven either way.
        assert!(
            ForAllInRangeFact::new("P", QuantifiedBound::Zero, QuantifiedBound::Literal(-1))
                .is_vacuous()
        );
        assert!(
            !ForAllInRangeFact::new("P", QuantifiedBound::Zero, QuantifiedBound::Length)
                .is_vacuous()
        );
        assert!(
            !ForAllInRangeFact::new("P", QuantifiedBound::Literal(0), QuantifiedBound::Length)
                .is_vacuous()
        );
    }

    #[test]
    fn vacuous_range_proves_no_element() {
        let fact = ForAllInRangeFact::new(
            "Ascii",
            QuantifiedBound::Literal(3),
            QuantifiedBound::Literal(3),
        );
        assert!(!fact.proves_element("Ascii", ElementIndex::Literal(3)));
        assert!(!fact.proves_element("Ascii", ElementIndex::InBounds));
    }
}
