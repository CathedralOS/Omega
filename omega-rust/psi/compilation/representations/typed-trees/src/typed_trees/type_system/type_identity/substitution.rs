//! Exact const-binder substitution in the existing structural identity grammar.
use super::IndexIdentityArgument;
use crate::typed_trees::type_system::type_identity::constraint_identity::atom;
use crate::typed_trees::type_system::type_identity::constraint_identity::compound;
use crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityContext;
use crate::typed_trees::type_system::type_identity::identity_context::normalize_const_or_nominal_name;
use crate::typed_trees::type_system::type_identity::identity_context::normalize_index_expression;
use crate::typed_trees::type_system::type_identity::identity_context::normalize_type_reference;
use crate::{
    TypedTrees,
    expression::ExpressionNode,
    types::{TypeReferenceHandle, TypeReferenceNode},
};
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use symbols::SymbolHandle;

pub(super) fn index_argument(
    program: &TypedTrees,
    argument: IndexIdentityArgument<'_>,
    context: &TypeIdentityContext<'_>,
) -> String {
    match argument {
        IndexIdentityArgument::TypeArgument(reference) => {
            if !program
                .type_reference_table
                .contains_type_reference(reference)
            {
                return rejected(context);
            }
            normalize_type_reference(program, reference, context)
        }
        IndexIdentityArgument::Expression(expression) => {
            if !program.expression_table.expression_is_valid(expression) {
                return rejected(context);
            }
            normalize_index_expression(program, expression, context)
        }
        IndexIdentityArgument::TypeReference(reference) => {
            if !program
                .type_reference_table
                .contains_type_reference(reference)
            {
                return rejected(context);
            }
            index_reference(program, reference, context)
        }
        IndexIdentityArgument::Subject(symbol) => {
            if !symbol.is_valid() {
                return rejected(context);
            }
            index_name(program, symbol, program.symbols.name(symbol), context)
        }
        IndexIdentityArgument::StaticArgument(argument) => {
            if argument.type_reference.is_valid() {
                return normalize_type_reference(program, argument.type_reference, context);
            }
            if let Some(literal) = &argument.const_literal {
                return index_integer(literal, context);
            }
            if let Some(identity) =
                super::identity_context::static_type_argument_identity(program, argument, context)
            {
                return identity;
            }
            if !argument.symbol.is_valid() || argument.evidence_projection.is_some() {
                return rejected(context);
            }
            let owner = index_name(program, argument.symbol, "", context);
            let Some(application) = &argument.application else {
                return owner;
            };
            compound(
                "static-application",
                std::iter::once(owner)
                    .chain(
                        application
                            .lifetime_arguments
                            .iter()
                            .map(|lifetime| atom("lifetime", lifetime.as_str())),
                    )
                    .chain(application.arguments.iter().map(|argument| {
                        index_argument(
                            program,
                            IndexIdentityArgument::StaticArgument(argument),
                            context,
                        )
                    })),
            )
        }
    }
}

pub(super) fn index_integer(
    literal: &numerics::literals::IntegerLiteral,
    context: &TypeIdentityContext<'_>,
) -> String {
    literal.value_bignum().map_or_else(
        || rejected(context),
        |value| atom("integer", &value.to_string()),
    )
}

pub(super) fn index_name(
    program: &TypedTrees,
    symbol: SymbolHandle,
    spelling: &str,
    context: &TypeIdentityContext<'_>,
) -> String {
    if let Some(identity) = index(program, symbol, spelling, context) {
        return identity;
    }
    if let Some(value) = integer(program, symbol, spelling) {
        return atom("integer", &value.to_string());
    }
    if let Some(value) = boolean(program, symbol, spelling) {
        return atom("boolean", &value.to_string());
    }
    normalize_const_or_nominal_name(program, symbol, spelling, "const-name", context)
}

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
    if let Some((argument, outer)) = context.applied_argument(symbol) {
        return Some(index_argument(program, argument, &outer));
    }
    if symbol.is_valid()
        && let Some((_, replacement)) = context
            .index_substitutions
            .iter()
            .find(|(candidate, _)| *candidate == symbol)
    {
        return Some(index_argument(
            program,
            *replacement,
            &TypeIdentityContext {
                index_substitutions: &[],
                ..*context
            },
        ));
    }
    let (ordinal, reference) = selected(context, symbol)?;
    if context.active_const_substitutions.len() >= 64
        || context.active_const_substitutions.contains(&symbol)
    {
        return Some(rejected(context));
    }
    let mut active = context.active_const_substitutions.to_vec();
    active.push(symbol);
    let nested = TypeIdentityContext {
        active_const_substitutions: &active,
        ..context.replacement_context_at(ordinal)
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
            index_name(program, *symbol, name.as_str(), context)
        }
        TypeReferenceNode::ConstExpression(expression) => {
            normalize_index_expression(program, *expression, context)
        }
        _ => normalize_type_reference(program, reference, context),
    }
}

