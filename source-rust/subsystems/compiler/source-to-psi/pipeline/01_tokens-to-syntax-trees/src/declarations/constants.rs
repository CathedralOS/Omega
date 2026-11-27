use crate::errors::parse_error::ParseError;
use crate::expressions::parse::parse_expression_handle;
use crate::token_input::cursor::{Input, ParseResult};
use crate::types::references::parse_type_reference_handle;
use source_text::SourceSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::ConstSyntax;
use syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use tokens::PunctuationKind;

pub(crate) fn parse_const_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ConstSyntax> {
    let (first, input) = input.take_identifier()?;
    let (scope, name, input) = if input.at_punctuation(PunctuationKind::ColonColon) {
        let input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let (name, input) = input.take_identifier()?;
        (first, name, input)
    } else {
        (Identifier::generated(""), first, input)
    };
    let (type_reference, input) = if input.at_punctuation(PunctuationKind::Colon) {
        let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
        parse_type_reference_handle(syntax_trees, input)?
    } else {
        (TypeReferenceHandle::invalid(), input)
    };
    let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
    let start = input;
    let (value, input) = parse_expression_handle(syntax_trees, input.with_constant_position(true))?;
    let input = input.with_constant_position(false);
    let type_reference = if type_reference.is_valid() {
        type_reference
    } else {
        initializer_shown_type(syntax_trees, value, start.source_span_until(input))?
    };
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    Ok((
        ConstSyntax {
            scope,
            name,
            is_public: false,
            type_reference,
            initializer: value,
        },
        input,
    ))
}

/// The type an unannotated constant's initializer shows by itself.
///
/// `const LIMIT = 7u8;` means `const LIMIT: u8 = 7u8;`. A named constant may
/// show its numeric type through a suffix instead of an annotation, and writing
/// both is allowed but unnecessary
/// (`wiki/spec/language/numeric_values.md#literal-landing-and-destinations`).
/// So the parser writes the type that the suffix, the `as` target or the
/// Boolean literal names, and every later stage reads an ordinary declared
/// type. Later uses never choose a constant's type, so nothing outside the
/// initializer is consulted.
///
/// A wholly anonymous initializer such as `2 + 3` names no type, and the spec
/// refuses it at the initializer. An initializer whose type only typing can
/// find, such as a call, another constant, a typed operand or a domain cast,
/// leaves `type_reference` invalid so typing infers it from the initializer.
fn initializer_shown_type(
    syntax_trees: &mut SyntaxTrees,
    initializer: ExpressionHandle,
    span: SourceSpan,
) -> Result<TypeReferenceHandle, ParseError> {
    let name = match shown_type(syntax_trees, initializer) {
        ShownType::Named(name) => name,
        // The cast's target is read-only syntax, so the declaration and the
        // cast can name the same type reference.
        ShownType::Cast(target) => return Ok(target),
        ShownType::Anonymous => {
            return Err(ParseError::at_source_span(
                "this constant has no type annotation and its initializer is wholly anonymous, so the declaration does not say what it holds: annotate the constant, as in `const LIMIT: u8 = 5;`, or give a literal a suffix, as in `5u8`",
                span,
            ));
        }
        ShownType::NeedsTyping => return Ok(TypeReferenceHandle::invalid()),
    };
    Ok(syntax_trees
        .type_references
        .insert(TypeReferenceNode::Named(Identifier::new(name, span))))
}

enum ShownType {
    Named(String),
    Cast(TypeReferenceHandle),
    Anonymous,
    NeedsTyping,
}

fn shown_type(syntax_trees: &SyntaxTrees, expression: ExpressionHandle) -> ShownType {
    match syntax_trees.expressions.expression(expression) {
        ExpressionNode::Integer(literal) => match literal.landing() {
            Some(landing) => ShownType::Named(landing.landed_type.to_string()),
            None => ShownType::Anonymous,
        },
        ExpressionNode::Float(spelling) => match spelling
            .literal(syntax_trees)
            .and_then(|literal| literal.landing())
        {
            Some(format) => ShownType::Named(format.name().to_owned()),
            None => ShownType::Anonymous,
        },
        ExpressionNode::Boolean(_) => ShownType::Named("bool".to_owned()),
        // A cast that adds a domain lands in a qualified type that the cast's
        // carrier alone does not spell.
        ExpressionNode::Cast(cast) if cast.semantic_domain.is_empty() => {
            ShownType::Cast(cast.target_type)
        }
        ExpressionNode::Unary(unary) => shown_type(syntax_trees, unary.operand),
        // `2 + 3` is wholly anonymous. `2u8 + 3` has a typed operand, whose
        // operation selects the carrier; only typing applies that rule.
        ExpressionNode::Binary(binary) => {
            match (
                shown_type(syntax_trees, binary.left),
                shown_type(syntax_trees, binary.right),
            ) {
                (ShownType::Anonymous, ShownType::Anonymous) => ShownType::Anonymous,
                _ => ShownType::NeedsTyping,
            }
        }
        _ => ShownType::NeedsTyping,
    }
}
