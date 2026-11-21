//! Authored alias parameters and right-hand sides used to instantiate later applications.
use source_text::SourceSpan;
use std::sync::Arc;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{Item, TypeAliasDefinition};

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct AliasBody {
    declarations: Arc<SyntaxTrees>,
    index: usize,
}

impl AliasBody {
    /// Copy only the aliases. Their operands keep the source spans of the declaring file.
    pub fn collect(syntax: &SyntaxTrees) -> Vec<(SourceSpan, Self)> {
        let aliases = syntax.root_items().filter_map(|item| match item {
            Item::TypeAlias(alias) => Some((alias.name.source_span(), item)),
            _ => None,
        });
        let mut aliases = aliases.peekable();
        if aliases.peek().is_none() {
            return Vec::new();
        }
        let mut declarations = SyntaxTrees::new(syntax.source_id);
        let mut names = Vec::new();
        for (name, item) in aliases {
            let copy = declarations.copy_item_from(syntax, item);
            declarations.push_root_item(copy);
            names.push(name);
        }
        let declarations = Arc::new(declarations);
        names
            .into_iter()
            .enumerate()
            .map(|(index, name)| {
                (
                    name,
                    Self {
                        declarations: Arc::clone(&declarations),
                        index,
                    },
                )
            })
            .collect()
    }

    pub fn declaration(&self, name: SourceSpan) -> Option<(&SyntaxTrees, &TypeAliasDefinition)> {
        let handle = self.declarations.root_item_handles().get(self.index)?;
        match self.declarations.root_item(*handle) {
            Item::TypeAlias(alias) if alias.name.source_span() == name => {
                Some((&self.declarations, alias))
            }
            _ => None,
        }
    }
}
