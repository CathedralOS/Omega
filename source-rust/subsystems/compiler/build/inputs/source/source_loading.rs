//! Acquire one requested source frontier before source settlement parses it.
use artifacts::compile_timings::{CompileTimings, source as stages};
use diagnostics::Diagnostic;
use source_acquisition::capture::SourceFileSystem;
use source_admission::SourceContents;
use source_provenance::DependencyScope;
use std::path::PathBuf;

pub(crate) fn load_source_files(
    frontier: Vec<(PathBuf, DependencyScope)>,
    filesystem: &SourceFileSystem,
    timings: &mut CompileTimings,
) -> Result<Vec<SourceContents>, Vec<Diagnostic>> {
    timings.record_result(stages::LOAD_FILES, || {
        let mut loaded = Vec::with_capacity(frontier.len());

        for (path, scope) in frontier {
            let source = filesystem.read_to_string(&path).map_err(|error| {
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
    })
}

#[cfg(test)]
mod tests {
    use super::CompileTimings;
    use super::load_source_files;
    use source_acquisition::capture::SourceFileSystem;
    use source_provenance::DependencyScope;
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
        let mut timings = CompileTimings::enabled();
        let loaded = load_source_files(
            vec![
                (path.clone(), DependencyScope::Build),
                (path.clone(), DependencyScope::Product),
            ],
            &SourceFileSystem::new(),
            &mut timings,
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
        let errors = load_source_files(
            vec![(unreadable.clone(), DependencyScope::Product)],
            &SourceFileSystem::new(),
            &mut timings,
        )
        .unwrap_err();
        assert_eq!(timings.phases().len(), 1);
        assert_eq!(
            timings.phases()[0].phase,
            "Source acquisition: SourcePaths -> SourceFiles"
        );
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0]
                .message
                .starts_with(&format!("failed to read {}:", unreadable.display()))
        );
    }
}
