use crate::{ParsedSource, ParsedSources, SyntaxTrees};
use arena::{Arena, Handle, SealedPagedArena};

pub struct ParsedSyntaxSource {
    pub source: ParsedSource,
    pub syntax: Handle<SyntaxTrees>,
}

impl ParsedSyntaxSource {
    pub const fn source(&self) -> &ParsedSource {
        &self.source
    }

    pub const fn syntax_handle(&self) -> Handle<SyntaxTrees> {
        self.syntax
    }
}

pub struct ParsedSyntaxPages {
    pub pages: SealedPagedArena<SyntaxTrees>,
    pub sources: Vec<ParsedSyntaxSource>,
}

impl ParsedSyntaxPages {
    pub fn sources(&self) -> &[ParsedSyntaxSource] {
        &self.sources
    }

    pub fn syntax(&self, handle: Handle<SyntaxTrees>) -> Option<&SyntaxTrees> {
        self.pages.try_get(handle)
    }

    pub fn syntax_for<T>(&self, handle: Handle<T>) -> Option<&SyntaxTrees> {
        self.syntax(Handle::from_parts(1, handle.generation()))
    }

    pub fn page_count(&self) -> usize {
        self.pages.page_count()
    }

    pub fn into_parts(self) -> (Vec<SyntaxTrees>, ParsedSources) {
        let trees = self.pages.into_pages().into_iter().flatten().collect();
        let mut sources = Arena::with_capacity(self.sources.len());
        let batch = sources.insert_many(self.sources.into_iter().map(|source| source.source));
        (trees, ParsedSources { sources, batch })
    }
}
