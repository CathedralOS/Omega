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
    // A shared target directory can retain this binary after its build worktree
    // is removed. Exercise real file reads without depending on that checkout.
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
