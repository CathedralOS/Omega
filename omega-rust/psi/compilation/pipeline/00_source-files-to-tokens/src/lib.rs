/*
We load ordered Omega source files into memory and tokenize their UTF-8 source
text into compact `TokenStream` tables (`load_sources`, `lex_sources`, `Lexer`,
and `LexError`).
*/

/*
Our lexical pipeline separates file ingestion (`load_sources`) from parallel
per-source lexical analysis (`lex_sources.rs`) and single-text scanning
(`lexer.rs`). Unreadable files fail during ingestion before any lexical scanning
runs, and in-memory source inputs enter `lex_sources` directly without touching
the filesystem or acquiring package authority.
*/

/*
Stage 00 (`source-files-to-tokens`) is the lexical entrance of the Psi
front-end pipeline, transforming `LoadedSources` into `LexedSources` for
`tokens-to-syntax-trees` (Stage 01).
*/

#![forbid(unsafe_code)]

mod lex_sources;
pub mod lexer;

pub(crate) mod load_sources {
    use diagnostics::Diagnostic;
    use source::{DependencyScope, SourceContents, SourceFileSystem};
    use std::path::PathBuf;

    pub fn load_sources(
        files: Vec<(PathBuf, DependencyScope)>,
        view: &SourceFileSystem,
    ) -> Result<Vec<SourceContents>, Vec<Diagnostic>> {
        let mut loaded = Vec::with_capacity(files.len());

        for (path, scope) in files {
            let source = view.read_to_string(&path).map_err(|error| {
                vec![Diagnostic::error(format!(
                    "failed to read {}: {error}",
                    path.display()
                ))]
            })?;

            loaded.push(SourceContents {
                path,
                source,
                scope,
            });
        }

        Ok(loaded)
    }

    #[cfg(test)]
    pub(crate) mod tests {
        use super::load_sources;
        use source::{DependencyScope, SourceFileSystem};
        use std::io::Write;
        use std::path::PathBuf;

        struct TemporarySourceFile(PathBuf);

        impl Drop for TemporarySourceFile {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }

        #[test]
        fn loading_preserves_order_and_distinct_scopes_and_reports_unreadable_paths() {
            let source = "// UTF-8 source: \u{03bb}\ndata Item {}\n";
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "omega-source-loading-{}-{nonce}.omg",
                std::process::id()
            ));
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            let _temporary_source = TemporarySourceFile(path.clone());
            file.write_all(source.as_bytes()).unwrap();
            drop(file);
            let loaded = load_sources(
                vec![
                    (path.clone(), DependencyScope::Build),
                    (path.clone(), DependencyScope::Product),
                ],
                &SourceFileSystem::new(),
            )
            .unwrap();
            let batch = loaded.as_slice();
            assert_eq!(batch.len(), 2);
            for (position, scope) in [DependencyScope::Build, DependencyScope::Product]
                .into_iter()
                .enumerate()
            {
                assert_eq!(batch[position].path, path);
                assert_eq!(batch[position].scope, scope);
                assert_eq!(batch[position].source.as_ref(), source);
            }

            let unreadable = path.join("not-a-directory");
            let errors = load_sources(
                vec![(unreadable.clone(), DependencyScope::Product)],
                &SourceFileSystem::new(),
            )
            .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert!(
                errors[0]
                    .message
                    .starts_with(&format!("failed to read {}:", unreadable.display()))
            );
        }
    }
}

pub use lex_sources::lex_sources;
pub use lexer::Lexer;
pub use lexer::lex_error::LexError;
pub use load_sources::load_sources;
