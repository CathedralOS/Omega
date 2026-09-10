//! `const` item parsing.
//!
//! `const Type::NAME: TypeReference = <expression>;` — the initializer parses
//! as an ordinary expression. Package/module-scoped `const NAME` uses the same
//! syntax with no type scope; semantic evaluation and resolution happen later.

use crate::parser::expression::parse_expression_handle;
use crate::parser::input::{Input, ParseResult};
use crate::parser::type_reference::parse_type_reference_handle;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::ConstDefinition;
use tokens::PunctuationKind;

pub(super) fn parse_const_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ConstDefinition> {
    let (first, input) = input.take_identifier()?;
    let (scope, name, input) = if input.at_punctuation(PunctuationKind::ColonColon) {
        let input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let (name, input) = input.take_identifier()?;
        (first, name, input)
    } else {
        (Identifier::generated(""), first, input)
    };
    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let (type_reference, input) = parse_type_reference_handle(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
    let (value, input) = parse_expression_handle(syntax_trees, input)?;
    // Name expressions carry coordinates on their path members. A declaration
    // also needs a source-owned initializer root, including a bare const alias.
    if let syntax_trees::expression::ExpressionNode::Name(path) =
        syntax_trees.expressions.expression(value)
    {
        let members = syntax_trees.expressions.identifier_path_members(*path);
        if let (Some(first), Some(last)) = (members.first(), members.last()) {
            let reference = source::SourceSpan::new(
                first.source_span().source_id,
                source::Span::new(first.source_span().span.start, last.source_span().span.end),
            );
            syntax_trees.expressions.set_source_span(value, reference);
        }
    }
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    Ok((
        ConstDefinition {
            scope,
            name,
            is_public: false,
            type_reference,
            value,
            normalization: None,
        },
        input,
    ))
}
