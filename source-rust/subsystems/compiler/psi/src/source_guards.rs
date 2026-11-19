use std::{fs, path::Path};

#[test]
fn terminal_module_carries_no_string_identity_fields() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut directories = vec![root.clone()];
    let mut violations = Vec::new();
    while let Some(directory) = directories.pop() {
        let entries = fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| panic!("cannot list {}: {error}", directory.display()))
                .path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if path.is_dir() {
                if !name.starts_with('.') && !matches!(name, "tests" | "examples" | "target") {
                    directories.push(path);
                }
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs")
                || matches!(name, "source_guards.rs" | "tests.rs")
                || name.ends_with("_tests.rs")
            {
                continue;
            }
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            let relative = path
                .strip_prefix(&root)
                .expect("scanned file is inside src");
            for (line, text) in source.lines().enumerate() {
                if text.trim_start().starts_with("//") {
                    continue;
                }
                for field in [
                    "identity: String",
                    "identity: Option<String>",
                    "identity: Vec<String>",
                    "identities: Vec<String>",
                    "identities: HashSet<String>",
                    "identities: BTreeSet<String>",
                ] {
                    if text.contains(field) {
                        violations.push(format!("{}:{}: {field}", relative.display(), line + 1));
                    }
                }
            }
        }
    }
    violations.sort();
    assert!(
        violations.is_empty(),
        "terminal rows must carry interned identity indices, not text: {violations:?}"
    );
}
