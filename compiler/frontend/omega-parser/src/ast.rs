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
