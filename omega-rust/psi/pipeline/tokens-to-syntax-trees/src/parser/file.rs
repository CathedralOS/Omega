use crate::parser::input::{Input, ParseResult};
use crate::parser::item::parse_item;
use syntax_trees::{SyntaxTrees, item::ItemHandle};

pub(super) fn parse_file<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
    root_items: &mut Vec<ItemHandle>,
) -> ParseResult<'tokens, 'source, ()> {
    while !input.tokens.is_empty() {
        let (item, rest) = parse_item(syntax_trees, input)?;
        root_items.push(syntax_trees.push_root_item(item));
        input = rest;
    }

    Ok(((), input))
}
