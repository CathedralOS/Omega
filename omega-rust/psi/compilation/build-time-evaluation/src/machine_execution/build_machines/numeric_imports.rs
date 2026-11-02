//! Install already-selected primitive Float meanings for a pure Terminal job.
//!
//! Checking preserves the public requirement call and its call-local result
//! policy. The host supplies only the admitted raw IEEE operation, through the
//! ordinary instance adapter boundary; saturation/trapping remain Terminal
//! operations. No source name chooses a provider, and no unselected import gets
//! a binding. Shared numeric semantics own rounding and NaN behavior.
//!
//! ConstEvaluable admission has already checked every occurrence in the closed
//! job: an unsupplied occurrence remains a bodyless requirement and refuses.
//! Only then may occurrences of the same primitive declaration share an import;
//! one selected occurrence cannot authorize another occurrence by itself.

use numerics::float_semantics::{FloatFormat, FloatSemantics};
use semantic_vocabulary::{IeeeFloatValue, ScalarType};
use terminal_interpreter::{
    TerminalEffectRejection, TerminalInstance, TerminalProgram, TerminalScalarImport,
    TerminalScalarValue,
};
use typed_trees::expression::BinaryOperator;

pub(super) fn install(
    program: &TerminalProgram<'_>,
    instance: &mut TerminalInstance<'_, '_>,
    typed: &typed_trees::TypedTrees,
    operators: &[crate::SelectedBuildTimeBinaryOperator],
) -> Result<(), String> {
    let identities = operators
        .iter()
        .map(|row| {
            typed
                .normalized_hermetic_symbol_identity(row.requirement)
                .map(|identity| (identity, row))
                .map_err(|error| format!("selected numeric requirement identity: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for import in program.imports() {
        let Some(declaration) = &import.declaration else {
            continue;
        };
        let identity = program
            .module()
            .identities
            .text(declaration.source.semantic_declaration);
        let mut selected = identities
            .iter()
            .filter(|(candidate, _)| Some(candidate.as_str()) == identity);
        let Some((_, first)) = selected.next() else {
            continue;
        };
        if selected
            .any(|(_, other)| other.operation != first.operation || other.format != first.format)
        {
            return Err(
                "one numeric requirement has competing selected executable meanings".into(),
            );
        }
        let format = match first.format {
            numerics::literals::FloatFormat::F32 => semantic_vocabulary::IeeeFloatFormat::Binary32,
            numerics::literals::FloatFormat::F64 => semantic_vocabulary::IeeeFloatFormat::Binary64,
        };
        if import.scalar_parameters != [ScalarType::IeeeFloat(format); 2] {
            return Err("selected numeric import lost its exact operand formats".into());
        }
        let operation = first.operation;
        let adapter =
            TerminalScalarImport::for_import(import, move |arguments: &[TerminalScalarValue]| {
                evaluate(operation, arguments)
            })
            .map_err(|error| error.to_string())?;
        instance
            .bind_import(import.id, adapter)
            .map_err(|error| format!("selected numeric import binding failed: {error:?}"))?;
    }
    Ok(())
}

fn evaluate(
    operation: BinaryOperator,
    arguments: &[TerminalScalarValue],
) -> Result<TerminalScalarValue, TerminalEffectRejection> {
    let [
        TerminalScalarValue::IeeeFloat(left),
        TerminalScalarValue::IeeeFloat(right),
    ] = arguments
    else {
        return Err(TerminalEffectRejection::new(
            "numeric import requires two IEEE operands",
        ));
    };
    if left.format() != right.format() {
        return Err(TerminalEffectRejection::new(
            "numeric import operand formats differ",
        ));
    }
    let format = match left {
        IeeeFloatValue::Binary32(_) => FloatFormat::BINARY32,
        IeeeFloatValue::Binary64(_) => FloatFormat::BINARY64,
    };
    let left_meaning = terminal_interpreter::float_result_policy::meaning(*left);
    let right_meaning = terminal_interpreter::float_result_policy::meaning(*right);
    let boolean = match operation {
        BinaryOperator::Equal => Some(FloatSemantics::equal(&left_meaning, &right_meaning)),
        BinaryOperator::NotEqual => Some(FloatSemantics::not_equal(&left_meaning, &right_meaning)),
        BinaryOperator::Less => Some(FloatSemantics::less(&left_meaning, &right_meaning)),
        BinaryOperator::LessOrEqual => {
            Some(FloatSemantics::less_or_equal(&left_meaning, &right_meaning))
        }
        BinaryOperator::Greater => Some(FloatSemantics::greater(&left_meaning, &right_meaning)),
        BinaryOperator::GreaterOrEqual => Some(FloatSemantics::greater_or_equal(
            &left_meaning,
            &right_meaning,
        )),
        _ => None,
    };
    if let Some(value) = boolean {
        return Ok(TerminalScalarValue::Boolean(value));
    }
    let result = match operation {
        BinaryOperator::Add => FloatSemantics::add(format, &left_meaning, &right_meaning),
        BinaryOperator::Subtract => FloatSemantics::subtract(format, &left_meaning, &right_meaning),
        BinaryOperator::Multiply => FloatSemantics::multiply(format, &left_meaning, &right_meaning),
        BinaryOperator::Divide => FloatSemantics::divide(format, &left_meaning, &right_meaning),
        _ => {
            return Err(TerminalEffectRejection::new(
                "selected operation has no primitive IEEE meaning",
            ));
        }
    };
    Ok(TerminalScalarValue::IeeeFloat(match left {
        IeeeFloatValue::Binary32(_) => IeeeFloatValue::Binary32(result.to_f32().to_bits()),
        IeeeFloatValue::Binary64(_) => IeeeFloatValue::Binary64(result.to_f64().to_bits()),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_supply_returns_raw_ieee_result_for_call_local_policy() {
        let maximum = TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(f32::MAX.to_bits()));
        let result = evaluate(BinaryOperator::Add, &[maximum, maximum]).expect("IEEE addition");
        assert!(matches!(
            result,
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(bits))
                if bits == f32::INFINITY.to_bits()
        ));
    }

    #[test]
    fn primitive_comparison_keeps_unordered_nan_meaning() {
        let nan = TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(f64::NAN.to_bits()));
        assert!(matches!(
            evaluate(BinaryOperator::Equal, &[nan, nan]).expect("equality"),
            TerminalScalarValue::Boolean(false)
        ));
        assert!(matches!(
            evaluate(BinaryOperator::NotEqual, &[nan, nan]).expect("inequality"),
            TerminalScalarValue::Boolean(true)
        ));
    }
}
