
use std::path::PathBuf;

use crate::lexer::TokenStream;
use crate::parser::SyntaxFile;
use crate::source::FileId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub file_id: FileId,
    pub path: PathBuf,
    pub tokens: TokenStream<'static>,
    pub syntax: SyntaxFile,
}

impl SourceFile {
    pub fn syntax(&self) -> &SyntaxFile {
        &self.syntax
    }
}
