//! The connected-pipeline map must describe real owners on disk.
//!
//! `omega-rust/pipeline.md` is the declared program route: every stage it names
//! must link an actual owner file, every transform directory under the two
//! pipeline roots must appear in it, and `X-to-Y` naming is confined to those
//! roots so a stage-shaped crate cannot sit silently outside the route.

use std::fs;
use std::path::Path;

use super::repository;

const PIPELINE_DOCUMENT: &str = "omega-rust/pipeline.md";

const PIPELINE_ROOTS: &[&str] = &["omega-rust/psi/pipeline", "omega-rust/omega/pipeline"];

/// `(target)` fragments of `[label](target)` links in document order.
fn markdown_link_targets(document: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut rest = document;
    while let Some(open) = rest.find("](") {
        let after = &rest[open + 2..];
        let Some(close) = after.find(')') else {
            break;
        };
        targets.push(after[..close].to_owned());
        rest = &after[close + 1..];
    }
    targets
}

/// Immediate child directories of the two pipeline roots, as repository-relative
/// paths. Root documents such as `psi/pipeline/README.md` are files, not stages.
fn pipeline_stage_dirs(root: &Path) -> Vec<String> {
    let mut stages = Vec::new();
    for pipeline_root in PIPELINE_ROOTS {
        for entry in fs::read_dir(root.join(pipeline_root))
            .unwrap_or_else(|error| panic!("{pipeline_root}: {error}"))
        {
            let entry = entry.unwrap_or_else(|error| panic!("{pipeline_root}: {error}"));
            if entry
                .file_type()
                .map(|file_type| file_type.is_dir())
                .unwrap_or(false)
            {
                stages.push(format!(
                    "{pipeline_root}/{}",
                    entry.file_name().to_string_lossy()
                ));
            }
        }
    }
    stages.sort();
    stages
}

fn collect_dirs(directory: &Path, root: &Path, directories: &mut Vec<String>) {
    for entry in
        fs::read_dir(directory).unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
    {
        let entry = entry.unwrap_or_else(|error| panic!("{error}"));
        if !entry
            .file_type()
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false)
        {
            continue;
        }
        let path = entry.path();
        directories.push(
            path.strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/"),
        );
        collect_dirs(&path, root, directories);
    }
}

#[test]
fn pipeline_route_links_resolve_to_real_files() {
    let root = repository();
    let document_path = root.join(PIPELINE_DOCUMENT);
    let document = fs::read_to_string(&document_path)
        .unwrap_or_else(|error| panic!("{}: {error}", document_path.display()));
    let document_dir = document_path.parent().unwrap_or(&root).to_path_buf();
    for target in markdown_link_targets(&document) {
        let path = target.split('#').next().unwrap_or("");
        if path.is_empty() || path.contains("://") {
            continue;
        }
        assert!(
            document_dir.join(path).exists(),
            "pipeline route link resolves to nothing: {target}"
        );
    }
}

#[test]
fn every_pipeline_stage_dir_is_named_in_the_route() {
    let root = repository();
    let document = fs::read_to_string(root.join(PIPELINE_DOCUMENT))
        .unwrap_or_else(|error| panic!("{PIPELINE_DOCUMENT}: {error}"));
    for stage in pipeline_stage_dirs(&root) {
        let short = stage.strip_prefix("omega-rust/").unwrap_or(stage.as_str());
        assert!(
            markdown_link_targets(&document)
                .iter()
                .any(|target| target == short || target.starts_with(&format!("{short}/"))),
            "pipeline stage dir is absent from the connected route: {stage}"
        );
    }
}

#[test]
fn pipeline_stage_names_stay_in_transform_shape() {
    for stage in pipeline_stage_dirs(&repository()) {
        let name = stage.rsplit('/').next().unwrap_or(stage.as_str());
        assert!(
            name.find("-to-").is_some_and(|at| {
                let (input, output) = name.split_at(at);
                let output = &output[4..];
                !input.is_empty()
                    && !output.is_empty()
                    && output
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            }),
            "pipeline stage name is not a transform-shaped X-to-Y name: {stage}"
        );
    }
}

#[test]
fn transform_stages_do_not_live_outside_pipeline_roots() {
    let root = repository();
    let mut directories = Vec::new();
    collect_dirs(&root.join("omega-rust"), &root, &mut directories);
    for directory in directories {
        let name = directory.rsplit('/').next().unwrap_or(directory.as_str());
        if !name.contains("-to-") {
            continue;
        }
        assert!(
            PIPELINE_ROOTS
                .iter()
                .any(|pipeline_root| directory.starts_with(&format!("{pipeline_root}/"))),
            "transform-shaped stage lives outside the pipeline roots: {directory}"
        );
    }
}
