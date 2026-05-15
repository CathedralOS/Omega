use crate::parser::input::{Input, ParseResult};
use crate::parser::item::parse_item;
use omega_syntax_trees::SyntaxTrees;

pub(super) fn parse_file<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ()> {
    while !input.tokens.is_empty() {
        let (item, rest) = parse_item(syntax_trees, input)?;
        syntax_trees.push_root_item(item);
        input = rest;
    }

    Ok(((), input))
}
