use crate::pipeline::phase_products::ParsedSources;
use crate::pipeline::source::SourceFile;
use crate::source::SourceMap;
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;

#[derive(Default)]
pub struct SourceStorage {
    pub files: Arena<SourceFile>,
    pub sources: SourceMap,
}

impl SourceStorage {
    pub fn extend(&mut self, parsed: ParsedSources) -> Result<(), Vec<Diagnostic>> {
        for parsed_source in parsed.sources.span_or_empty(parsed.batch) {
            let added = self
                .sources
                .add(parsed_source.path.clone(), parsed_source.source.to_string());

            debug_assert_eq!(added.id, parsed_source.file_id);

            self.files.append(SourceFile {
                file_id: parsed_source.file_id,
                path: parsed_source.path.clone(),
                ast: parsed_source.ast.clone(),
            });
        }

        Ok(())
    }

    pub fn next_file_id(&self) -> usize {
        self.sources.len()
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}
