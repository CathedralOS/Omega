//! Contents bounds at one completed read cut. Geometry comes from the declared
//! collection, never from whichever element facts survived. Every possible
//! element contributes to the hull; a missing element contributes its carrier.
//! These consequences do not grant initialization, index formation or access.

use crate::facts::boolean_predicates::{self, BoundPredicate};
use facts::{FactContextHandle, FactPlan, IntegerRange, PlaceSegment, ProgramPoint};
use symbols::SymbolHandle;
use typed_trees::types::PrimitiveType;
use validation::CanonicalPlace;

pub(crate) struct CurrentIntegerBounds<'a> {
    program: &'a dyn validation::CheckingInput,
    semantic: &'a FactPlan,
    contexts: &'a [FactContextHandle],
    state: SymbolHandle,
    predicates: Vec<BoundPredicate>,
}

impl<'a> CurrentIntegerBounds<'a> {
    pub(crate) fn new(
        program: &'a dyn validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        semantic: &'a FactPlan,
        contexts: &'a [FactContextHandle],
        state: SymbolHandle,
        point: ProgramPoint,
    ) -> Self {
        Self {
            program,
            semantic,
            contexts,
            state,
            predicates: boolean_predicates::retained(program, operators, semantic, contexts, point),
        }
    }

    pub(crate) fn at_place(
        &self,
        place: &CanonicalPlace,
        primitive: PrimitiveType,
    ) -> Option<IntegerRange> {
        // A proof search limit declines the whole projection, never a suffix
        // of the candidates. This bounds work without asserting partial coverage.
        self.projected(place, primitive, &mut 4096)
    }

    fn projected(
        &self,
        place: &CanonicalPlace,
        primitive: PrimitiveType,
        budget: &mut usize,
    ) -> Option<IntegerRange> {
        *budget = budget.checked_sub(1)?;
        let Some(position) = place
            .segments
            .iter()
            .position(|segment| matches!(segment, PlaceSegment::Index { .. }))
        else {
            return self.exact(place, primitive);
        };
        let mut collection = place.clone();
        collection.segments.truncate(position);
        let extent =
            validation::closed_collection_length_at_place(self.program, self.state, &collection)?;
        if extent == 0 || usize::try_from(extent).ok()? > *budget {
            return None;
        }
        let mut hull: Option<IntegerRange> = None;
        for index in 0..extent {
            let mut element = place.clone();
            element.segments[position] = PlaceSegment::FixedIndex {
                index: usize::try_from(index).ok()?,
            };
            let bounds = self.projected(&element, primitive, budget)?;
            hull = Some(match hull {
                Some(previous) => IntegerRange {
                    minimum: previous.minimum.min(bounds.minimum),
                    maximum: previous.maximum.max(bounds.maximum),
                },
                None => bounds,
            });
        }
        hull
    }

    fn exact(&self, place: &CanonicalPlace, primitive: PrimitiveType) -> Option<IntegerRange> {
        let contexts = || {
            self.contexts
                .iter()
                .map(|context| self.semantic.contexts.get(*context))
        };
        let mut bounds = validation::carrier_integer_range(primitive)?;
        if let Some(snapshot) =
            super::integer_bounds_at_place(self.program, self.semantic, contexts(), place)
        {
            bounds.minimum = bounds.minimum.max(snapshot.minimum);
            bounds.maximum = bounds.maximum.min(snapshot.maximum);
        }
        for predicate in &self.predicates {
            if let Some(consequence) =
                predicate.integer_interval_at_place(place, primitive, |place| {
                    super::scalar_value_at_place(self.program, self.semantic, contexts(), place)
                })
            {
                bounds.minimum = bounds.minimum.max(consequence.minimum);
                bounds.maximum = bounds.maximum.min(consequence.maximum);
            }
        }
        (bounds.minimum <= bounds.maximum).then_some(bounds)
    }
}
