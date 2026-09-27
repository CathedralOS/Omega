//! Closed source evaluation for the sealed float-meaning catalog.
//!
//! This is bounded source checking, not portable proof evidence. Evaluate
//! only exact toolchain projections/applications; an unknown operand or
//! exhausted work budget supplies no fact. The checked binder shares format
//! recognition here so source checking and emitted rows cannot select
//! different canonical constants.

use super::semantic_operations::{
    exact_toolchain_float_semantic_contract, symbol_is_declared_in_sealed_source,
};
use numerics::float_projection::FloatProjectionOperation;
use numerics::float_semantics::{FloatFormat, FloatMeaning};
use numerics::float_semantics_catalog::{
    FLOAT_FORMAT_CORE_SOURCE, FloatSemanticOperand, FloatSemanticResult, FloatSemanticValueKind,
};
use semantic_vocabulary::IeeeFloatFormat;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::operator::resolve_named_expression_call;

struct ClosedFloatMeaning {
    format: IeeeFloatFormat,
    meaning: FloatMeaning,
    type_reference: typed_trees::types::TypeReferenceHandle,
}

/// Decide a closed meaning equality, preserving its selected equality
/// operation and exact carrier. Open terms do not gain symbolic float laws.
pub(crate) fn closed_float_meaning_equality(
    program: &TypedTrees,
    machine_symbol: symbols::SymbolHandle,
    expression: ExpressionHandle,
) -> Option<bool> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    let spelling = match binary.operator {
        BinaryOperator::Equal => language_core::OperatorSpelling::Equal,
        BinaryOperator::NotEqual => language_core::OperatorSpelling::NotEqual,
        _ => return None,
    };
    // This is a service-work bound, not a limit on the language's values.
    // Exhaustion retains the existing unsupported-proof diagnostic.
    let mut remaining_work = 1024;
    let left = closed_meaning(program, binary.left, &mut remaining_work)?;
    let right = closed_meaning(program, binary.right, &mut remaining_work)?;
    // A proof-only call has no runtime place type. Use the exact selected
    // declarations' result types, not wildcard operands that could select an
    // unrelated overloaded equality. Authored equality still blocks this rule.
    if left.format != right.format
        || !typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            machine_symbol,
            expression,
            spelling,
            &[Some(left.type_reference), Some(right.type_reference)],
        )
    {
        return None;
    }
    Some(if binary.operator == BinaryOperator::Equal {
        left.meaning == right.meaning
    } else {
        left.meaning != right.meaning
    })
}

fn closed_meaning(
    program: &TypedTrees,
    expression: ExpressionHandle,
    remaining_work: &mut usize,
) -> Option<ClosedFloatMeaning> {
    *remaining_work = remaining_work.checked_sub(1)?;
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    let arguments = program.expression_table.expression_handles(call.arguments);
    if let Some((selected_operator, _, operation, _)) =
        crate::proof_contracts::float_projection_invocations::exact_projection_operation(
            program, call,
        )
    {
        let [source] = arguments else { return None };
        let ExpressionNode::Float(literal) = program.expression_table.expression(*source) else {
            return None;
        };
        let (format, meaning) = match operation {
            FloatProjectionOperation::Meaning32 => (
                IeeeFloatFormat::Binary32,
                FloatMeaning::from_f32(f32::from_bits(literal.f32_bits())),
            ),
            FloatProjectionOperation::Meaning64 => (
                IeeeFloatFormat::Binary64,
                FloatMeaning::from_f64(literal.landed_f64()),
            ),
        };
        let operator = program
            .operators()
            .iter()
            .find(|operator| operator.symbol == selected_operator)?;
        return Some(ClosedFloatMeaning {
            format,
            meaning,
            type_reference: operator.return_type,
        });
    }
    let operator = resolve_named_expression_call(program, call)?;
    let (row, contract) = exact_toolchain_float_semantic_contract(program, operator)?;
    if row.result != FloatSemanticValueKind::Meaning || arguments.len() != row.parameters.len() {
        return None;
    }
    let mut operands = Vec::with_capacity(arguments.len());
    let mut declared_format = None;
    let mut operand_format = None;
    for (argument, kind) in arguments.iter().zip(row.parameters) {
        match kind {
            FloatSemanticValueKind::Format => {
                let format = exact_toolchain_float_format_const(program, *argument)?;
                declared_format = Some(format);
                operands.push(FloatSemanticOperand::Format(match format {
                    IeeeFloatFormat::Binary32 => FloatFormat::BINARY32,
                    IeeeFloatFormat::Binary64 => FloatFormat::BINARY64,
                }));
            }
            FloatSemanticValueKind::Meaning => {
                let value = closed_meaning(program, *argument, remaining_work)?;
                if operand_format.is_some_and(|previous| previous != value.format) {
                    return None;
                }
                operand_format = Some(value.format);
                operands.push(FloatSemanticOperand::Meaning(value.meaning));
            }
            _ => return None,
        }
    }
    let format = declared_format.or(operand_format)?;
    let discharge = row.kernel_discharge(&operands)?;
    if discharge.contract != contract {
        return None;
    }
    let FloatSemanticResult::Meaning(meaning) = discharge.result else {
        return None;
    };
    Some(ClosedFloatMeaning {
        format,
        meaning,
        type_reference: operator.return_type,
    })
}

