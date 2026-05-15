use std::path::PathBuf;
use std::sync::Arc;

use crate::source::{SourceFile, SourceId, SourceSpan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn from_files(files: Vec<SourceFile>) -> Self {
        Self { files }
    }

    pub fn add(&mut self, path: PathBuf, source: String) -> &SourceFile {
        self.files.push(SourceFile {
            source_id: SourceId(self.files.len()),
            path,
            source: Arc::from(source),
        });

        self.files
            .last()
            .expect("source map should contain added file")
    }

    pub fn get(&self, source_id: SourceId) -> Option<&SourceFile> {
        self.files.get(source_id.0)
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn text_at(&self, source_span: SourceSpan) -> &str {
        self.get(source_span.source_id)
            .map(|file| file.text_at(source_span.span))
            .unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::Span;
    use crate::source::{SourceId, SourceMap, SourceSpan};

    #[test]
    fn resolves_source_span_text() {
        let mut sources = SourceMap::default();
        let source_id = sources
            .add(PathBuf::from("main.omg"), String::from("machine main {}"))
            .source_id;
        let source_span = SourceSpan::new(source_id, Span::new(8, 12));

        assert_eq!(source_id, SourceId(0));
        assert_eq!(sources.text_at(source_span), "main");
    }

    #[test]
    fn invalid_source_span_resolves_to_empty_text() {
        let sources = SourceMap::default();
        let source_span = SourceSpan::new(SourceId(99), Span::new(0, 4));

        assert_eq!(sources.text_at(source_span), "");
    }
}
