use super::diff::{Edit, render_hunks};
use super::*;
use crate::{ExternalSourceContext, LocalSourceLimits, resolve_external_local_package_source};
use omega_package_source::VerifiedPackageSourceEntryKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn lines(value: &[u8]) -> Vec<SourceLine<'_>> {
    let count = source_line_count(value);
    split_lines(value, count, count).unwrap()
}

fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time follows Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "omega-package-source-patch-{name}-{}-{stamp}",
        std::process::id()
    ))
}

fn write_package(root: &Path, main: &[u8]) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(
        root.join("build.omg"),
        b"machine build(builder: &mut Build) {\n\
                  builder.package(\"source-review\");\n\
              }\n",
    )
    .unwrap();
    std::fs::write(root.join("main.omg"), main).unwrap();
}

fn make_tree_writable(root: &Path) {
    let Ok(metadata) = std::fs::symlink_metadata(root) else {
        return;
    };
    if metadata.file_type().is_symlink() {
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if metadata.is_dir() { 0o700 } else { 0o600 };
        let _ = std::fs::set_permissions(root, std::fs::Permissions::from_mode(mode));
    }
    #[cfg(not(unix))]
    {
        let mut permissions = metadata.permissions();
        permissions.set_readonly(false);
        let _ = std::fs::set_permissions(root, permissions);
    }
    if metadata.is_dir()
        && let Ok(entries) = std::fs::read_dir(root)
    {
        for entry in entries.flatten() {
            make_tree_writable(&entry.path());
        }
    }
}

