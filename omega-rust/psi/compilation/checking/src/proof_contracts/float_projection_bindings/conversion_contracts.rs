//! Read a selected callable's IEEE conversion guarantee, not its leaf name.
//!
//! The result projection, source parameter and format constant must join the
//! same ensures equality through the sealed float catalogs. No conversion
//! meaning comes from a destination annotation or a callable named `from_f64`.
//! Float and integer inputs share the same result/format/parameter join. The
//! integer semantic family has eight carriers, so its proof-position overload
//! is selected by that exact parameter kind before checking sealed custody.

use crate::frozen_program::expression_table;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::operator::OperatorDefinition;
use typed_trees::types::PrimitiveType;

/// Return the carriers of a one-argument callable whose normal-return
/// contract states a sealed format conversion. Admission of the call and
/// proof of that contract remain with their ordinary owners.
pub fn float_format_conversion_contract(
    program: &dyn crate::CheckingInput,
    operator: &OperatorDefinition,
) -> Option<(PrimitiveType, PrimitiveType)> {
    numeric_float_conversion_contract(program, operator)
        .filter(|(source, _)| matches!(source, PrimitiveType::F32 | PrimitiveType::F64))
}

/// A sealed conversion to a float carrier, from either a float projection or
/// the selected integer parameter. The declaration name selects no semantics.
pub fn numeric_float_conversion_contract(
    program: &dyn crate::CheckingInput,
    operator: &OperatorDefinition,
) -> Option<(PrimitiveType, PrimitiveType)> {
    selected_numeric_float_conversion_contract(program, operator)
        .map(|(source, target, _)| (source, target))
}

fn selected_numeric_float_conversion_contract(
    program: &dyn crate::CheckingInput,
    operator: &OperatorDefinition,
) -> Option<(
    PrimitiveType,
    PrimitiveType,
    &'static numerics::float_semantics_catalog::FloatSemanticOperation,
)> {
    use numerics::float_semantics_catalog::{FloatSemanticKernel, FloatSemanticValueKind};
    let [parameter] = program.operator_parameters(operator) else {
        return None;
    };
    let source = program.primitive_type_reference(parameter.type_reference)?;
    let target = program.primitive_type_reference(operator.return_type)?;
    if !(matches!(source, PrimitiveType::F32 | PrimitiveType::F64)
        || numerics::float_semantics_catalog::IntegerCarrier::from_spelling(source.name())
            .is_some())
        || !matches!(target, PrimitiveType::F32 | PrimitiveType::F64)
    {
        return None;
    }
    let resolve = |program: &dyn crate::CheckingInput,
                   conversion: &typed_trees::expression::TableCallExpression| {
        let source_kind = if matches!(source, PrimitiveType::F32 | PrimitiveType::F64) {
            FloatSemanticValueKind::Meaning
        } else {
            FloatSemanticValueKind::Integer(
                numerics::float_semantics_catalog::IntegerCarrier::from_spelling(source.name())?,
            )
        };
        super::semantic_operations::exact_toolchain_float_semantic_application(
            program,
            conversion,
            &[FloatSemanticValueKind::Format, source_kind],
        )
        .map(|(_, semantic, _)| (None, semantic))
    };
    scan_conversion_equalities(program, operator, resolve, |candidate| {
        let (result, result_primitive, meaning_type) = projection(program, candidate.result)?;
        if result_primitive != target
            || crate::reserved_result_owner(program, result).map(|owner| owner.0)
                != Some(operator.symbol)
            || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                operator.symbol,
                candidate.expression,
                language_core::OperatorSpelling::Equal,
                &[Some(meaning_type), Some(meaning_type)],
            )
        {
            return None;
        }
        let semantic = candidate.semantic;
        let [format, value] =
            expression_table(program).expression_handles(candidate.conversion.arguments)
        else {
            return None;
        };
        let expected_format = match target {
            PrimitiveType::F32 => semantic_vocabulary::IeeeFloatFormat::Binary32,
            PrimitiveType::F64 => semantic_vocabulary::IeeeFloatFormat::Binary64,
            _ => return None,
        };
        if super::semantic_values::exact_toolchain_float_format_const(program, *format)
            != Some(expected_format)
        {
            return None;
        }
        let value = if matches!(source, PrimitiveType::F32 | PrimitiveType::F64) {
            if semantic.name != "convert" {
                return None;
            }
            let (value, source_primitive, _) = projection(program, *value)?;
            if source_primitive != source {
                return None;
            }
            value
        } else {
            let [
                FloatSemanticValueKind::Format,
                FloatSemanticValueKind::Integer(carrier),
            ] = semantic.parameters
            else {
                return None;
            };
            if !matches!(semantic.kernel(), FloatSemanticKernel::FromInteger(_))
                || PrimitiveType::from_name(carrier.spelling()) != Some(source)
            {
                return None;
            }
            *value
        };
        let value = expression_table(program).name_path_node(value)?;
        if value.symbol.is_valid() && value.symbol == parameter.symbol {
            Some((source, target, semantic))
        } else {
            None
        }
    })
}

