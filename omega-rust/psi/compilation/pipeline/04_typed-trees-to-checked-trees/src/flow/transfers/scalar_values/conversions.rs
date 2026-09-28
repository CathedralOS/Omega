//! A selected call result can still be a selected call when the statement
//! wraps it in authored value conversions before landing it. Each conversion
//! is classified once against the callee's declared carrier — mirroring
//! `construct_integer_cast`'s admission order — so the captured result keeps
//! the conversion's own bound instead of dissolving into the destination's
//! carrier or vanishing entirely. A step that cannot describe a normal-return
//! value fails the capture, never a widened guess.
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use facts::IntegerRange;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::types::PrimitiveType;

/// A call nested under authored integer conversions: the call itself plus the
/// conversion steps applied to its result, innermost first. `expression` is
/// the authored call node that identifies this exact call occurrence.
pub(super) struct SelectedCall<'a> {
    pub call: &'a typed_trees::expression::TableCallExpression,
    pub expression: ExpressionHandle,
    conversions: Vec<SelectedConversion>,
}

/// One classified conversion step over the running result carrier.
pub(super) struct SelectedConversion {
    pub source: PrimitiveType,
    pub target: PrimitiveType,
    pub kind: ConversionKind,
}

pub(super) enum ConversionKind {
    /// A same-carrier retag or a total carrier inclusion leaves the captured
    /// result untouched, matching `IntegerWiden`'s lowering selection.
    Widen,
    /// Modular conversion: monotone only while the operand interval stays
    /// inside the target carrier, otherwise the full carrier is all that is
    /// known.
    Wrapping,
    /// A representable normal return keeps the meet of the operand interval
    /// and the target carrier; an empty meet describes no returning value.
    Trapping,
    /// Clamping is monotone over the operand interval's endpoints.
    Saturating,
    /// A partial exact conversion keeps the meet of the operand interval and
    /// the retained occurrence fact's proved spelling range.
    Exact(IntegerRange),
}

impl SelectedCall<'_> {
    /// Fold the captured call result through each authored conversion.
    pub(super) fn convert<Value: super::captured::CapturedValue>(
        &self,
        mut value: Value,
    ) -> Option<Value> {
        for conversion in &self.conversions {
            value = value.convert(conversion)?;
        }
        Some(value)
    }
}

/// A non-call operand under authored value conversions: the operand resolves
/// against live caller facts or its declared storage instead of a callee body.
/// This is the same conversion fold as `SelectedCall` with a different
/// operand source, reached only when the selected scalar plan retained no
/// form for the statement's source — today that is exactly the policies
/// `construct_integer_cast` cannot express, like `Saturating`.
pub(super) struct SelectedOperand {
    pub operand: ExpressionHandle,
    conversions: Vec<SelectedConversion>,
}

impl SelectedOperand {
    pub(super) fn convert<Value: super::captured::CapturedValue>(
        &self,
        mut value: Value,
    ) -> Option<Value> {
        for conversion in &self.conversions {
            value = value.convert(conversion)?;
        }
        Some(value)
    }
}

/// Peel value casts around a non-call operand. The operand must resolve to a
/// caller place or a literal — a nested computation or a call stays with the
/// selected plan/`selected_call` routes that own it. The operand's own
/// carrier starts the conversion fold, mirroring the callee return type in
/// `selected_call`.
pub(super) fn selected_operand(
    program: &typed_trees::TypedTrees,
    exact_casts: &[validation::ExactIntegerCastFact],
    state: symbols::SymbolHandle,
    statement_index: usize,
    source: ExpressionHandle,
) -> Option<SelectedOperand> {
    let mut casts = Vec::new();
    let mut expression = source;
    let operand = loop {
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Cast(cast)
                if !cast.form.is_recast() && cast.semantic_domain.is_empty() =>
            {
                casts.push((expression, cast));
                expression = cast.value;
            }
            ExpressionNode::Call(_) => return None,
            _ => break expression,
        }
    };
    if casts.is_empty() {
        return None;
    }
    let mut source_type = match program.expression_table.expression(operand) {
        ExpressionNode::Integer(literal) => match literal.landing()?.landed_type {
            numerics::literals::LandedIntegerType::I8 => PrimitiveType::I8,
            numerics::literals::LandedIntegerType::I16 => PrimitiveType::I16,
            numerics::literals::LandedIntegerType::I32 => PrimitiveType::I32,
            numerics::literals::LandedIntegerType::I64 => PrimitiveType::I64,
            numerics::literals::LandedIntegerType::U8 => PrimitiveType::U8,
            numerics::literals::LandedIntegerType::U16 => PrimitiveType::U16,
            numerics::literals::LandedIntegerType::U32 => PrimitiveType::U32,
            numerics::literals::LandedIntegerType::U64 => PrimitiveType::U64,
            numerics::literals::LandedIntegerType::Addr => return None,
        },
        _ => program.primitive_type_reference(crate::flow::expression_type_reference_in_state(
            program,
            state,
            statement_index,
            operand,
        )?)?,
    };
    crate::values::bounds::primitive_range(source_type)?;
    let mut conversions = Vec::with_capacity(casts.len());
    for (cast_expression, cast) in casts.iter().rev() {
        let target_type = program.primitive_type_reference(cast.target_type)?;
        let conversion = classify(
            *cast_expression,
            cast,
            source_type,
            target_type,
            exact_casts,
        )?;
        source_type = conversion.target;
        conversions.push(conversion);
    }
    Some(SelectedOperand {
        operand,
        conversions,
    })
}