fn cleanup(root: &Path) {
    make_tree_writable(root);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn myers_diff_reconstructs_exact_line_edits() {
    let baseline = lines(b"alpha\nbeta\ngamma\nlast");
    let candidate = lines(b"alpha\nchanged\ngamma\nlast\nadded\n");
    let mut budget = DiffBudget::new(PackageSourcePatchLimits::default());
    budget.add_lines(baseline.len(), candidate.len()).unwrap();
    let edits = myers_diff(&baseline, &candidate, &mut budget).unwrap();
    let mut old = Vec::new();
    let mut new = Vec::new();
    for edit in edits {
        match edit {
            Edit::Equal {
                baseline: old_index,
                candidate: new_index,
            } => {
                old.push(baseline[old_index]);
                new.push(candidate[new_index]);
            }
            Edit::Remove { baseline: index } => old.push(baseline[index]),
            Edit::Add { candidate: index } => new.push(candidate[index]),
        }
    }
    assert_eq!(old, baseline);
    assert_eq!(new, candidate);
}

#[test]
fn myers_diff_reconstructs_every_small_repeated_line_sequence() {
    fn sequence(mask: usize, length: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        for index in 0..length {
            bytes.push(if mask & (1 << index) == 0 { b'a' } else { b'b' });
            bytes.push(b'\n');
        }
        bytes
    }

    for baseline_length in 0..=4 {
        for candidate_length in 0..=4 {
            for baseline_mask in 0..(1 << baseline_length) {
                for candidate_mask in 0..(1 << candidate_length) {
                    let baseline_bytes = sequence(baseline_mask, baseline_length);
                    let candidate_bytes = sequence(candidate_mask, candidate_length);
                    let baseline = lines(&baseline_bytes);
                    let candidate = lines(&candidate_bytes);
                    let mut budget = DiffBudget::new(PackageSourcePatchLimits::default());
                    budget.add_lines(baseline.len(), candidate.len()).unwrap();
                    let edits = myers_diff(&baseline, &candidate, &mut budget).unwrap();
                    let mut reconstructed_baseline = Vec::new();
                    let mut reconstructed_candidate = Vec::new();
                    for edit in edits {
                        match edit {
                            Edit::Equal {
                                baseline: old_index,
                                candidate: new_index,
                            } => {
                                reconstructed_baseline.push(baseline[old_index]);
                                reconstructed_candidate.push(candidate[new_index]);
                            }
                            Edit::Remove { baseline: index } => {
                                reconstructed_baseline.push(baseline[index]);
                            }
                            Edit::Add { candidate: index } => {
                                reconstructed_candidate.push(candidate[index]);
                            }
                        }
                    }
                    assert_eq!(reconstructed_baseline, baseline);
                    assert_eq!(reconstructed_candidate, candidate);
                }
            }
        }
    }
}

#[test]
fn hunk_rendering_escapes_control_bytes_and_omits_distant_context() {
    let baseline = lines(b"same\nold\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\nold-two\n");
    let candidate =
        lines(b"same\nnew\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\nnew-two\x00\n");
    let mut budget = DiffBudget::new(PackageSourcePatchLimits::default());
    budget.add_lines(baseline.len(), candidate.len()).unwrap();
    let edits = myers_diff(&baseline, &candidate, &mut budget).unwrap();
    let mut output = BoundedOutput::new(4_096);
    render_hunks(&mut output, &baseline, &candidate, &edits).unwrap();
    let rendered = output.finish();
    assert!(rendered.contains("removed lf old\nadded lf new\n"));
    assert!(rendered.contains("added lf new-two\\x00\n"));
    assert!(!rendered.contains("context lf six\n"));
    assert_eq!(rendered.matches("hunk ").count(), 2);
}

#[test]
fn entry_rendering_retains_line_endings_modes_kinds_and_symlink_spelling() {
    let baseline_file = VerifiedPackageSourceEntryKind::File {
        bytes: b"first\r\nlast".to_vec(),
        executable: false,
    };
    let candidate_file = VerifiedPackageSourceEntryKind::File {
        bytes: b"first\nlast\n".to_vec(),
        executable: true,
    };
    let mut output = BoundedOutput::new(4_096);
    let mut budget = DiffBudget::new(PackageSourcePatchLimits::default());
    render_entry(
        &mut output,
        &mut budget,
        b"control-\x1b.omg",
        Some(&baseline_file),
        Some(&candidate_file),
    )
    .unwrap();
    let rendered = output.finish();
    assert!(rendered.contains("entry control-\\x1b.omg\n"));
    assert!(rendered.contains("baseline_executable false\n"));
    assert!(rendered.contains("candidate_executable true\n"));
    assert!(rendered.contains("removed lf first\\x0d\n"));
    assert!(rendered.contains("added lf first\n"));
    assert!(rendered.contains("removed none last\n"));
    assert!(rendered.contains("added lf last\n"));

    let directory = VerifiedPackageSourceEntryKind::Directory;
    let symlink = VerifiedPackageSourceEntryKind::Symlink {
        target_bytes: b"../target\nspoof".to_vec(),
    };
    let mut output = BoundedOutput::new(4_096);
    render_entry(
        &mut output,
        &mut budget,
        b"changed-kind",
        Some(&directory),
        Some(&symlink),
    )
    .unwrap();
    let rendered = output.finish();
    assert!(rendered.contains("baseline_kind directory\n"));
    assert!(rendered.contains("candidate_kind symlink\n"));
    assert!(rendered.contains("candidate_target ../target\\x0aspoof\n"));
}

#[test]
fn output_ceiling_rejects_without_returning_a_truncated_patch() {
    let mut output = BoundedOutput::new(5);
    output.push("12345").unwrap();
    assert!(matches!(
        output.push("6"),
        Err(PackageSourcePatchError::OutputExceeded {
            maximum_bytes: 5,
            required_at_least: 6,
        })
    ));
}

#[test]
fn diff_work_and_trace_are_independently_bounded() {
    let baseline = lines(b"a\nb\nc\n");
    let candidate = lines(b"x\ny\nz\n");
    let mut work_budget = DiffBudget::new(PackageSourcePatchLimits::new(
        10, 100, 100, 100, 1, 10_000, 10_000,
    ));
    assert!(matches!(
        myers_diff(&baseline, &candidate, &mut work_budget),
        Err(PackageSourcePatchError::DiffWorkExceeded { maximum: 1 })
    ));

    let mut trace_budget = DiffBudget::new(PackageSourcePatchLimits::new(
        10, 100, 100, 100, 10_000, 1, 10_000,
    ));
    assert!(matches!(
        myers_diff(&baseline, &candidate, &mut trace_budget),
        Err(PackageSourcePatchError::DiffTraceExceeded { maximum_cells: 1 })
    ));
}

#[test]
fn custody_patch_is_exact_bounded_and_marks_unreviewable_content() {
    let live = temp_root("live");
    let baseline_cache = temp_root("baseline-cache");
    let candidate_cache = temp_root("candidate-cache");
    let alternate_cache = temp_root("alternate-cache");
    write_package(&live, b"machine first() {\n}\n");
    let context = ExternalSourceContext::derive(b"source-patch-test");
    let baseline = resolve_external_local_package_source(
        &live,
        &baseline_cache,
        LocalSourceLimits::default(),
        context.clone(),
    )
    .unwrap()
    .into_custody();

    std::fs::write(
        live.join("main.omg"),
        b"machine second() {\n    // end_source_patch\n}\n",
    )
    .unwrap();
    std::fs::write(live.join("opaque.bin"), [0, 0xff, b'\n']).unwrap();
    let candidate = resolve_external_local_package_source(
        &live,
        &candidate_cache,
        LocalSourceLimits::default(),
        context,
    )
    .unwrap()
    .into_custody();

    assert_eq!(baseline.key(), candidate.key());
    let patch = render_package_source_patch(
        Some(&baseline),
        &candidate,
        PackageSourcePatchLimits::default(),
    )
    .unwrap();
    assert_eq!(patch.changed_entries(), 2);
    assert_eq!(patch.incomplete_model_content_entries(), 1);
    assert!(patch.requires_standalone_audit());
    assert!(patch.as_str().contains("removed lf machine first() {"));
    assert!(patch.as_str().contains("added lf machine second() {"));
    assert!(patch.as_str().contains("added lf     // end_source_patch"));
    assert!(
        patch
            .as_str()
            .contains("content_review unavailable_binary_or_non_utf8\n")
    );
    assert_eq!(
        patch
            .as_str()
            .lines()
            .filter(|line| *line == "end_source_patch")
            .count(),
        1,
        "source text cannot forge a renderer control lane"
    );
    for private_root in [&live, &baseline_cache, &candidate_cache] {
        assert!(!patch.as_str().contains(&private_root.display().to_string()));
    }

    let defaults = PackageSourcePatchLimits::default();
    let exact = PackageSourcePatchLimits::new(
        defaults.maximum_entries_per_snapshot(),
        defaults.maximum_bytes_per_snapshot(),
        defaults.maximum_metadata_bytes_per_snapshot(),
        defaults.maximum_lines(),
        defaults.maximum_diff_work(),
        defaults.maximum_trace_cells(),
        patch.as_str().len(),
    );
    assert!(render_package_source_patch(Some(&baseline), &candidate, exact).is_ok());
    let short = PackageSourcePatchLimits::new(
        exact.maximum_entries_per_snapshot(),
        exact.maximum_bytes_per_snapshot(),
        exact.maximum_metadata_bytes_per_snapshot(),
        exact.maximum_lines(),
        exact.maximum_diff_work(),
        exact.maximum_trace_cells(),
        exact.maximum_output_bytes() - 1,
    );
    assert!(matches!(
        render_package_source_patch(Some(&baseline), &candidate, short),
        Err(PackageSourcePatchError::OutputExceeded { .. })
    ));

    let metadata_limited = PackageSourcePatchLimits::new(
        defaults.maximum_entries_per_snapshot(),
        defaults.maximum_bytes_per_snapshot(),
        1,
        defaults.maximum_lines(),
        defaults.maximum_diff_work(),
        defaults.maximum_trace_cells(),
        defaults.maximum_output_bytes(),
    );
    assert!(matches!(
        render_package_source_patch(Some(&baseline), &candidate, metadata_limited),
        Err(PackageSourcePatchError::SourceMetadataExceeded {
            side: PackageSourcePatchSide::Baseline,
            maximum_bytes: 1,
        })
    ));
    let line_limited = PackageSourcePatchLimits::new(
        defaults.maximum_entries_per_snapshot(),
        defaults.maximum_bytes_per_snapshot(),
        defaults.maximum_metadata_bytes_per_snapshot(),
        1,
        defaults.maximum_diff_work(),
        defaults.maximum_trace_cells(),
        defaults.maximum_output_bytes(),
    );
    assert!(matches!(
        render_package_source_patch(Some(&baseline), &candidate, line_limited),
        Err(PackageSourcePatchError::TooManyLines { maximum: 1 })
    ));
    let entry_limited = PackageSourcePatchLimits::new(
        1,
        defaults.maximum_bytes_per_snapshot(),
        defaults.maximum_metadata_bytes_per_snapshot(),
        defaults.maximum_lines(),
        defaults.maximum_diff_work(),
        defaults.maximum_trace_cells(),
        defaults.maximum_output_bytes(),
    );
    assert!(matches!(
        render_package_source_patch(Some(&baseline), &candidate, entry_limited),
        Err(PackageSourcePatchError::SourceCustody {
            side: PackageSourcePatchSide::Baseline,
            error: SourceResolveError::TooManyFiles { limit: 1 },
        })
    ));
    let byte_limited = PackageSourcePatchLimits::new(
        defaults.maximum_entries_per_snapshot(),
        1,
        defaults.maximum_metadata_bytes_per_snapshot(),
        defaults.maximum_lines(),
        defaults.maximum_diff_work(),
        defaults.maximum_trace_cells(),
        defaults.maximum_output_bytes(),
    );
    assert!(matches!(
        render_package_source_patch(Some(&baseline), &candidate, byte_limited),
        Err(PackageSourcePatchError::SourceCustody {
            side: PackageSourcePatchSide::Baseline,
            error: SourceResolveError::TooManyBytes { limit: 1 },
        })
    ));

    let alternate = resolve_external_local_package_source(
        &live,
        &alternate_cache,
        LocalSourceLimits::default(),
        ExternalSourceContext::derive(b"different-context"),
    )
    .unwrap()
    .into_custody();
    assert!(matches!(
        render_package_source_patch(
            Some(&baseline),
            &alternate,
            PackageSourcePatchLimits::default()
        ),
        Err(PackageSourcePatchError::PackageKeyMismatch)
    ));

    cleanup(&live);
    cleanup(&baseline_cache);
    cleanup(&candidate_cache);
    cleanup(&alternate_cache);
}
