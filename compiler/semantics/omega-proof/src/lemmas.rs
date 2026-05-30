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
    /// `decreases items -> Slice::Length` style termination proofs.
    TailLengthDecreases,
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
    /// Build a `for all i in start..end, P(i)` fact.
    pub fn new(
        predicate: impl Into<String>,
        start: QuantifiedBound,
        end: QuantifiedBound,
    ) -> Self {
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

    /// Whether the quantified range is provably empty, in which case the fact
    /// holds vacuously. Only literal bounds can be compared; symbolic bounds are
    /// treated conservatively as possibly non-empty.
    pub fn is_vacuous(&self) -> bool {
        match (self.start, self.end) {
            (QuantifiedBound::Literal(start), QuantifiedBound::Literal(end)) => start >= end,
            (QuantifiedBound::Zero, QuantifiedBound::Literal(end)) => end == 0,
            (QuantifiedBound::Zero, QuantifiedBound::Zero) => true,
            _ => false,
        }
    }
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
    use super::*;

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
            &[LemmaFact::IndexLessThanLength]
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
            &[LemmaFact::WindowWithinLength]
        );
        assert_eq!(
            ProofLemma::WindowSubrange.premises(),
            &[LemmaFact::WindowWithinLength]
        );
        assert_eq!(
            ProofLemma::WindowSubrange.conclusion(),
            LemmaFact::WindowWithinParent
        );
    }

    #[test]
    fn tail_length_lemma_backs_termination_measure() {
        assert_eq!(
            ProofLemma::TailLengthDecreases.premises(),
            &[LemmaFact::NonEmpty]
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
}
