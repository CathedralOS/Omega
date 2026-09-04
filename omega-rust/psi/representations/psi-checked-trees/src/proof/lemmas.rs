use psi_arena::Arena;
use psi_symbols::SymbolHandle;

/// A reusable length/bounds/window proof lemma recorded as durable evidence in
/// the checked trees. The prover applies these so individual check sites do not
/// re-derive the same slice/sequence arithmetic.
///
/// This mirrors the checker-internal proof lemma but lives in the
/// representation layer so the shape is preserved through to checked trees
/// without depending on checker implementation details.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProofLemmaKind {
    /// `index < length` implies the offset is in bounds.
    #[default]
    IndexInBounds,
    /// A non-empty collection has a valid offset `0`.
    NonEmptyHasFirst,
    /// `items[start..end]` has length `end - start` when `start <= end <= length`.
    WindowLength,
    /// `items[start..end]` is contained within `items`.
    WindowSubrange,
    /// `items[1..]` is strictly shorter than a non-empty `items`.
    TailLengthDecreases,
}

/// A recorded application of a proof lemma to a concrete collection symbol.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProofLemmaFact {
    pub kind: ProofLemmaKind,
    /// The collection (slice/array/sequence) the lemma is applied to.
    pub collection_symbol: SymbolHandle,
}

/// A `for all i in start..end, P(i)` proof fact over a sequence/slice.
///
/// The binder is proof-only and never lowers to a runtime loop. It captures the
/// narrow sequence-style invariants text/slice domains need, such as "every
/// element of the window is in the predicate domain".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QuantifiedRangeFact {
    /// The collection the quantifier ranges over.
    pub collection_symbol: SymbolHandle,
    /// The per-element predicate domain/contract each index must satisfy.
    pub predicate_symbol: SymbolHandle,
    /// Inclusive lower bound of the quantified index range.
    pub start: QuantifiedBoundFact,
    /// Exclusive upper bound of the quantified index range.
    pub end: QuantifiedBoundFact,
}

/// A bound of a quantified index range.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum QuantifiedBoundFact {
    /// The literal `0`.
    #[default]
    Zero,
    /// A concrete literal offset.
    Literal(i64),
    /// The (symbolic) length of the collection.
    Length,
}

/// Durable root holding reusable lemma applications and quantified facts so the
/// shape survives into the checked trees alongside other proof evidence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LemmaFacts {
    pub lemmas: Arena<ProofLemmaFact>,
    pub quantified: Arena<QuantifiedRangeFact>,
}

impl LemmaFacts {
    pub fn with_roots(
        lemmas: Arena<ProofLemmaFact>,
        quantified: Arena<QuantifiedRangeFact>,
    ) -> Self {
        Self { lemmas, quantified }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lemma_facts_constructor_keeps_roots_explicit() {
        let lemmas = Arena::<ProofLemmaFact>::with_capacity(1);
        let quantified = Arena::<QuantifiedRangeFact>::with_capacity(2);

        let facts = LemmaFacts::with_roots(lemmas.clone(), quantified.clone());

        assert_eq!(facts.lemmas, lemmas);
        assert_eq!(facts.quantified, quantified);
    }

    #[test]
    fn quantified_bound_defaults_to_zero() {
        assert_eq!(QuantifiedBoundFact::default(), QuantifiedBoundFact::Zero);
        assert_eq!(ProofLemmaKind::default(), ProofLemmaKind::IndexInBounds);
    }
}
