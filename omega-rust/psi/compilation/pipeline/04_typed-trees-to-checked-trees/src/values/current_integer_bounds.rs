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

pub(crate) enum IntegerReadStep {
    Static(checked_trees::CheckedStructuralPredicatePathSegment),
    Index(Option<IntegerRange>),
}

impl<'a> CurrentIntegerBounds<'a> {
    pub(crate) fn at_statement(
        program: &'a dyn validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        semantic: &'a FactPlan,
        contexts: &'a [FactContextHandle],
        state: SymbolHandle,
        statement_index: usize,
    ) -> Option<Self> {
        let (machine, _) = validation::find_state(program, state)?;
        Some(Self::new(
            program,
            operators,
            semantic,
            contexts,
            state,
            ProgramPoint::Statement {
                machine_symbol: machine.symbol,
                state_symbol: state,
                statement_index,
            },
        ))
    }
    pub(crate) fn selected_read(
        &self,
        root: SymbolHandle,
        reference: typed_trees::types::TypeReferenceHandle,
        mutable: bool,
        steps: &[IntegerReadStep],
        primitive: PrimitiveType,
    ) -> Option<IntegerRange> {
        self.selected_path(
            root,
            reference,
            mutable,
            steps,
            &mut Vec::new(),
            primitive,
            &mut 4096,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn selected_path(
        &self,
        root: SymbolHandle,
        reference: typed_trees::types::TypeReferenceHandle,
        mutable: bool,
        steps: &[IntegerReadStep],
        path: &mut Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
        primitive: PrimitiveType,
        budget: &mut usize,
    ) -> Option<IntegerRange> {
        *budget = budget.checked_sub(1)?;
        let Some((step, tail)) = steps.split_first() else {
            let (segments, selected, _) =
                super::scalar::structural_fields::resolve_structural_path(
                    self.program,
                    reference,
                    mutable,
                    path,
                    &mut |_, _| Some(()),
                )?;
            if self.program.primitive_type_reference(selected) != Some(primitive) {
                return None;
            }
            return self.exact(
                &CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(root),
                    segments,
                },
                primitive,
            );
        };
        match step {
            IntegerReadStep::Static(step) => {
                path.push(step.clone());
                let result =
                    self.selected_path(root, reference, mutable, tail, path, primitive, budget);
                path.pop();
                result
            }
            IntegerReadStep::Index(selector) => {
                let (segments, _, _) = super::scalar::structural_fields::resolve_structural_path(
                    self.program,
                    reference,
                    mutable,
                    path,
                    &mut |_, _| Some(()),
                )?;
                let collection = CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(root),
                    segments,
                };
                let extent = validation::closed_collection_length_at_place(
                    self.program,
                    self.state,
                    &collection,
                )?;
                let last = extent.checked_sub(1)?;
                let zero = numerics::bignum::BigInt::from_u64(0);
                let last_value = numerics::bignum::BigInt::from_u64(last);
                let (first, last) = match selector {
                    Some(selector) => (
                        selector.minimum.clone().max(zero).to_u64()?,
                        selector.maximum.clone().min(last_value).to_u64()?,
                    ),
                    None => (0, last),
                };
                if first > last || usize::try_from(last - first + 1).ok()? > *budget {
                    return None;
                }
                let mut hull: Option<IntegerRange> = None;
                for index in first..=last {
                    path.push(
                        checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(index),
                    );
                    let bounds =
                        self.selected_path(root, reference, mutable, tail, path, primitive, budget);
                    path.pop();
                    let bounds = bounds?;
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
        }
    }
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
        // A live observation of this exact dynamic selection is independent
        // of the per-slot coverage. Preserve it even when the collection's
        // full extent is unavailable or another slot has only carrier bounds.
        let exact = self.exact(place, primitive);
        let projected = self.projected(place, primitive, &mut 4096);
        match (exact, projected) {
            (Some(exact), Some(projected)) => {
                let minimum = exact.minimum.max(projected.minimum);
                let maximum = exact.maximum.min(projected.maximum);
                (minimum <= maximum).then_some(IntegerRange { minimum, maximum })
            }
            (exact, projected) => exact.or(projected),
        }
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
        let trace = std::env::var("OMEGA_TRACE_READ_BOUNDS")
            .ok()
            .is_some_and(|name| {
                validation::find_state(self.program, self.state)
                    .is_some_and(|(machine, _)| machine.name.as_str() == name)
            });
        if trace {
            eprintln!(
                "READ-BOUNDS exact {:?} {:?} predicates {:?}",
                place, primitive, self.predicates
            );
        }
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
                if trace {
                    eprintln!("READ-BOUNDS consequence {:?}", consequence);
                }
                bounds.minimum = bounds.minimum.max(consequence.minimum);
                bounds.maximum = bounds.maximum.min(consequence.maximum);
            }
        }
        (bounds.minimum <= bounds.maximum).then_some(bounds)
    }
}
