//! A selected call result can still be a selected call when the statement
//! wraps it in authored value conversions before landing it. Each conversion
//! is classified once against the callee's declared carrier. A same-carrier
//! retag or total inclusion keeps the captured result; a partial conversion
//! is the arithmetic engine's decision, which flow does not read, so it fails
//! the capture, never a widened guess.
use super::captured::CapturedValue;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::PrimitiveType;

/// A call nested under authored integer conversions: the call itself plus the
/// conversion steps applied to its result, innermost first. `expression` is
/// the authored call node that identifies this exact call occurrence.
pub(super) struct SelectedCall<'a> {
    pub call: &'a typed_trees::expression::TableCallExpression,
    pub expression: ExpressionHandle,
    conversions: Vec<SelectedConversion>,
}

/// One conversion step over the running result carrier: a same-carrier retag
/// or a total carrier inclusion, matching `IntegerWiden`'s lowering selection.
pub(super) struct SelectedConversion {
    pub source: PrimitiveType,
    pub target: PrimitiveType,
}

impl SelectedCall<'_> {
    /// Fold the captured call result through each authored conversion.
    pub(super) fn convert<Value: CapturedValue>(&self, value: Value) -> Option<Value> {
        convert_through(&self.conversions, value)
    }
}

/// The authored-conversion fold both selected operands share: peel each
/// conversion in order, failing on the first that cannot apply.
fn convert_through<Value: CapturedValue>(
    conversions: &[SelectedConversion],
    mut value: Value,
) -> Option<Value> {
    for conversion in conversions {
        value = value.convert(conversion)?;
    }
    Some(value)
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
    pub(super) fn convert<Value: CapturedValue>(&self, value: Value) -> Option<Value> {
        convert_through(&self.conversions, value)
    }
}

/// Peel value casts around a non-call operand. The operand must resolve to a
/// caller place or a literal — a nested computation or a call stays with the
/// selected plan/`selected_call` routes that own it. The operand's own
/// carrier starts the conversion fold, mirroring the callee return type in
/// `selected_call`.

/// Peel the transparent-cast prefix of `source`: non-recast, non-domain casts
/// accumulate until the first non-cast expression, which is returned with the
/// accumulated chain.
fn peel_transparent_casts<'a>(
    program: &'a dyn ::validation::CheckingInput,
    source: ExpressionHandle,
) -> Option<(
    ExpressionHandle,
    Vec<(
        ExpressionHandle,
        &'a typed_trees::expression::TableCastExpression,
    )>,
)> {
    let mut casts = Vec::new();
    let mut expression = source;
    loop {
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
            _ => return Some((expression, casts)),
        }
    }
}

/// Classify every cast in a peeled chain, advancing the running carrier
/// through each conversion's target.
fn classify_cast_chain(
    program: &dyn ::validation::CheckingInput,
    casts: &[(
        ExpressionHandle,
        &typed_trees::expression::TableCastExpression,
    )],
    mut source_type: PrimitiveType,
) -> Option<Vec<SelectedConversion>> {
    validation::carrier_integer_range(source_type)?;
    let mut conversions = Vec::with_capacity(casts.len());
    for (_, cast) in casts.iter().rev() {
        let target_type = program.primitive_type_reference(cast.target_type)?;
        let conversion = classify(source_type, target_type)?;
        source_type = conversion.target;
        conversions.push(conversion);
    }
    Some(conversions)
}

pub(super) fn selected_operand(
    program: &dyn ::validation::CheckingInput,
    state: symbols::SymbolHandle,
    statement_index: usize,
    source: ExpressionHandle,
) -> Option<SelectedOperand> {
    let (operand, casts) = peel_transparent_casts(program, source)?;
    if casts.is_empty()
        || matches!(
            program.expression_table.expression(operand),
            ExpressionNode::Call(_)
        )
    {
        return None;
    }
    let source_type = match program.expression_table.expression(operand) {
        ExpressionNode::Integer(literal) => {
            PrimitiveType::non_address_landed_integer(literal.landing()?.landed_type)?
        }
        _ => program.primitive_type_reference(validation::expression_type_reference_in_state(
            program,
            state,
            statement_index,
            operand,
        )?)?,
    };
    let conversions = classify_cast_chain(program, &casts, source_type)?;
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
/// extends it to occurrence-proved fixed-integer carrier conversions.
pub(super) fn selected_call<'a>(
    program: &'a dyn ::validation::CheckingInput,
    source: ExpressionHandle,
) -> Option<SelectedCall<'a>> {
    let (expression, casts) = peel_transparent_casts(program, source)?;
    let call = match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => call,
        _ => return None,
    };
    if casts.is_empty() {
        return Some(SelectedCall {
            call,
            expression,
            conversions: Vec::new(),
        });
    }
    let callee = program.state_by_symbol(call.target_symbol)?;
    let source_type = program.primitive_type_reference(callee.return_type)?;
    let conversions = classify_cast_chain(program, &casts, source_type)?;
    Some(SelectedCall {
        call,
        expression,
        conversions,
    })
}

/// Classify one authored cast over the running result carrier, in
/// `construct_integer_cast`'s order: a same-carrier retag or a total inclusion
/// is a widening. A partial conversion has no flow-side value.
fn classify(source: PrimitiveType, target: PrimitiveType) -> Option<SelectedConversion> {
    // Fixed-width integer carriers only: Addr and the non-integer primitives
    // are distinct carriers that this slice does not convert.
    validation::carrier_integer_range(target)?;
    (source == target || validation::integer_widen_is_total(source, target))
        .then_some(SelectedConversion { source, target })
}