/// Normal-return enclosure of a selected Integer-to-float conversion over
/// its entire admitted source carrier. The sealed kernel supplies rounded
/// endpoint images; annotations and callable spelling supply no bound.
/// Executable call formation, Requires and policy adaptation remain separate.
pub fn integer_float_conversion_bounds(
    program: &dyn crate::CheckingInput,
    operator: &OperatorDefinition,
) -> Option<(
    PrimitiveType,
    PrimitiveType,
    numerics::float_semantics::FloatMeaning,
    numerics::float_semantics::FloatMeaning,
)> {
    let (source, target, semantic) = selected_numeric_float_conversion_contract(program, operator)?;
    let carrier = numerics::float_semantics_catalog::IntegerCarrier::from_spelling(source.name())?;
    let format = match target {
        PrimitiveType::F32 => numerics::float_semantics::FloatFormat::BINARY32,
        PrimitiveType::F64 => numerics::float_semantics::FloatFormat::BINARY64,
        _ => return None,
    };
    let (minimum, maximum) = carrier.format().bounds();
    let (minimum, maximum) = semantic.integer_operand_image(format, carrier, &minimum, &maximum)?;
    Some((source, target, minimum, maximum))
}

/// One resolved equality candidate: a reserved-result side against a named
/// conversion call whose toolchain semantic contract was selected exactly.
/// Each caller's `resolve` admits its own conversion catalog; `selected` is
/// the resolved operator where that catalog names one.
struct ConversionEqualityCandidate<'program> {
    expression: ExpressionHandle,
    result: ExpressionHandle,
    conversion: &'program typed_trees::expression::TableCallExpression,
    selected: Option<&'program OperatorDefinition>,
    semantic: &'static numerics::float_semantics_catalog::FloatSemanticOperation,
}

/// Scan the operator's ensures equalities for result-vs-conversion pairs in
/// both operand orders, resolving each call with `resolve` and returning the
/// first candidate `judge` accepts.
fn scan_conversion_equalities<'program, R>(
    program: &'program dyn crate::CheckingInput,
    operator: &OperatorDefinition,
    mut resolve: impl FnMut(
        &'program dyn crate::CheckingInput,
        &'program typed_trees::expression::TableCallExpression,
    ) -> Option<(
        Option<&'program OperatorDefinition>,
        &'static numerics::float_semantics_catalog::FloatSemanticOperation,
    )>,
    mut judge: impl FnMut(&ConversionEqualityCandidate<'_>) -> Option<R>,
) -> Option<R> {
    for expression in crate::proof_contracts::proof_facts::ensures_expression_facts(
        program,
        program.operator_contracts(operator),
    ) {
        let Some(equality) = expression_table(program).binary_node(expression) else {
            continue;
        };
        if equality.operator != BinaryOperator::Equal {
            continue;
        }
        for (result, converted) in [
            (equality.left, equality.right),
            (equality.right, equality.left),
        ] {
            let Some(conversion) = expression_table(program).call_node(converted) else {
                continue;
            };
            let Some((selected, semantic)) = resolve(program, conversion) else {
                continue;
            };
            if let Some(verdict) = judge(&ConversionEqualityCandidate {
                expression: expression,
                result,
                conversion,
                selected,
                semantic,
            }) {
                return Some(verdict);
            }
        }
    }
    None
}

