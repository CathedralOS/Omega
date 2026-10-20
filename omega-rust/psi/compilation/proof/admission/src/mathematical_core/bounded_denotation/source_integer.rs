//! A source elaborator and its bounded derivations must use one integer
//! vocabulary, not independently minted carriers with similar spellings.
//! This state retains the existing denoter's interning and arithmetic laws
//! while lending its arena and append-only signature to the source producer.
//! Every result remains an untrusted term for ordinary kernel checking.

use super::{BoundedDenotationError, Denotation};
use crate::mathematical_core::{Declaration, TermArena, TermHandle};
use numerics::bignum::BigInt;

mod closed;
mod open;

#[derive(Clone, Copy)]
pub struct SourceIntegerVocabulary {
    pub carrier: u32,
    pub less_than: u32,
    pub less_or_equal: u32,
    pub add: u32,
    pub subtract: u32,
    pub multiply: u32,
}

/// One producer arena's persistent integer theory. Use this with the same
/// arena and append-only declaration list throughout the elaboration. Stored
/// handles are local to that pair, just like handles in a term telescope.
pub struct SourceIntegerTheory {
    state: Denotation,
    vocabulary: SourceIntegerVocabulary,
}

impl SourceIntegerTheory {
    pub fn new(
        arena: &mut TermArena,
        declarations: &mut Vec<Declaration>,
    ) -> Result<Self, BoundedDenotationError> {
        let mut state = Denotation::new_in(std::mem::take(arena), std::mem::take(declarations));
        let vocabulary = (|| {
            Ok(SourceIntegerVocabulary {
                carrier: state.integer()?,
                less_than: state.integer_less_than()?,
                less_or_equal: state.integer_less_or_equal()?,
                add: state.add_operation()?,
                subtract: state.subtract_operation()?,
                multiply: state.multiply_operation()?,
            })
        })();
        std::mem::swap(arena, &mut state.arena);
        std::mem::swap(declarations, &mut state.declarations);
        Ok(Self {
            state,
            vocabulary: vocabulary?,
        })
    }

    /// The fixed integer vocabulary and audited arithmetic laws available to
    /// the source safety receiver. This is a semantic roster, not a scan of
    /// newly appended declarations: external axioms and temporary parameters
    /// can never acquire a grant by sharing this signature.
    pub fn source_safety_roster(&self) -> Vec<u32> {
        let state = &self.state;
        let mut positions = state
            .integer
            .into_iter()
            .chain(state.integer_less_than)
            .chain(state.integer_less_or_equal)
            .chain(state.integer_laws.values().copied())
            .collect::<Vec<_>>();
        positions.extend(state.binary_numerals.source_safety_roster());
        positions.extend(state.addition.source_safety_roster());
        positions.extend(state.subtraction.source_safety_roster());
        positions.extend(state.multiplication.source_safety_roster());
        positions.sort_unstable();
        positions.dedup();
        positions
    }

    pub fn vocabulary(&self) -> SourceIntegerVocabulary {
        self.vocabulary
    }

    pub fn literal(
        &mut self,
        arena: &mut TermArena,
        declarations: &mut Vec<Declaration>,
        value: BigInt,
    ) -> Result<TermHandle, BoundedDenotationError> {
        self.with(arena, declarations, |state| state.math_literal(value))
    }

    /// Produce evidence for a closed source predicate in this exact integer
    /// vocabulary. Unsupported/open or false goals yield no term. The caller
    /// must check the resulting term at its original goal and retain the full
    /// signature/assumption closure; this is not an admission verdict.
    pub fn closed_predicate(
        &mut self,
        arena: &mut TermArena,
        declarations: &mut Vec<Declaration>,
        goal: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        self.with(arena, declarations, |state| {
            state.closed_source_predicate(goal, 0)
        })
    }

    /// A proof function over the supplied predicate/IH telescope. Value
    /// parameters must already be closed terms in this same signature.
    /// Assumptions become lambda binders, never new signature axioms.
    pub fn predicate_from_assumptions(
        &mut self,
        arena: &mut TermArena,
        declarations: &mut Vec<Declaration>,
        goal: TermHandle,
        assumptions: &[TermHandle],
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        self.with(arena, declarations, |state| {
            state.source_assumption_predicate(goal, assumptions)
        })
    }

    fn with<T>(
        &mut self,
        arena: &mut TermArena,
        declarations: &mut Vec<Declaration>,
        produce: impl FnOnce(&mut Denotation) -> Result<T, BoundedDenotationError>,
    ) -> Result<T, BoundedDenotationError> {
        std::mem::swap(arena, &mut self.state.arena);
        std::mem::swap(declarations, &mut self.state.declarations);
        let result = produce(&mut self.state);
        std::mem::swap(arena, &mut self.state.arena);
        std::mem::swap(declarations, &mut self.state.declarations);
        result
    }
}
