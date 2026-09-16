//! Payload evaluation for the shared selected-call custody walk.
use super::ScalarValue;
use super::conversions::{ConversionKind, SelectedConversion};
use crate::flow::CanonicalPlace;
use crate::flow::FlowBuildContext;
use crate::flow::transfers::scalar_values::CallValues;
use arena::HandleSpan;
use checked_trees::FlowSemanticContextRef;
use checked_trees::expression::ExpressionHandle;
use checked_trees::expression::ExpressionNode;
use checked_trees::{CheckedScalarExpression, CheckedStructuralPredicatePathSegment};
use facts::FactPlan;
use facts::IntegerRange;
use numerics::bignum::BigInt;
use symbols::SymbolHandle;

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
    /// Fallback for an argument with no live snapshot: its runtime value must
    /// still satisfy the formal's declared scalar type, so that declaration —
    /// at worst the raw carrier — bounds the incoming value.
    fn formal_fallback(
        _program: &typed_trees::TypedTrees,
        _parameter: &typed_trees::signature::StateParameter,
    ) -> Option<Self> {
        None
    }
    /// Evaluate a non-call operand below authored conversions: a literal or a
    /// caller place with a live snapshot or declared storage invariant. Never
    /// a source replay of nested computation the plan did not retain.
    fn operand(
        _program: &typed_trees::TypedTrees,
        _live: &LiveValues<'_, '_>,
        _statement_index: usize,
        _expression: ExpressionHandle,
    ) -> Option<Self> {
        None
    }
    /// Fallback for a resolved receiver field with no live snapshot: declared
    /// storage invariants on frozen storage, then the raw carrier. Exact
    /// scalar values have no declared fallback.
    fn field_fallback(
        _program: &typed_trees::TypedTrees,
        _reference: typed_trees::types::TypeReferenceHandle,
        _frozen: bool,
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
                    .state_parameters(crate::semantic_calls::find_state(live.program, live.state)?),
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

    fn operand(
        program: &typed_trees::TypedTrees,
        live: &LiveValues<'_, '_>,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<Self> {
        if let Some(value) = Self::literal(program.expression_table.expression(expression)) {
            return Some(value);
        }
        let place = crate::flow::canonical_place_from_expression_in_state(
            program,
            live.state,
            statement_index,
            expression,
        )?;
        Self::at_place(&place, live)
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
            ConversionKind::Wrapping => {
                crate::values::wrapping_cast_value(conversion.target, admitted)?
            }
            ConversionKind::Trapping => source.exact_cast_value_to(target, admitted)?,
            ConversionKind::Saturating => {
                let minimum = crate::values::integer_magnitude(target.minimum_value());
                let maximum = crate::values::integer_magnitude(target.maximum_value());
                return Some(Self::Integer(value.max(minimum).min(maximum)));
            }
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
                    .state_parameters(crate::semantic_calls::find_state(live.program, live.state)?),
                symbols,
                state: live.state,
            },
        )
    }

    fn evaluate_local(
        expression: &CheckedScalarExpression,
        values: &mut CallValues<Self>,
    ) -> Option<Self> {
        crate::values::bounds::evaluate(expression, values)
    }

    fn formal_fallback(
        program: &typed_trees::TypedTrees,
        parameter: &typed_trees::signature::StateParameter,
    ) -> Option<Self> {
        let primitive = program.primitive_type_reference(parameter.type_reference)?;
        crate::values::bounds::declared_bounds(program, parameter.type_reference, primitive)
            .or_else(|| crate::values::bounds::primitive_range(primitive))
    }

    fn operand(
        program: &typed_trees::TypedTrees,
        live: &LiveValues<'_, '_>,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<Self> {
        if let Some(value) = Self::literal(program.expression_table.expression(expression)) {
            return Some(value);
        }
        let place = crate::flow::canonical_place_from_expression_in_state(
            program,
            live.state,
            statement_index,
            expression,
        )?;
        let primitive =
            program.primitive_type_reference(crate::flow::expression_type_reference_in_state(
                program,
                live.state,
                statement_index,
                expression,
            )?)?;
        let contexts = live
            .context
            .contexts
            .semantic_context_refs
            .span_or_empty(live.active)
            .iter()
            .map(|reference| reference.context)
            .collect::<Vec<_>>();
        crate::values::bounds::PlaceIntegerBounds {
            program,
            semantic: live.semantic,
            contexts: &contexts,
            parameters: program
                .state_parameters(crate::semantic_calls::find_state(program, live.state)?),
            symbols: &[],
            state: live.state,
        }
        .bounds_at_place(&place, primitive)
    }

    fn field_fallback(
        program: &typed_trees::TypedTrees,
        reference: typed_trees::types::TypeReferenceHandle,
        frozen: bool,
    ) -> Option<Self> {
        let primitive = program.primitive_type_reference(reference)?;
        // The declared storage invariant answers only while every link of the
        // receiver path is frozen; past that the carrier still bounds any
        // value a successful read can return.
        (frozen
            .then(|| crate::values::bounds::declared_bounds(program, reference, primitive))
            .flatten())
        .or_else(|| crate::values::bounds::primitive_range(primitive))
    }

    fn convert(self, conversion: &SelectedConversion) -> Option<Self> {
        let carrier = crate::values::bounds::primitive_range(conversion.target)?;
        // These branches mirror the cast arms of `bounds::evaluate`: widening
        // is transparent, wrapping keeps the interval only inside the
        // carrier, a representable normal return is the meet, and a partial
        // exact conversion keeps the meet with its proved spelling range.
        let bounds = match &conversion.kind {
            ConversionKind::Widen => self,
            ConversionKind::Wrapping => {
                if crate::values::bounds::contains(&carrier, &self) {
                    self
                } else {
                    carrier.clone()
                }
            }
            ConversionKind::Trapping => Self {
                minimum: self.minimum.max(carrier.minimum.clone()),
                maximum: self.maximum.min(carrier.maximum.clone()),
            },
            ConversionKind::Saturating => {
                let clamp = |endpoint: BigInt| {
                    endpoint
                        .max(carrier.minimum.clone())
                        .min(carrier.maximum.clone())
                };
                Self {
                    minimum: clamp(self.minimum),
                    maximum: clamp(self.maximum),
                }
            }
            ConversionKind::Exact(range) => Self {
                minimum: self.minimum.max(range.minimum.clone()),
                maximum: self.maximum.min(range.maximum.clone()),
            },
        };
        // A meet that cannot denote a normal-return value, and any interval
        // escaping the target carrier, fails the capture outright.
        crate::values::bounds::contains(&carrier, &bounds).then_some(bounds)
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
