//! Payload evaluation for the shared selected-call custody walk.

use super::*;
use checked_trees::{CheckedScalarExpression, CheckedStructuralPredicatePathSegment};
use facts::IntegerRange;

pub(super) struct LiveValues<'a, 'plans> {
    pub program: &'a typed_trees::TypedTrees,
    pub semantic: &'a FactPlan,
    pub context: &'a FlowBuildContext<'plans>,
    pub state: SymbolHandle,
    pub active: HandleSpan<FlowSemanticContextRef>,
}

pub(super) trait CapturedValue: Sized {
    const REQUIRES_SELECTED_ARGUMENTS: bool;
    fn literal(expression: &ExpressionNode) -> Option<Self>;
    fn at_place(place: &CanonicalPlace, live: &LiveValues<'_, '_>) -> Option<Self>;
    fn evaluate_live(
        expression: &CheckedScalarExpression,
        symbols: &[SymbolHandle],
        live: &LiveValues<'_, '_>,
    ) -> Option<Self>;
    fn evaluate_local(
        expression: &CheckedScalarExpression,
        values: &mut CallValues<Self>,
    ) -> Option<Self>;
}

impl CapturedValue for ScalarValue {
    const REQUIRES_SELECTED_ARGUMENTS: bool = false;
    fn literal(expression: &ExpressionNode) -> Option<Self> {
        match expression {
            ExpressionNode::Integer(value) => value.value_bignum().map(Self::Integer),
            ExpressionNode::Boolean(value) => Some(Self::Boolean(*value)),
            _ => None,
        }
    }

    fn at_place(place: &CanonicalPlace, live: &LiveValues<'_, '_>) -> Option<Self> {
        crate::values::scalar_value_at_place(
            live.program,
            live.semantic,
            live.context
                .contexts
                .semantic_context_refs
                .span_or_empty(live.active)
                .iter()
                .map(|reference| live.semantic.contexts.get(reference.context)),
            place,
        )
    }

    fn evaluate_live(
        expression: &CheckedScalarExpression,
        symbols: &[SymbolHandle],
        live: &LiveValues<'_, '_>,
    ) -> Option<Self> {
        crate::values::evaluate_checked_scalar(
            expression,
            &mut crate::values::PlaceScalarValues {
                program: live.program,
                parameters: live
                    .program
                    .state_parameters(crate::find_state(live.program, live.state)?),
                symbols,
                value_at_place: |place: &CanonicalPlace| Self::at_place(place, live),
            },
        )
    }

    fn evaluate_local(
        expression: &CheckedScalarExpression,
        values: &mut CallValues<Self>,
    ) -> Option<Self> {
        crate::values::evaluate_checked_scalar(expression, values)
    }
}

impl CapturedValue for IntegerRange {
    const REQUIRES_SELECTED_ARGUMENTS: bool = true;
    fn literal(expression: &ExpressionNode) -> Option<Self> {
        let ExpressionNode::Integer(literal) = expression else {
            return None;
        };
        let value = literal.value_bignum()?;
        Some(Self {
            minimum: value.clone(),
            maximum: value,
        })
    }

    fn at_place(place: &CanonicalPlace, live: &LiveValues<'_, '_>) -> Option<Self> {
        crate::values::integer_bounds_at_place(
            live.program,
            live.semantic,
            live.context
                .contexts
                .semantic_context_refs
                .span_or_empty(live.active)
                .iter()
                .map(|reference| live.semantic.contexts.get(reference.context)),
            place,
        )
    }

    fn evaluate_live(
        expression: &CheckedScalarExpression,
        symbols: &[SymbolHandle],
        live: &LiveValues<'_, '_>,
    ) -> Option<Self> {
        let contexts = live
            .context
            .contexts
            .semantic_context_refs
            .span_or_empty(live.active)
            .iter()
            .map(|reference| reference.context)
            .collect::<Vec<_>>();
        crate::values::bounds::evaluate(
            expression,
            &mut crate::values::bounds::PlaceIntegerBounds {
                program: live.program,
                semantic: live.semantic,
                contexts: &contexts,
                parameters: live
                    .program
                    .state_parameters(crate::find_state(live.program, live.state)?),
                symbols,
            },
        )
    }

    fn evaluate_local(
        expression: &CheckedScalarExpression,
        values: &mut CallValues<Self>,
    ) -> Option<Self> {
        crate::values::bounds::evaluate(expression, values)
    }
}

impl crate::values::bounds::IntegerBoundsSource for CallValues<IntegerRange> {
    fn binding(
        &mut self,
        position: usize,
        _: typed_trees::types::PrimitiveType,
    ) -> Option<IntegerRange> {
        self.bindings.get(position)?.clone()
    }

    fn storage(
        &mut self,
        symbol: SymbolHandle,
        _: typed_trees::types::PrimitiveType,
    ) -> Option<IntegerRange> {
        self.storage
            .iter()
            .find(|(candidate, _)| *candidate == symbol)
            .map(|(_, bounds)| bounds.clone())
    }

    fn structural_field(
        &mut self,
        _: u32,
        _: &[CheckedStructuralPredicatePathSegment],
    ) -> Option<IntegerRange> {
        None
    }

    fn indexed_field(
        &mut self,
        _: u32,
        _: &[CheckedStructuralPredicatePathSegment],
        _: Option<&IntegerRange>,
    ) -> Option<IntegerRange> {
        None
    }
}