/// Resolve only a direct endpoint binder. Substituting an expression retains its
/// source-selected arithmetic; decoded static integers need no synthetic AST.
pub(super) fn range_endpoint(
    program: &TypedTrees,
    symbol: SymbolHandle,
    end_inclusive: bool,
    context: &TypeIdentityContext<'_>,
) -> Option<String> {
    let (ordinal, reference) = selected(context, symbol)?;
    if context.active_const_substitutions.len() >= 64
        || context.active_const_substitutions.contains(&symbol)
    {
        return Some(rejected(context));
    }
    let mut active = context.active_const_substitutions.to_vec();
    active.push(symbol);
    let nested = TypeIdentityContext {
        active_const_substitutions: &active,
        ..context.replacement_context_at(ordinal)
    };
    Some(
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::ConstExpression(expression) => {
                crate::typed_trees::type_system::type_identity::constraint_identity::normalized_range_endpoint(program, *expression, end_inclusive, &nested)
            }
            TypeReferenceNode::Named { symbol, name } => {
                if let Some(identity) = range_endpoint(program, *symbol, end_inclusive, &nested) {
                    return Some(identity);
                }
                if let Some(value) = integer(program, *symbol, name.as_str()) {
                    let value = crate::closed_numeric::canonical_integer_range_endpoint(
                        numerics::bignum::BigInt::from_i128(value),
                        end_inclusive,
                    );
                    crate::typed_trees::type_system::type_identity::constraint_identity::normalized_range_integer(&value, &nested)
                } else {
                    let identity = normalize_const_or_nominal_name(
                        program,
                        *symbol,
                        name.as_str(),
                        "const-name",
                        &nested,
                    );
                    if end_inclusive {
                        identity
                    } else {
                        compound("exclusive-end", [identity])
                    }
                }
            }
            _ => {
                let identity = normalize_type_reference(program, reference, &nested);
                if end_inclusive {
                    identity
                } else {
                    compound("exclusive-end", [identity])
                }
            }
        },
    )
}

pub(super) fn array_length(
    program: &TypedTrees,
    symbol: SymbolHandle,
    _spelling: &str,
    context: &TypeIdentityContext<'_>,
) -> Option<String> {
    let (ordinal, reference) = selected(context, symbol)?;
    if context.active_const_substitutions.len() >= 64
        || context.active_const_substitutions.contains(&symbol)
    {
        return Some(rejected(context));
    }
    let mut active = context.active_const_substitutions.to_vec();
    active.push(symbol);
    let nested = TypeIdentityContext {
        active_const_substitutions: &active,
        ..context.replacement_context_at(ordinal)
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
            _ => normalize_type_reference(program, reference, &nested),
        },
    )
}

fn rejected(context: &TypeIdentityContext<'_>) -> String {
    if let Some(invalid) = context.invalid_index_argument {
        invalid.set(true);
    }
    if let Some(missing) = context.missing_exact_nominal_owner {
        missing.set(true);
    }
    "unsupported-const-substitution".to_owned()
}

// A named evaluated Boolean and its literal denote the same exact index.
// Runtime symbols never enter this decoder: only admitted const declarations do.
fn boolean(program: &TypedTrees, symbol: SymbolHandle, spelling: &str) -> Option<bool> {
    let value = if symbol.is_valid() {
        let declaration = program.const_declaration_by_symbol(symbol)?;
        let encoding = declaration.evaluated_value.as_ref()?.encoding()?;
        CanonicalConstValue::new("", encoding.to_owned(), "").decode_encoding()?
    } else {
        if let Some(value) = language_semantics::const_value::boolean_literal_spelling(spelling) {
            return Some(value);
        }
        CanonicalConstValue::from_atom(spelling)?.decode_encoding()?
    };
    match value {
        DecodedCanonicalConstValue::Boolean(value) => Some(value),
        _ => None,
    }
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
    let declaration = program.const_declaration_by_symbol(symbol)?;
    let encoding = declaration.evaluated_value.as_ref()?.encoding()?;
    match CanonicalConstValue::new("", encoding.to_owned(), "").decode_encoding()? {
        DecodedCanonicalConstValue::Integer { value, .. } => Some(value),
        _ => None,
    }
}

impl TypedTrees {
    /// Read a closed array extent through exact declaration substitutions.
    /// Canonical const atoms and evaluated declarations share identity's
    /// decoder; expression operands must already be literal or a selected
    /// const name. This query neither evaluates operators nor admits a binder's
    /// carrier or constraints. Unknown, cyclic and unrepresentable extents fail.
    pub fn closed_array_extent(
        &self,
        length: &crate::types::FixedArrayLength,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<usize> {
        let (mut symbol, mut spelling) = match length {
            crate::types::FixedArrayLength::Literal(length) => return Some(*length),
            crate::types::FixedArrayLength::ConstParameter { symbol, name }
                if symbol.is_valid() =>
            {
                (*symbol, name.as_str())
            }
            _ => return None,
        };
        let mut active = Vec::new();
        loop {
            let replacement = substitutions
                .iter()
                .rev()
                .find(|(parameter, _)| symbol.is_valid() && *parameter == symbol);
            let Some((_, reference)) = replacement else {
                return usize::try_from(integer(self, symbol, spelling)?).ok();
            };
            if active.contains(&symbol)
                || !self
                    .type_reference_table
                    .contains_type_reference(*reference)
            {
                return None;
            }
            active.push(symbol);
            match self.type_reference_table.type_reference(*reference) {
                TypeReferenceNode::Named {
                    symbol: selected,
                    name,
                } => {
                    symbol = *selected;
                    spelling = name.as_str();
                }
                TypeReferenceNode::ConstExpression(expression) => {
                    if !self.expression_table.expression_is_valid(*expression) {
                        return None;
                    }
                    match self.expression_table.expression(*expression) {
                        ExpressionNode::Integer(literal) => {
                            return usize::try_from(literal.value_u64()?).ok();
                        }
                        ExpressionNode::Name(path) if path.symbol.is_valid() => {
                            symbol = path.symbol;
                            spelling = "";
                        }
                        _ => return None,
                    }
                }
                _ => return None,
            }
        }
    }
}
