use crate::parse_error::ParseError;
use omega_abstract_syntax_tree::item::Item;
use omega_abstract_syntax_tree::tables::AstTables;
use omega_core::source::FileId;
use omega_lexer::Token;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstFile {
    pub file_id: FileId,
    pub items: Vec<Item>,
    pub tables: AstTables,
}

pub fn parse_ast_file(tokens: &[Token<'_>]) -> Result<AstFile, ParseError> {
    parse_ast_file_with_id(FileId::default(), tokens)
}

pub fn parse_ast_file_with_id(
    file_id: FileId,
    tokens: &[Token<'_>],
) -> Result<AstFile, ParseError> {
    crate::parser::parse_ast_file_impl(file_id, tokens)
}

pub fn parse_ast_file_with_source(
    file_id: FileId,
    _source: std::sync::Arc<str>,
    tokens: &[Token<'_>],
) -> Result<AstFile, ParseError> {
    parse_ast_file_with_id(file_id, tokens)
}

pub(crate) fn build_ast_file(file_id: FileId, mut items: Vec<Item>) -> AstFile {
    merge_machine_items(&mut items);
    let tables = AstTables::from_items(&items);

    AstFile {
        file_id,
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
