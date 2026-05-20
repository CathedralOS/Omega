use crate::pipeline::frontend::ParsedSources;
use crate::pipeline::source::SourceFile;
use crate::source::SourceMap;
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_syntax_trees::SyntaxTrees;

#[derive(Default)]
pub struct SourceStorage {
    pub files: Arena<SourceFile>,
    pub sources: SourceMap,
    pub syntax_trees: SyntaxTrees,
}

impl SourceStorage {
    pub fn extend(&mut self, parsed: ParsedSources) -> Result<(), Vec<Diagnostic>> {
        for parsed_source in parsed.sources.span_or_empty(parsed.batch) {
            let added = self
                .sources
                .add(parsed_source.path.clone(), parsed_source.source.to_string());

            debug_assert_eq!(added.source_id, parsed_source.source_id);

            self.files.append(SourceFile {
                source_id: parsed_source.source_id,
                path: parsed_source.path.clone(),
                root_items: parsed_source.root_items.clone(),
            });
        }

        Ok(())
    }

    pub fn next_source_id(&self) -> usize {
        self.sources.len()
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}