fn projection(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<(
    ExpressionHandle,
    PrimitiveType,
    typed_trees::types::TypeReferenceHandle,
)> {
    let call = expression_table(program).call_node(expression)?;
    let (selected, primitive, _, _) =
        crate::proof_contracts::float_projection_invocations::exact_projection_operation(
            program, call,
        )?;
    let operator = typed_trees::operator::declaration_by_symbol(program, selected)?;
    let [operand] = expression_table(program).expression_handles(call.arguments) else {
        return None;
    };
    Some((*operand, primitive, operator.return_type))
}

/// A selected integer-result guarantee over the same sealed float projection.
/// Kernel identity establishes truncation or saturation, never the callable's
/// name. The result occurrence and source parameter belong to this operator.
pub fn float_integer_conversion_contract(
    program: &dyn crate::CheckingInput,
    operator: &OperatorDefinition,
) -> Option<(
    PrimitiveType,
    &'static numerics::float_semantics_catalog::FloatSemanticOperation,
)> {
    use numerics::float_semantics_catalog::{FloatSemanticKernel, FloatSemanticValueKind};
    let [parameter] = program.operator_parameters(operator) else {
        return None;
    };
    let source = program.primitive_type_reference(parameter.type_reference)?;
    if !matches!(source, PrimitiveType::F32 | PrimitiveType::F64) {
        return None;
    }
    let target = program.primitive_type_reference(operator.return_type)?;
    fn resolve<'program>(
        program: &'program dyn crate::CheckingInput,
        conversion: &'program typed_trees::expression::TableCallExpression,
    ) -> Option<(
        Option<&'program OperatorDefinition>,
        &'static numerics::float_semantics_catalog::FloatSemanticOperation,
    )> {
        let selected = typed_trees::operator::resolve_named_expression_call(program, conversion)?;
        super::semantic_operations::exact_toolchain_float_semantic_contract(program, selected)
            .map(|(semantic, _)| (Some(selected), semantic))
    }
    scan_conversion_equalities(program, operator, resolve, |candidate| {
        if crate::reserved_result_owner(program, candidate.result).map(|owner| owner.0)
            != Some(operator.symbol)
        {
            return None;
        }
        let Some(selected) = candidate.selected else {
            return None;
        };
        let semantic = candidate.semantic;
        let FloatSemanticValueKind::Integer(carrier) = semantic.result else {
            return None;
        };
        if PrimitiveType::from_name(carrier.spelling()) != Some(target)
            || !matches!(
                semantic.kernel(),
                FloatSemanticKernel::ToIntegerExact | FloatSemanticKernel::ToIntegerSaturating
            )
            || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                operator.symbol,
                candidate.expression,
                language_core::OperatorSpelling::Equal,
                &[Some(operator.return_type), Some(selected.return_type)],
            )
        {
            return None;
        }
        let [value] = expression_table(program).expression_handles(candidate.conversion.arguments)
        else {
            return None;
        };
        let (value, primitive, _) = projection(program, *value)?;
        let value = expression_table(program).name_path_node(value)?;
        if primitive == source && value.symbol.is_valid() && value.symbol == parameter.symbol {
            Some((source, semantic))
        } else {
            None
        }
    })
}

/// Both admitted integer-conversion kernels are monotone on ordered inputs.
/// Exact here is the named truncating operation's contract-gated meaning;
/// authored Exact coercions separately owe integrality before using it.
pub fn float_integer_conversion_bounds(
    program: &dyn crate::CheckingInput,
    operator: &OperatorDefinition,
    minimum: f64,
    maximum: f64,
) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    use numerics::float_semantics::FloatMeaning;
    use numerics::float_semantics_catalog::{FloatSemanticOperand, FloatSemanticResult};
    let (_, semantic) = float_integer_conversion_contract(program, operator)?;
    if minimum > maximum || minimum.is_nan() || maximum.is_nan() {
        return None;
    }
    let convert = |value| {
        let discharged = semantic
            .kernel_discharge(&[FloatSemanticOperand::Meaning(FloatMeaning::from_f64(value))])?;
        let FloatSemanticResult::Integer(value) = discharged.result else {
            return None;
        };
        Some(value)
    };
    Some((convert(minimum)?, convert(maximum)?))
}
