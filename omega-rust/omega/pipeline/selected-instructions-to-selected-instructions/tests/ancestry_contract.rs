//! The staged optimizer types in this crate retain their producer stages as
//! replay and custody evidence only. Ordinary consumers read the current
//! program and facts through the direct accessors — selected,
//! register_environment, selections, budget_per_pass, liveness, ranges,
//! legality — instead of climbing
//! `liveness_stage().selected_stage().optimized_target()`.
//! `optimized_target_owner` is the single sanctioned ancestry walk: it
//! returns the retained proof-input `Arc` custody checks compare by
//! identity. Named input hops (`live_range_stage`, `liveness_stage`,
//! `source_legality_stage`, `transformation_stage`,
//! `source_segment_home_stage` passed to custody validators) stay: they name
//! the stage under inspection, not a data read.

use std::path::{Path, PathBuf};

fn crate_src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

#[test]
fn staged_types_read_current_data_not_producer_ancestry() {
    let root = crate_src();
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    assert!(!files.is_empty());
    for path in &files {
        let source = std::fs::read_to_string(path).unwrap();
        let name = path.display().to_string();
        assert!(
            !source.contains(".optimized_target()"),
            "{name} reads the retained proof input as data"
        );
        assert!(
            !source.contains("selected_stage()") || source.contains("optimized_target_owner"),
            "{name} walks producer ancestry outside optimized_target_owner"
        );
    }
}
