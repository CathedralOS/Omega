use crate::parse_error::ParseError;
use omega_syntax_trees::item::Item;
use omega_syntax_trees::tables::AstTables;
use omega_core::source::SourceId;
use omega_tokens::Token;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceTrees {
    pub source_id: SourceId,
    pub items: Vec<Item>,
    pub tables: AstTables,
}

pub fn parse_source_trees(tokens: &[Token<'_>]) -> Result<SourceTrees, ParseError> {
    parse_source_trees_with_id(SourceId::default(), tokens)
}

pub fn parse_source_trees_with_id(
    source_id: SourceId,
    tokens: &[Token<'_>],
) -> Result<SourceTrees, ParseError> {
    crate::parser::parse_source_trees_impl(source_id, tokens)
}

pub fn parse_source_trees_with_source(
    source_id: SourceId,
    _source: std::sync::Arc<str>,
    tokens: &[Token<'_>],
) -> Result<SourceTrees, ParseError> {
    parse_source_trees_with_id(source_id, tokens)
}

pub(crate) fn build_source_trees(source_id: SourceId, mut items: Vec<Item>) -> SourceTrees {
    merge_machine_items(&mut items);
    let tables = AstTables::from_items(&items);

    SourceTrees {
        source_id,
        items,
        tables,
    }
}

fn merge_machine_items(items: &mut Vec<Item>) {
    let mut merged = Vec::with_capacity(items.len());

    for item in items.drain(..) {
        match item {
            Item::Machine(machine) => {
                if let Some(Item::Machine(existing)) = merged
                    .iter_mut()
                    .find(|existing_item| matches!(existing_item, Item::Machine(existing) if existing.name == machine.name))
                {
                    existing.contains.extend(machine.contains);
                    existing.owned_data.extend(machine.owned_data);
                    existing.states.extend(machine.states);
                } else {
                    merged.push(Item::Machine(machine));
                }
            }
            other => merged.push(other),
        }
    }

    *items = merged;
}
