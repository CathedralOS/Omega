/* Numeric substitution preserves the selected declaration's landing boundary. */

use crate::resolution::tables::body_expressions;
use crate::source_resolved::SymbolResolvedTrees;
use crate::source_resolved::expression::{ExpressionHandle, ExpressionNode};
use crate::source_resolved::types::TypeReference;
use diagnostics::Diagnostic;
use numeric_literals::{FloatFormat, FloatLiteral, IntegerLanding, LandedIntegerType};
use numerics::arithmetic::ArithmeticDomain;
use source_text::SourceSpan;
use symbols::{BuiltinTypeAtom, SymbolHandle};

pub(super) fn retain_declared_carrier(
    program: &mut SymbolResolvedTrees,
    expression: ExpressionHandle,
    declaration_symbol: SymbolHandle,
    reference: SourceSpan,
) -> Result<(), Diagnostic> {
    let declaration = program
        .const_declaration(declaration_symbol)
        .ok_or_else(|| Diagnostic::error("constant substitution lost its declared carrier"))?;
    let declared_type = declaration.declared_type.clone();
    retain_value_carrier(program, expression, &declared_type, reference)
}

fn retain_value_carrier(
    program: &mut SymbolResolvedTrees,
    expression: ExpressionHandle,
    declared_type: &TypeReference,
    reference: SourceSpan,
) -> Result<(), Diagnostic> {
    if let TypeReference::FixedArray(array) = declared_type {
        let ExpressionNode::ArrayLiteral(elements) =
            program.tables.bodies.expressions.expression(expression)
        else {
            return Ok(());
        };
        let elements = *elements;
        let element_type = program.child_type_reference(array.element_type).clone();
        for element_ordinal in 0..elements.count() {
            let element =
                *body_expressions(program).expression_handle_at_offset(elements, element_ordinal);
            retain_value_carrier(program, element, &element_type, reference)?;
        }
        return Ok(());
    }
    let TypeReference::Named { symbol, .. } = declared_type else {
        return Ok(());
    };
    let Some(atom) = program.symbols.builtin_type_atom(*symbol) else {
        return Ok(());
    };
    retain_scalar_carrier(program, expression, atom, reference)
}

pub(super) fn retain_scalar_carrier(
    program: &mut SymbolResolvedTrees,
    expression: ExpressionHandle,
    atom: BuiltinTypeAtom,
    reference: SourceSpan,
) -> Result<(), Diagnostic> {
    let integer_type = match atom {
        BuiltinTypeAtom::Integer(carrier) => Some(LandedIntegerType::Fixed(carrier)),
        BuiltinTypeAtom::Address => Some(LandedIntegerType::Addr),
        _ => None,
    };
    let float_format = match atom {
        BuiltinTypeAtom::F32 => Some(FloatFormat::F32),
        BuiltinTypeAtom::F64 => Some(FloatFormat::F64),
        _ => None,
    };
    let mismatch = || {
        Diagnostic::error(
            "constant initializer landing conflicts with its declared numeric carrier",
        )
        .with_source_span(reference)
    };
    let node = program.tables.bodies.expressions.expression_mut(expression);
    match node {
        ExpressionNode::Integer(literal) => {
            if let Some(landed_type) = integer_type {
                let landing = IntegerLanding {
                    landed_type,
                    domain: ArithmeticDomain::Exact,
                };
                if literal.landing().is_some_and(|actual| actual != landing) {
                    return Err(mismatch());
                }
                *literal = literal.with_landing(landing);
            } else if let Some(format) = float_format {
                if literal.landing().is_some() {
                    return Err(mismatch());
                }
                let value = literal.value_bignum().ok_or_else(mismatch)?;
                let float = FloatLiteral::parse(&value.to_string()).ok_or_else(mismatch)?;
                *node = ExpressionNode::Float(float.with_landing(format));
            }
        }
        ExpressionNode::Float(literal) => {
            if integer_type.is_some() {
                return Err(mismatch());
            }
            if let Some(format) = float_format {
                if literal.landing().is_some_and(|actual| actual != format) {
                    return Err(mismatch());
                }
                *literal = literal.with_landing(format);
            }
        }
        _ => {}
    }
    Ok(())
}

/* Nominal leaves require identity custody beyond their encoded value labels. */
pub(super) fn has_nominal_carrier(program: &SymbolResolvedTrees, carrier: &TypeReference) -> bool {
    match carrier {
        TypeReference::Named { symbol, .. } => {
            program.symbols.get(*symbol).kind == symbols::SymbolKind::Data
        }
        TypeReference::FixedArray(array) => {
            has_nominal_carrier(program, program.child_type_reference(array.element_type))
        }
        TypeReference::Constrained(carrier) => {
            has_nominal_carrier(program, program.child_type_reference(carrier.base_type))
        }
        _ => false,
    }
}

/*
    Reconstruct equality from resolved declarations and array geometry, never
    from rendered type names or equal structural initializer encodings.
*/
pub(super) fn same_resolved_carrier(
    program: &SymbolResolvedTrees,
    left: &TypeReference,
    right: &TypeReference,
) -> bool {
    match (left, right) {
        (TypeReference::Named { symbol: left, .. }, TypeReference::Named { symbol: right, .. }) => {
            left.is_valid() && left == right
        }
        (TypeReference::FixedArray(left), TypeReference::FixedArray(right)) => {
            left.length == right.length
                && same_resolved_carrier(
                    program,
                    program.child_type_reference(left.element_type),
                    program.child_type_reference(right.element_type),
                )
        }
        _ => false,
    }
}

pub(super) fn encoding_has_nominal_carrier(encoding: &str) -> bool {
    use application_identity::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
    fn contains(value: &DecodedCanonicalConstValue) -> bool {
        match value {
            DecodedCanonicalConstValue::Record { .. }
            | DecodedCanonicalConstValue::Variant { .. } => true,
            DecodedCanonicalConstValue::Array { values, .. } => values.iter().any(contains),
            _ => false,
        }
    }
    CanonicalConstValue::new("", encoding, "")
        .decode_encoding()
        .is_some_and(|value| contains(&value))
}
