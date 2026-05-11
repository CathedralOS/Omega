use std::path::PathBuf;

use crate::ast::AstFile;
use crate::source::FileId;
use omega_abstract_syntax_tree::item::Item;
use omega_abstract_syntax_tree::tables::AstTables;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub file_id: FileId,
    pub path: PathBuf,
    pub ast: AstFile,
}

impl Default for SourceFile {
    fn default() -> Self {
        Self {
            file_id: FileId::default(),
            path: PathBuf::default(),
            ast: AstFile {
                file_id: FileId::default(),
                items: Vec::new(),
                tables: AstTables::default(),
            },
        }
    }
}

impl SourceFile {
    pub fn items(&self) -> &[Item] {
        &self.ast.items
    }
}
