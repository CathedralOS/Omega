//! Exact const-binder substitution in the existing structural identity grammar.

use super::{
    TypeIdentityContext, atom, compound, normalize_const_or_nominal_name,
    normalize_index_expression,
};
use crate::{
    TypedTrees,
    expression::ExpressionNode,
    types::{TypeReferenceHandle, TypeReferenceNode},
};
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use symbols::SymbolHandle;

fn selected(
    context: &TypeIdentityContext<'_>,
    symbol: SymbolHandle,
) -> Option<(usize, TypeReferenceHandle)> {
    if symbol.is_valid() {
        return context
            .substitutions
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (candidate, _))| *candidate == symbol)
            .map(|(index, (_, reference))| (index, *reference));
    }
    None
}

pub(super) fn index(
    program: &TypedTrees,
    symbol: SymbolHandle,
    _spelling: &str,
    context: &TypeIdentityContext<'_>,
) -> Option<String> {
    let (_, reference) = selected(context, symbol)?;
    if context.active_const_substitutions.len() >= 64
        || context.active_const_substitutions.contains(&symbol)
    {
        return Some(rejected(context));
    }
    let mut active = context.active_const_substitutions.to_vec();
    active.push(symbol);
    let nested = TypeIdentityContext {
        active_const_substitutions: &active,
        ..*context
    };
    Some(index_reference(program, reference, &nested))
}

fn index_reference(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    context: &TypeIdentityContext<'_>,
) -> String {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, name } => {
            if let Some(identity) = index(program, *symbol, name.as_str(), context) {
                return identity;
            }
            if let Some(value) = integer(program, *symbol, name.as_str()) {
                return atom("integer", &value.to_string());
            }
            normalize_const_or_nominal_name(program, *symbol, name.as_str(), "const-name", context)
        }
        TypeReferenceNode::ConstExpression(expression) => {
            normalize_index_expression(program, *expression, context)
        }
        _ => rejected(context),
    }
}

pub(super) fn array_length(
    program: &TypedTrees,
    symbol: SymbolHandle,
    _spelling: &str,
    context: &TypeIdentityContext<'_>,
) -> Option<String> {
    let (_, reference) = selected(context, symbol)?;
    if context.active_const_substitutions.len() >= 64
        || context.active_const_substitutions.contains(&symbol)
    {
        return Some(rejected(context));
    }
    let mut active = context.active_const_substitutions.to_vec();
    active.push(symbol);
    let nested = TypeIdentityContext {
        active_const_substitutions: &active,
        ..*context
    };
    Some(
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, name } => {
                if let Some(identity) = array_length(program, *symbol, name.as_str(), &nested) {
                    return Some(identity);
                }
                match integer(program, *symbol, name.as_str())
                    .and_then(|value| usize::try_from(value).ok())
                {
                    Some(value) => atom("literal", &value.to_string()),
                    None => atom(
                        "const-parameter",
                        &nested.name(program, *symbol, name.as_str()),
                    ),
                }
            }
            TypeReferenceNode::ConstExpression(expression) => {
                match program.expression_table.expression(*expression) {
                    ExpressionNode::Integer(value) => atom("literal", &value.to_string()),
                    _ => compound(
                        "const-expression",
                        [normalize_index_expression(program, *expression, &nested)],
                    ),
                }
            }
            _ => rejected(context),
        },
    )
}

fn rejected(context: &TypeIdentityContext<'_>) -> String {
    if let Some(missing) = context.missing_exact_nominal_owner {
        missing.set(true);
    }
    "unsupported-const-substitution".to_owned()
}

fn integer(program: &TypedTrees, symbol: SymbolHandle, spelling: &str) -> Option<i128> {
    if !symbol.is_valid() {
        if let Ok(value) = spelling.parse() {
            return Some(value);
        }
        let value = CanonicalConstValue::from_atom(spelling)?;
        return match value.identity().decode_encoding()? {
            DecodedCanonicalConstValue::Integer { value, .. } => Some(value),
            _ => None,
        };
    }
    let declaration = program
        .const_declarations()
        .iter()
        .find(|declaration| declaration.symbol == symbol)?;
    let encoding = declaration.canonical_value_encoding.as_ref()?;
    match CanonicalConstValue::new("", encoding.clone(), "").decode_encoding()? {
        DecodedCanonicalConstValue::Integer { value, .. } => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
