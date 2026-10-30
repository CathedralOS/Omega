//! Payload evaluation for the shared selected-call custody walk.
use validation::CanonicalPlace;

use super::ScalarValue;
use super::conversions::{ConversionKind, SelectedConversion};

use crate::flow::FlowBuildContext;
use crate::flow::transfers::scalar_values::CallValues;
use arena::HandleSpan;
use checked_trees::FlowSemanticContextRef;
use checked_trees::expression::ExpressionHandle;
use checked_trees::expression::ExpressionNode;
use checked_trees::{CheckedScalarExpression, CheckedStructuralPredicatePathSegment};
use facts::FactPlan;
use facts::IntegerRange;
use symbols::SymbolHandle;

pub(super) struct LiveValues<'a, 'plans> {
    pub program: &'a dyn ::validation::CheckingInput,
    pub semantic: &'a FactPlan,
    pub context: &'a FlowBuildContext<'plans>,
    pub state: SymbolHandle,
    pub active: HandleSpan<FlowSemanticContextRef>,
}

impl LiveValues<'_, '_> {
    /// Each active semantic context, resolved against the plan's arena.
    fn active_contexts(&self) -> impl Iterator<Item = &facts::FactContext> + '_ {
        self.context
            .contexts
            .semantic_context_refs
            .span_or_empty(self.active)
            .iter()
            .map(|reference| self.semantic.contexts.get(reference.context))
    }
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
    fn boolean(expression: &CheckedScalarExpression, values: &mut CallValues<Self>)
    -> Option<bool>;
    fn join(self, other: Self) -> Option<Self>;
    /// Fallback for an argument with no live snapshot: its runtime value must
    /// still satisfy the formal's declared scalar type, so that declaration —
    /// at worst the raw carrier — bounds the incoming value.
    fn formal_fallback(
        _program: &dyn ::validation::CheckingInput,
        _parameter: &typed_trees::signature::StateParameter,
    ) -> Option<Self> {
        None
    }
    /// The value held at the place `expression` names in `live`'s state.
    fn value_at_expression(
        program: &dyn ::validation::CheckingInput,
        live: &LiveValues<'_, '_>,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<Self> {
        let place = validation::canonical_place_from_expression_in_state(
            program,
            live.state,
            statement_index,
            expression,
        )?;
        Self::at_place(&place, live)
    }
    /// Evaluate a non-call operand below authored conversions: a literal or a
    /// caller place with a live snapshot or declared storage invariant. Never
    /// a source replay of nested computation the plan did not retain.
    fn operand(
        _program: &dyn ::validation::CheckingInput,
        _live: &LiveValues<'_, '_>,
        _statement_index: usize,
        _expression: ExpressionHandle,
    ) -> Option<Self> {
        None
    }
    /// Apply one authored conversion step to a captured call result. A step
    /// that cannot describe a normal-return value fails the capture, never a
    /// guess at the destination carrier.
    fn convert(self, conversion: &SelectedConversion) -> Option<Self>;
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
            live.active_contexts(),
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
                    .state_parameters(live.program.state_by_symbol(live.state)?),
                symbols,
                value_at_place: |place: &CanonicalPlace, _observation| Self::at_place(place, live),
            },
        )
    }

    fn evaluate_local(
        expression: &CheckedScalarExpression,
        values: &mut CallValues<Self>,
    ) -> Option<Self> {
        crate::values::evaluate_checked_scalar(expression, values)
    }

    fn boolean(
        expression: &CheckedScalarExpression,
        values: &mut CallValues<Self>,
    ) -> Option<bool> {
        match Self::evaluate_local(expression, values)? {
            Self::Boolean(value) => Some(value),
            _ => None,
        }
    }

    fn join(self, other: Self) -> Option<Self> {
        (self == other).then_some(self)
    }

    fn operand(
        program: &dyn ::validation::CheckingInput,
        live: &LiveValues<'_, '_>,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<Self> {
        if let Some(value) = Self::literal(program.expression_table.expression(expression)) {
            return Some(value);
        }
        Self::value_at_expression(program, live, statement_index, expression)
    }

    fn convert(self, conversion: &SelectedConversion) -> Option<Self> {
        let ScalarValue::Integer(value) = self else {
            return None;
        };
        if conversion.source == conversion.target {
            return Some(Self::Integer(value));
        }
        let source = crate::values::integer_type(conversion.source)?;
        let target = crate::values::integer_type(conversion.target)?;
        // The callee's declared carrier already constrained the captured
        // magnitude; admission failing here means the snapshot never denoted
        // a source value, so there is nothing to convert.
        let admitted = crate::values::admitted_integer(source, &value)?;
        let converted = match &conversion.kind {
            ConversionKind::Widen => source.widen_value_to(target, admitted)?,
            ConversionKind::Exact(range) => {
                // The retained occurrence fact proved the spelling's range;
                // a captured value outside it never denoted this conversion's
                // normal return.
                if value < range.minimum || value > range.maximum {
                    return None;
                }
                source.exact_cast_value_to(target, admitted)?
            }
        };
        Some(Self::Integer(crate::values::integer_magnitude(converted)))
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
            live.active_contexts(),
            place,
        )
    }

    fn evaluate_live(
        expression: &CheckedScalarExpression,
        symbols: &[SymbolHandle],
        live: &LiveValues<'_, '_>,
    ) -> Option<Self> {
        let contexts = crate::flow::active_context_handles(live.context, live.active);
        validation::selected_scalar_interval(
            expression,
            &mut crate::values::LivePlaceIntervals {
                program: live.program,
                semantic: live.semantic,
                contexts: &contexts,
                parameters: live
                    .program
                    .state_parameters(live.program.state_by_symbol(live.state)?),
                symbols,
                state: live.state,
            },
        )
    }

    fn evaluate_local(
        expression: &CheckedScalarExpression,
        values: &mut CallValues<Self>,
    ) -> Option<Self> {
        validation::selected_scalar_interval(expression, values)
    }

    fn boolean(
        expression: &CheckedScalarExpression,
        values: &mut CallValues<Self>,
    ) -> Option<bool> {
        let CheckedScalarExpression::Boolean(expression) = expression else {
            return None;
        };
        validation::selected_boolean_decision(expression, values)
    }

    fn join(self, other: Self) -> Option<Self> {
        Some(Self {
            minimum: self.minimum.min(other.minimum),
            maximum: self.maximum.max(other.maximum),
        })
    }

    fn formal_fallback(
        program: &dyn ::validation::CheckingInput,
        parameter: &typed_trees::signature::StateParameter,
    ) -> Option<Self> {
        let primitive = program.primitive_type_reference(parameter.type_reference)?;
        validation::frozen_declared_integer_range(program, parameter.type_reference, primitive)
            .or_else(|| validation::carrier_integer_range(primitive))
    }

    fn operand(
        program: &dyn ::validation::CheckingInput,
        live: &LiveValues<'_, '_>,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<Self> {
        if let Some(value) = Self::literal(program.expression_table.expression(expression)) {
            return Some(value);
        }
        let place = validation::canonical_place_from_expression_in_state(
            program,
            live.state,
            statement_index,
            expression,
        )?;
        let primitive =
            program.primitive_type_reference(validation::expression_type_reference_in_state(
                program,
                live.state,
                statement_index,
                expression,
            )?)?;
        let contexts = crate::flow::active_context_handles(live.context, live.active);
        crate::values::LivePlaceIntervals {
            program,
            semantic: live.semantic,
            contexts: &contexts,
            parameters: program.state_parameters(program.state_by_symbol(live.state)?),
            symbols: &[],
            state: live.state,
        }
        .interval_at_place(&place, primitive)
    }

    fn convert(self, conversion: &SelectedConversion) -> Option<Self> {
        let carrier = validation::carrier_integer_range(conversion.target)?;
        // Widening preserves the captured bounds. A partial conversion keeps
        // the meet with its proved spelling range, independently of the
        // arithmetic policy attached to its result.
        let bounds = match &conversion.kind {
            ConversionKind::Widen => self,
            ConversionKind::Exact(range) => Self {
                minimum: self.minimum.max(range.minimum.clone()),
                maximum: self.maximum.min(range.maximum.clone()),
            },
        };
        // A meet that cannot denote a normal-return value, and any interval
        // escaping the target carrier, fails the capture outright.
        validation::integer_range_contains(&carrier, &bounds).then_some(bounds)
    }
}

impl validation::SelectedScalarIntervalSource for CallValues<IntegerRange> {
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
        position: u32,
        path: &[CheckedStructuralPredicatePathSegment],
    ) -> Option<IntegerRange> {
        self.fields
            .iter()
            .find(|(candidate_position, candidate, _)| {
                *candidate_position == position && candidate.as_slice() == path
            })
            .map(|(_, _, bounds)| bounds.clone())
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
