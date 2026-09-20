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

// Named input hops are custody/replay lanes: each `*_stage()` accessor may be
// called only by the files that replay or custody-validate the stage it
// names. This table is the complete in-crate inventory — a new call site must
// land in a custody file listed here, and a retired call site must drop its
// row. Downstream consumers (selected-instructions-to-register-homes) are
// outside this scan; `transformation_stage` has no in-crate caller because
// its custody hop is exercised there.
const CUSTODY_HOP_FILES: &[(&str, &[&str])] = &[
    (
        "selected_stage()",
        &["analyses/liveness/staging/validation.rs"],
    ),
    (
        "liveness_stage()",
        &[
            "analyses/legality/compute.rs",
            "analyses/legality/validation.rs",
            "selected_optimization/optimization_output.rs",
        ],
    ),
    (
        "live_range_stage()",
        &[
            "analyses/legality/mod.rs",
            "rewrites/fixed_view/fixed_precolored_segment_homes/validation.rs",
            "rewrites/literal_folds/execution.rs",
        ],
    ),
    ("transformation_stage()", &[]),
    (
        "source_legality_stage()",
        &[
            "rewrites/fixed_view/fixed_view_copies/model.rs",
            "rewrites/fixed_view/fixed_view_copies/validation.rs",
        ],
    ),
    (
        "source_segment_home_stage()",
        &["analyses/reanalysis/validation.rs"],
    ),
];

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

#[test]
fn named_stage_hops_stay_at_custody_sites() {
    let root = crate_src();
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    assert!(!files.is_empty());
    let mut called = vec![false; CUSTODY_HOP_FILES.len()];
    for path in &files {
        let relative = path.strip_prefix(&root).unwrap();
        let source = std::fs::read_to_string(path).unwrap();
        let name = path.display().to_string();
        for (index, (accessor, allowed)) in CUSTODY_HOP_FILES.iter().enumerate() {
            let needle = format!(".{accessor}");
            if !source.contains(&needle) {
                continue;
            }
            called[index] = true;
            assert!(
                allowed.iter().any(|entry| relative == Path::new(entry)),
                "{name} calls {accessor} outside its custody lanes"
            );
        }
    }
    for (index, (accessor, allowed)) in CUSTODY_HOP_FILES.iter().enumerate() {
        for entry in *allowed {
            let file = root.join(entry);
            assert!(
                file.is_file(),
                "custody lane {entry} for {accessor} is gone"
            );
        }
        if !allowed.is_empty() {
            assert!(
                called[index],
                "{accessor} has no caller at its listed custody lanes"
            );
        }
    }
}
