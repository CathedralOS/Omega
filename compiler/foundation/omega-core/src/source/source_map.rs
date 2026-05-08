use std::path::PathBuf;

use crate::source::{FileId, SourceFile, SourceSpan};

#[derive(Debug, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn add(&mut self, path: PathBuf, source: String) -> &SourceFile {
        self.files.push(SourceFile {
            id: FileId(self.files.len()),
            path,
            source,
        });

        self.files
            .last()
            .expect("source map should contain added file")
    }

    pub fn get(&self, file_id: FileId) -> Option<&SourceFile> {
        self.files.get(file_id.0)
    }

    pub fn text_at(&self, source_span: SourceSpan) -> &str {
        self.get(source_span.file_id)
            .map(|file| file.text_at(source_span.span))
            .unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::Span;
    use crate::source::{FileId, SourceMap, SourceSpan};

    #[test]
    fn resolves_source_span_text() {
        let mut sources = SourceMap::default();
        let file_id = sources
            .add(PathBuf::from("main.omg"), String::from("machine main {}"))
            .id;
        let source_span = SourceSpan::new(file_id, Span::new(8, 12));

        assert_eq!(file_id, FileId(0));
        assert_eq!(sources.text_at(source_span), "main");
    }

    #[test]
    fn invalid_source_span_resolves_to_empty_text() {
        let sources = SourceMap::default();
        let source_span = SourceSpan::new(FileId(99), Span::new(0, 4));

        assert_eq!(sources.text_at(source_span), "");
    }
}
