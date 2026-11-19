//! Parse one in-memory source and retain its tokens for source editing.
#![forbid(unsafe_code)]

use source_text::SourceId;
use syntax_trees::SyntaxTrees;
use tokens::{TokenScratch, TokenStream};

pub struct ParsedText<'text> {
    pub tokens: TokenStream<'text>,
    pub syntax: SyntaxTrees,
}

#[derive(Debug)]
pub enum SourceParseError {
    Lex { message: String },
    Parse { message: String },
}

pub fn parse_text(text: &str) -> Result<ParsedText<'_>, SourceParseError> {
    let tokens =
        source_lexer::lex_unit_reusing(text, &mut TokenScratch::default()).map_err(|error| {
            SourceParseError::Lex {
                message: error.message,
            }
        })?;
    let source = SourceId::default();
    let mut syntax = SyntaxTrees::new(source);
    source_parser::parse_into(&mut syntax, source, &tokens).map_err(|error| {
        SourceParseError::Parse {
            message: error.message,
        }
    })?;
    Ok(ParsedText { tokens, syntax })
}
