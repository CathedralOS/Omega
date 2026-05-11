use crate::parser::input::{Input, ParseResult};
use crate::parser::item::parse_item;
use omega_syntax_trees::item::Item;

pub(super) fn parse_file<'tokens, 'source>(
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Vec<Item>> {
    let mut items = Vec::new();

    while !input.tokens.is_empty() {
        let (item, rest) = parse_item(input)?;
        items.push(item);
        input = rest;
    }

    Ok((items, input))
}
