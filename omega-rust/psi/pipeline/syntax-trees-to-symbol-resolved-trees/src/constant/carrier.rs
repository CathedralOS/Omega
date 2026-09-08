//! Numeric substitution preserves the selected declaration's landing boundary.

use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::{FloatFormat, FloatLiteral, IntegerLanding, LandedIntegerType};
use source::SourceSpan;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
use symbol_resolved_trees::types::TypeReference;
use symbols::{BuiltinTypeAtom, SymbolHandle};

pub(super) fn retain_declared_carrier(
    program: &mut SymbolResolvedTrees,
    expression: ExpressionHandle,
    declaration_symbol: SymbolHandle,
    reference: SourceSpan,
) -> Result<(), Diagnostic> {
    let declaration = program
        .const_declarations
        .iter()
        .find(|declaration| declaration.symbol == declaration_symbol)
        .ok_or_else(|| Diagnostic::error("constant substitution lost its declared carrier"))?;
    let TypeReference::Named { symbol, .. } = &declaration.declared_type else {
        return Ok(());
    };
    let Some(atom) = program.symbols.builtin_type_atom(*symbol) else {
        return Ok(());
    };
    let integer_type = match atom {
        BuiltinTypeAtom::I8 => Some(LandedIntegerType::I8),
        BuiltinTypeAtom::I16 => Some(LandedIntegerType::I16),
        BuiltinTypeAtom::I32 => Some(LandedIntegerType::I32),
        BuiltinTypeAtom::I64 => Some(LandedIntegerType::I64),
        BuiltinTypeAtom::U8 => Some(LandedIntegerType::U8),
        BuiltinTypeAtom::U16 => Some(LandedIntegerType::U16),
        BuiltinTypeAtom::U32 => Some(LandedIntegerType::U32),
        BuiltinTypeAtom::U64 => Some(LandedIntegerType::U64),
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