/// Peel value casts around a selected call. A borrow recast or a
/// semantic-domain suffix retires the slice outright; a remaining non-cast,
/// non-call operand is simply not a qualified call. Every cast between the
/// call and the landing must classify against the running carrier — this
/// keeps `scalar_qualified_call_expression`'s same-carrier transparency and
/// extends it to the remaining fixed-integer cast policies.
pub(super) fn selected_call<'a>(
    program: &'a typed_trees::TypedTrees,
    exact_casts: &[validation::ExactIntegerCastFact],
    source: ExpressionHandle,
) -> Option<SelectedCall<'a>> {
    let mut casts = Vec::new();
    let mut expression = source;
    let call = loop {
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Cast(cast)
                if !cast.form.is_recast() && cast.semantic_domain.is_empty() =>
            {
                casts.push((expression, cast));
                expression = cast.value;
            }
            ExpressionNode::Call(call) => break (expression, call),
            _ => return None,
        }
    };
    let (expression, call) = call;
    if casts.is_empty() {
        return Some(SelectedCall {
            call,
            expression,
            conversions: Vec::new(),
        });
    }
    let callee = crate::semantic::calls::find_state(program, call.target_symbol)?;
    let mut source_type = program.primitive_type_reference(callee.return_type)?;
    crate::values::bounds::primitive_range(source_type)?;
    let mut conversions = Vec::with_capacity(casts.len());
    for (cast_expression, cast) in casts.iter().rev() {
        let target_type = program.primitive_type_reference(cast.target_type)?;
        let conversion = classify(
            *cast_expression,
            cast,
            source_type,
            target_type,
            exact_casts,
        )?;
        source_type = conversion.target;
        conversions.push(conversion);
    }
    Some(SelectedCall {
        call,
        expression,
        conversions,
    })
}

/// Classify one authored cast over the running result carrier, in
/// `construct_integer_cast`'s order: a same-carrier or total inclusion is a
/// widening, a partial `Exact` conversion needs its retained occurrence fact,
/// and the remaining policies keep their own result law. `Saturating` needs
/// no occurrence proof — the clamp is total — so it is admitted here where
/// the selected-scalar plan has no node for it.
fn classify(
    expression: ExpressionHandle,
    cast: &typed_trees::expression::TableCastExpression,
    source: PrimitiveType,
    target: PrimitiveType,
    exact_casts: &[validation::ExactIntegerCastFact],
) -> Option<SelectedConversion> {
    // Fixed-width integer carriers only: Addr and the non-integer primitives
    // are distinct carriers that this slice does not convert.
    crate::values::bounds::primitive_range(target)?;
    let kind = if source == target || validation::integer_widen_is_total(source, target) {
        ConversionKind::Widen
    } else {
        match cast.domain {
            ArithmeticDomain::Exact => {
                let fact = exact_casts.iter().find(|fact| {
                    fact.expression == expression
                        && fact.source_type == source
                        && fact.target_type == target
                })?;
                ConversionKind::Exact(IntegerRange {
                    minimum: fact.minimum.clone(),
                    maximum: fact.maximum.clone(),
                })
            }
            ArithmeticDomain::Wrapping => ConversionKind::Wrapping,
            ArithmeticDomain::Trapping => ConversionKind::Trapping,
            ArithmeticDomain::Saturating => ConversionKind::Saturating,
        }
    };
    Some(SelectedConversion {
        source,
        target,
        kind,
    })
}
