/*
    Reusable arithmetic, order, and bounded-range lemma constructors for the
    obligation checker.

    We define the named lemma schemas and step constructors used when assembling
    bounded integer and interval derivations.
*/

macro_rules! proof_lemmas {
    ($( { $(#[$doc:meta])* $variant:ident => ($name:literal, $foundation:ident, $premises:expr, $conclusion:expr) } ),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ProofLemma {
            $( $(#[$doc])* $variant ),+
        }

        impl ProofLemma {
            pub const fn foundation(self) -> proof_admission::ProofRuleFoundation {
                match self {
                    $( ProofLemma::$variant => proof_admission::ProofRuleFoundation::$foundation ),+
                }
            }

            pub const ALL: [ProofLemma; proof_lemmas!(@count $( $variant )+)] = [
                $( ProofLemma::$variant ),+
            ];

            pub fn name(self) -> &'static str {
                match self {
                    $( ProofLemma::$variant => $name ),+
                }
            }

            pub fn premises(self) -> &'static [LemmaFact] {
                match self {
                    $( ProofLemma::$variant => $premises ),+
                }
            }

            pub fn conclusion(self) -> LemmaFact {
                match self {
                    $( ProofLemma::$variant => $conclusion ),+
                }
            }
        }
    };
    (@count $( $variant:ident )+) => {
        <[()]>::len(&[$(proof_lemmas!(@unit $variant)),+])
    };
    (@unit $variant:ident) => { () };
}

proof_lemmas! {
    {
        IndexInBounds => (
            "index_in_bounds",
            ConstructiveDecidable,
            &[LemmaFact::IndexLessThanLength],
            LemmaFact::InBounds
        )
    },
    {
        NonEmptyHasFirst => (
            "non_empty_has_first",
            ConstructiveDecidable,
            &[LemmaFact::NonEmpty],
            LemmaFact::InBounds
        )
    },
    {
        WindowLength => (
            "window_length",
            ConstructiveDecidable,
            &[LemmaFact::WindowWithinLength],
            LemmaFact::WindowLengthIsExtent
        )
    },
    {
        WindowSubrange => (
            "window_subrange",
            ConstructiveDecidable,
            &[LemmaFact::WindowWithinLength],
            LemmaFact::WindowWithinParent
        )
    },
    {
        TailLengthDecreases => (
            "tail_length_decreases",
            ConstructiveDecidable,
            &[LemmaFact::NonEmpty],
            LemmaFact::TailStrictlyShorter
        )
    },
}

impl ProofLemma {
    pub fn apply(self, established: &[LemmaFact]) -> Option<LemmaFact> {
        self.premises()
            .iter()
            .all(|premise| established.contains(premise))
            .then(|| self.conclusion())
    }

    pub fn discharging(goal: LemmaFact, established: &[LemmaFact]) -> Option<ProofLemma> {
        ProofLemma::ALL
            .into_iter()
            .find(|lemma| lemma.conclusion() == goal && lemma.apply(established) == Some(goal))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LemmaFact {
    IndexLessThanLength,
    NonEmpty,
    WindowWithinLength,
    InBounds,
    WindowLengthIsExtent,
    WindowWithinParent,
    TailStrictlyShorter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForAllInRangeFact {
    pub predicate: String,
    pub start: QuantifiedBound,
    pub end: QuantifiedBound,
}

impl ForAllInRangeFact {
    pub const ELIMINATION_FOUNDATION: proof_admission::ProofRuleFoundation =
        proof_admission::ProofRuleFoundation::ConstructiveDecidable;

    pub fn new(predicate: impl Into<String>, start: QuantifiedBound, end: QuantifiedBound) -> Self {
        Self {
            predicate: predicate.into(),
            start,
            end,
        }
    }

    pub fn over_full_extent(predicate: impl Into<String>) -> Self {
        Self::new(predicate, QuantifiedBound::Zero, QuantifiedBound::Length)
    }

    pub fn proves_element(&self, predicate: &str, index: ElementIndex) -> bool {
        if self.predicate != predicate || self.is_vacuous() {
            return false;
        }
        self.contains_index(index)
    }

    fn contains_index(&self, index: ElementIndex) -> bool {
        let literal = match index {
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
            QuantifiedBound::Length => matches!(index, ElementIndex::LiteralInBounds(_)),
        };
        lower_ok && upper_ok
    }

    pub fn is_vacuous(&self) -> bool {
        match (self.start, self.end) {
            (QuantifiedBound::Length, _) => true,
            (_, QuantifiedBound::Zero) => {
                !matches!(self.start, QuantifiedBound::Literal(start) if start < 0)
            }
            (QuantifiedBound::Literal(start), QuantifiedBound::Literal(end)) => start >= end,
            (QuantifiedBound::Zero, QuantifiedBound::Literal(end)) => end <= 0,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementIndex {
    Literal(i64),
    InBounds,
    LiteralInBounds(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantifiedBound {
    Zero,
    Literal(i64),
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
    fn discharging_finds_named_lemma_for_goal() {
        assert_eq!(
            ProofLemma::discharging(LemmaFact::InBounds, &[LemmaFact::NonEmpty]),
            Some(ProofLemma::NonEmptyHasFirst)
        );
        assert_eq!(
            ProofLemma::discharging(LemmaFact::InBounds, &[LemmaFact::IndexLessThanLength]),
            Some(ProofLemma::IndexInBounds)
        );
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
    fn full_extent_fact_proves_in_bounds_element() {
        let fact = ForAllInRangeFact::over_full_extent("Ascii");
        assert!(fact.proves_element("Ascii", ElementIndex::InBounds));
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
        assert!(!fact.proves_element("Positive", ElementIndex::Literal(0)));
        assert!(!fact.proves_element("Positive", ElementIndex::Literal(4)));
        let symbolic = ForAllInRangeFact::over_full_extent("Positive");
        assert!(!symbolic.proves_element("Positive", ElementIndex::Literal(2)));
        assert!(symbolic.proves_element("Positive", ElementIndex::LiteralInBounds(2)));
    }

    #[test]
    fn literal_in_bounds_discharges_length_bounded_ranges() {
        let tail = ForAllInRangeFact::new(
            "Checked",
            QuantifiedBound::Literal(3),
            QuantifiedBound::Length,
        );
        assert!(tail.proves_element("Checked", ElementIndex::LiteralInBounds(3)));
        assert!(tail.proves_element("Checked", ElementIndex::LiteralInBounds(9)));
        assert!(!tail.proves_element("Checked", ElementIndex::LiteralInBounds(2)));
        assert!(!tail.proves_element("Checked", ElementIndex::Literal(9)));
        assert!(!tail.proves_element("Checked", ElementIndex::InBounds));

        let head = ForAllInRangeFact::new(
            "Checked",
            QuantifiedBound::Zero,
            QuantifiedBound::Literal(5),
        );
        assert!(head.proves_element("Checked", ElementIndex::LiteralInBounds(4)));
        assert!(!head.proves_element("Checked", ElementIndex::LiteralInBounds(5)));

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
        assert!(
            ForAllInRangeFact::new("P", QuantifiedBound::Literal(3), QuantifiedBound::Zero)
                .is_vacuous()
        );
        assert!(
            ForAllInRangeFact::new("P", QuantifiedBound::Zero, QuantifiedBound::Zero).is_vacuous()
        );
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
}
