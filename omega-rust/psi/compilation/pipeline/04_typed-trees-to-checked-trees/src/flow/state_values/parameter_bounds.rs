//! Conservative, finite joins for scalar state parameters.
//!
//! Union alone enumerates a counter's values one by one: Squalr's byte-comparison
//! loop kept the flow builder running for hundreds of thousands of sweeps. Preserve small
//! joins exactly, then widen growing endpoints to authored thresholds or the
//! integer carrier. Each result still contains every previously delivered range;
//! this cannot prove a stronger proposition than the original union. Unknown
//! input is absorbing, and widening never changes independent qualifications.
//!
//! These rows belong to the unpublished flow fixed point. Growth counts are
//! scheduling scratch, not proof evidence; only a changed interval dirties a
//! destination. Final materialization and the builder's fail-closed fallback
//! remain responsible for publishing settled facts.

use facts::IntegerRange;
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, state::State};

const WIDENING_DELAY: u8 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ParameterBounds {
    pub(super) symbol: SymbolHandle,
    pub(super) bounds: Option<IntegerRange>,
    growth: u8,
}

impl ParameterBounds {
    pub(super) fn new(symbol: SymbolHandle, bounds: Option<IntegerRange>) -> Self {
        Self {
            symbol,
            bounds,
            growth: 0,
        }
    }

    pub(super) fn height(&self, threshold_count: usize) -> usize {
        if self.bounds.is_none() {
            return 0;
        }
        // Delayed exact joins, finite outward movement on both axes, then loss
        // to unknown. No narrowing phase can revisit a threshold.
        threshold_count
            .saturating_add(1)
            .saturating_mul(2)
            .saturating_add(usize::from(WIDENING_DELAY))
            .saturating_add(1)
    }

    pub(super) fn join(
        &mut self,
        program: &TypedTrees,
        destination: &State,
        incoming: Option<&IntegerRange>,
        thresholds: &[BigInt],
    ) -> bool {
        let Some(previous) = self.bounds.as_ref() else {
            return false;
        };
        let Some(incoming) = incoming else {
            self.bounds = None;
            return true;
        };
        let mut joined = IntegerRange {
            minimum: previous.minimum.clone().min(incoming.minimum.clone()),
            maximum: previous.maximum.clone().max(incoming.maximum.clone()),
        };
        if &joined == previous {
            return false;
        }
        self.growth = self.growth.saturating_add(1);
        if self.growth >= WIDENING_DELAY {
            let carrier = program
                .state_parameters(destination)
                .iter()
                .find(|parameter| parameter.symbol == self.symbol)
                .and_then(|parameter| program.primitive_type_reference(parameter.type_reference))
                .and_then(crate::values::bounds::primitive_range);
            let Some(carrier) = carrier.filter(|carrier| {
                carrier.minimum <= joined.minimum && joined.maximum <= carrier.maximum
            }) else {
                // An unresolved carrier or inconsistent incoming bound supplies
                // no finite widening ceiling. Forget it rather than truncate a
                // delivered value or infer a carrier from source spelling.
                self.bounds = None;
                return true;
            };
            if joined.minimum < previous.minimum {
                joined.minimum = thresholds
                    .iter()
                    .rev()
                    .find(|threshold| {
                        carrier.minimum <= **threshold && **threshold <= joined.minimum
                    })
                    .cloned()
                    .unwrap_or(carrier.minimum);
            }
            if joined.maximum > previous.maximum {
                joined.maximum = thresholds
                    .iter()
                    .find(|threshold| {
                        joined.maximum <= **threshold && **threshold <= carrier.maximum
                    })
                    .cloned()
                    .unwrap_or(carrier.maximum);
            }
        }
        self.bounds = Some(joined);
        true
    }
}
