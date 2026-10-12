//! Original type-role operands shared by static application consumers.
//!
//! This copies the retained typed graph into invocation scratch. It does not
//! select declarations, infer operands, admit bounds or create an application.

use crate::expression::StaticMachineArgument;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};

pub fn static_argument_from_type_reference(
    program: &crate::TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<StaticMachineArgument> {
    program.static_operand_from_type_reference(handle).ok()
}

pub fn static_const_literal_from_type_reference(
    program: &crate::TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<numerics::literals::IntegerLiteral> {
    let retained = program
        .type_reference_table
        .completed_const_result(handle)
        .cloned();
    let spelling = match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Named { name, .. } => name.as_str(),
        TypeReferenceNode::ConstExpression(_) if retained.is_some() => "",
        _ => return None,
    };
    let meaning = program.type_reference_table.const_integer_meaning(handle);
    let canonical_integer = retained
        .or_else(|| language_semantics::const_value::CanonicalConstValue::from_atom(spelling))
        .and_then(|value| match value.decode_encoding()? {
            language_semantics::const_value::DecodedCanonicalConstValue::Integer {
                value, ..
            } => Some(value.to_string()),
            _ => None,
        });
    // An atom without its original integer meaning remains a named atom. The
    // receiving slot cannot donate a landing to an older missing receipt.
    if meaning == crate::types::ConstIntegerMeaning::Unavailable {
        return None;
    }
    let mut spelling = canonical_integer.as_deref().unwrap_or(spelling);
    let negative = spelling.starts_with('-');
    if negative {
        spelling = &spelling[1..];
    }
    let (radix, digits) = if let Some(digits) = spelling.strip_prefix("0b") {
        (numerics::literals::IntegerRadix::Binary, digits)
    } else if let Some(digits) = spelling.strip_prefix("0o") {
        (numerics::literals::IntegerRadix::Octal, digits)
    } else if let Some(digits) = spelling.strip_prefix("0x") {
        (numerics::literals::IntegerRadix::Hexadecimal, digits)
    } else {
        (numerics::literals::IntegerRadix::Decimal, spelling)
    };
    let literal = numerics::literals::IntegerLiteral::from_parts(negative, radix, digits).ok()?;
    Some(match meaning {
        crate::types::ConstIntegerMeaning::Typed {
            primitive,
            arithmetic_domain,
        } => literal.with_landing(numerics::literals::IntegerLanding {
            landed_type: primitive.landed_landing()?,
            domain: arithmetic_domain,
        }),
        _ => literal,
    })
}
