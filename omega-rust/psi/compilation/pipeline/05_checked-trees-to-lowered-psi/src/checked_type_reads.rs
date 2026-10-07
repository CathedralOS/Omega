//! Source guard keeping resolved expression and carrier types with checking.
//! Split needles keep the guard's own source from matching forbidden names.

use std::fs;
use std::path::Path;

#[test]
fn lowering_reads_checked_type_facts_without_reconstruction() {
    let forbidden = [
        concat!("declared_place_", "type_raw"),
        concat!("expression_result_", "type_reference"),
        concat!("unwrapped_", "type_reference"),
    ];
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut directories = vec![source_root.clone()];
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
            if path.extension().and_then(|extension| extension.to_str()) != Some("rs")
                || name == "tests.rs"
                || name.ends_with("_tests.rs")
            {
                continue;
            }
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            for (line_number, line) in source.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                for name in forbidden {
                    if line.contains(name) {
                        violations.push(format!(
                            "{}:{}: {name}",
                            path.strip_prefix(&source_root)
                                .expect("scanned source stays within the crate")
                                .display(),
                            line_number + 1,
                        ));
                    }
                }
            }
        }
    }
    violations.sort();
    assert!(
        violations.is_empty(),
        "lowering must read checked place/result/carrier facts: {violations:?}"
    );
}