/// The sealed IEEE format a `FloatFormat::BINARY*` const names, by exact
/// hermetic identity and sealed-source custody.
///
/// Const substitution erases the authored const path before the typed trees:
/// the operand arrives either as a surviving `Name` or — the common case — as
/// the const's own `StructLiteral` value inlined at the use. A literal is
/// matched by resolving its `type_symbol` to the sealed `toolchain::FloatFormat`
/// record, then comparing every field value against the sealed const's
/// `canonical_value_encoding` leaf-by-leaf, so `BINARY32`/`BINARY64` stay
/// distinguishable even though they share one record type.
pub fn exact_toolchain_float_format_const(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<IeeeFloatFormat> {
    const FORMAT_CONST_IDENTITY: [(IeeeFloatFormat, &str); 2] = [
        (
            IeeeFloatFormat::Binary32,
            "toolchain::FloatFormat::BINARY32",
        ),
        (
            IeeeFloatFormat::Binary64,
            "toolchain::FloatFormat::BINARY64",
        ),
    ];
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            if !path.symbol.is_valid()
                || !symbol_is_declared_in_sealed_source(
                    program,
                    path.symbol,
                    FLOAT_FORMAT_CORE_SOURCE,
                )
            {
                return None;
            }
            match program
                .normalized_hermetic_symbol_identity(path.symbol)
                .ok()?
                .as_str()
            {
                "toolchain::FloatFormat::BINARY32" => Some(IeeeFloatFormat::Binary32),
                "toolchain::FloatFormat::BINARY64" => Some(IeeeFloatFormat::Binary64),
                _ => None,
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            if !literal.type_symbol.is_valid()
                || program
                    .normalized_hermetic_symbol_identity(literal.type_symbol)
                    .ok()
                    .as_deref()
                    != Some("toolchain::FloatFormat")
                || !symbol_is_declared_in_sealed_source(
                    program,
                    literal.type_symbol,
                    FLOAT_FORMAT_CORE_SOURCE,
                )
            {
                return None;
            }
            FORMAT_CONST_IDENTITY
                .iter()
                .find(|(_, identity)| {
                    program.const_declarations().iter().any(|declaration| {
                        program
                            .normalized_hermetic_symbol_identity(declaration.symbol)
                            .ok()
                            .as_deref()
                            == Some(*identity)
                            && symbol_is_declared_in_sealed_source(
                                program,
                                declaration.symbol,
                                FLOAT_FORMAT_CORE_SOURCE,
                            )
                            && declaration
                                .canonical_value_encoding
                                .as_deref()
                                .and_then(|encoding| {
                                    language_semantics::const_value::CanonicalConstValue::new(
                                        "", encoding, "",
                                    )
                                    .decode_encoding()
                                })
                                .is_some_and(|decoded| {
                                    struct_literal_matches_decoded_const(
                                        program, expression, &decoded,
                                    )
                                })
                    })
                })
                .map(|(format, _)| *format)
        }
        _ => None,
    }
}

/// Whether one expression equals a decoded canonical const value
/// leaf-by-leaf — field names and scalar values only. The record's toolchain
/// custody is the caller's symbol decision; the encoded `type_name` strings
/// are encoded claims, not resolved type authority.
fn struct_literal_matches_decoded_const(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: &language_semantics::const_value::DecodedCanonicalConstValue,
) -> bool {
    use language_semantics::const_value::DecodedCanonicalConstValue as Decoded;
    match (program.expression_table.expression(expression), expected) {
        (ExpressionNode::Integer(literal), Decoded::Integer { value, .. }) => {
            literal
                .value_i64()
                .map(i128::from)
                .or_else(|| literal.value_u64().map(i128::from))
                == Some(*value)
        }
        (ExpressionNode::Boolean(observed), Decoded::Boolean(expected)) => observed == expected,
        (ExpressionNode::StructLiteral(literal), Decoded::Record { fields, .. }) => {
            let observed = program.expression_table.struct_fields(literal.fields);
            observed.len() == fields.len()
                && fields.iter().all(|(name, expected_field)| {
                    observed
                        .iter()
                        .find(|field| field.name.as_str() == name.as_str())
                        .is_some_and(|field| {
                            struct_literal_matches_decoded_const(
                                program,
                                field.value,
                                expected_field,
                            )
                        })
                })
        }
        _ => false,
    }
}
