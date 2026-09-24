//! Conformance audit for the documented pipeline route.
//!
//! `omega-rust/pipeline.md` declares the connected program route and the Omega
//! frontend stages as labelled owner links, and states that private analyses and
//! target setup are not additional public program stages. These checks pin the
//! map to the disk: every declared owner resolves inside its named crate, every
//! pipeline crate on disk is a declared route owner — no orphan stages, no
//! duplicate claims on the route — and every crate named like a transform
//! keeps its single placement home under `omega-rust/{psi,omega}/pipeline/`,
//! the only directories the route can own.

use super::{pipeline_package_name, repository};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn pipeline_map() -> String {
    std::fs::read_to_string(repository().join("omega-rust/pipeline.md"))
        .expect("read omega-rust/pipeline.md")
}

/// `[label](target)` entries, in document order.
fn links(document: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut rest = document;
    while let Some(open) = rest.find('[') {
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find(']') else {
            break;
        };
        let label = &after_open[..close];
        let after_close = &after_open[close + 1..];
        if let Some(inner) = after_close
            .strip_prefix('(')
            .and_then(|s| s.split_once(')'))
        {
            found.push((label.to_string(), inner.0.to_string()));
        }
        rest = &after_close[1..];
    }
    found
}

/// Repo-root-relative location of a pipeline.md link target.
fn resolve(target: &str) -> PathBuf {
    let target = target.split('#').next().unwrap();
    let path = Path::new(target);
    if path.is_absolute() {
        return path.to_owned();
    }
    let mut base = repository().join("omega-rust");
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                base.pop();
            }
            std::path::Component::Normal(part) => base.push(part),
            _ => {}
        }
    }
    base
}

/// Link-bearing rows of one contiguous markdown table, in row order.
/// Header and separator rows carry no links and drop out.
fn table_rows(document: &str) -> Vec<Vec<(String, String)>> {
    let mut tables = Vec::new();
    let mut current = Vec::new();
    for line in document.lines() {
        if line.trim_start().starts_with('|') {
            current.extend(links(line));
        } else if !current.is_empty() {
            tables.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tables.push(current);
    }
    tables
}

/// The declared route is "connected": each `X-to-Y` row's owner crate begins
/// where the previous row's ended. Backend owners such as `machine-emission`
/// are not `X-to-Y` crates and close a route without a chainable edge.
#[test]
fn route_rows_chain_each_output_into_the_next_input() {
    let document = pipeline_map();
    for table in table_rows(&document) {
        for pair in table.windows(2) {
            let Some((_, output)) = pair[0].0.split_once("-to-") else {
                continue;
            };
            let Some((input, _)) = pair[1].0.split_once("-to-") else {
                continue;
            };
            assert_eq!(
                output, input,
                "route rows {} and {} are not connected: {} ends at {output} \
                 but {} starts at {input}",
                pair[0].0, pair[1].0, pair[0].0, pair[1].0,
            );
        }
    }
}

/// `(parent, crate)` for each crate directory under
/// `omega-rust/{psi,omega}/pipeline/`.
fn pipeline_crates() -> Vec<(String, String)> {
    let mut crates = Vec::new();
    for parent in ["omega-rust/psi/pipeline", "omega-rust/omega/pipeline"] {
        let dir = repository().join(parent);
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|_| panic!("read {parent}")) {
            let entry = entry.unwrap();
            if entry.path().join("Cargo.toml").is_file() {
                crates.push((parent.to_string(), entry.file_name().into_string().unwrap()));
            }
        }
    }
    crates.sort();
    crates
}

#[test]
fn every_declared_route_owner_resolves_inside_its_named_crate() {
    let document = pipeline_map();
    for (label, target) in links(&document) {
        let resolved = resolve(&target);
        assert!(
            resolved.exists(),
            "pipeline.md link [{label}]({target}) does not resolve"
        );
        for prefix in ["psi/pipeline/", "omega/pipeline/"] {
            if let Some(rest) = target.strip_prefix(prefix) {
                let crate_dir = rest.split('/').next().unwrap();
                assert_eq!(
                    label,
                    pipeline_package_name(crate_dir),
                    "route row label {label} must name its owner crate {crate_dir}"
                );
                let remainder = &rest[crate_dir.len() + 1..];
                assert!(
                    remainder == "README.md" || remainder.starts_with("src/"),
                    "route owner for {label} must be a file inside the crate: {target}"
                );
                let crate_root = repository().join("omega-rust").join(prefix).join(crate_dir);
                assert!(
                    crate_root.join("Cargo.toml").is_file(),
                    "declared route owner {crate_dir} has no Cargo.toml"
                );
                assert!(
                    crate_root.join("src/lib.rs").is_file(),
                    "declared route owner {crate_dir} has no src/lib.rs"
                );
            }
        }
    }
}

/// A `#fragment` target names a heading in the linked markdown: the file
/// check above strips the fragment, so a renamed or deleted heading drifts
/// silently. GitHub's anchor form lowercases, drops punctuation other than
/// `-` and `_`, and turns spaces into hyphens.
fn markdown_anchors(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
        .lines()
        .filter_map(|line| {
            let heading = line.trim_start_matches('#');
            (heading != line && heading.starts_with(' ')).then(|| {
                heading
                    .trim()
                    .to_lowercase()
                    .chars()
                    .filter_map(|c| match c {
                        c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
                        ' ' => Some('-'),
                        _ => None,
                    })
                    .collect()
            })
        })
        .collect()
}

#[test]
fn every_doc_link_fragment_resolves_to_a_heading() {
    let document = pipeline_map();
    for (label, target) in links(&document) {
        let Some((_, fragment)) = target.split_once('#') else {
            continue;
        };
        let resolved = resolve(&target);
        assert!(
            resolved.is_file(),
            "pipeline.md link [{label}]({target}) does not resolve to a file"
        );
        assert_eq!(
            resolved.extension().and_then(|ext| ext.to_str()),
            Some("md"),
            "pipeline.md link [{label}]({target}) names a fragment on a non-markdown target"
        );
        assert!(
            markdown_anchors(&resolved)
                .iter()
                .any(|anchor| anchor == fragment),
            "pipeline.md link [{label}]({target}) names no heading in {resolved:?}"
        );
    }
}

#[test]
fn every_pipeline_crate_is_a_documented_route_owner() {
    let document = pipeline_map();
    let mut claims: BTreeMap<String, usize> = pipeline_crates()
        .into_iter()
        .map(|(_, name)| (name, 0))
        .collect();
    for (_, target) in links(&document) {
        for prefix in ["psi/pipeline/", "omega/pipeline/"] {
            if let Some(rest) = target.strip_prefix(prefix) {
                let crate_dir = rest.split('/').next().unwrap();
                if let Some(count) = claims.get_mut(crate_dir) {
                    *count += 1;
                }
            }
        }
    }
    for (name, count) in claims {
        assert_eq!(
            count, 1,
            "pipeline crate {name} must be owned by exactly one documented route \
             row; found {count}"
        );
    }
}

#[test]
fn pipeline_crate_names_and_packages_follow_the_route_shape() {
    for (parent, directory_name) in pipeline_crates() {
        let package_name = pipeline_package_name(&directory_name);
        let parts: Vec<&str> = package_name.split("-to-").collect();
        assert_eq!(
            parts.len(),
            2,
            "pipeline crate {directory_name} must keep the followable X-to-Y shape"
        );
        for part in parts {
            assert!(
                !part.is_empty()
                    && part
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "pipeline crate {directory_name} has a malformed route segment"
            );
        }
        let manifest = std::fs::read_to_string(
            repository()
                .join(parent)
                .join(&directory_name)
                .join("Cargo.toml"),
        )
        .unwrap_or_else(|_| panic!("read {directory_name}/Cargo.toml"));
        assert!(
            manifest.contains(&format!("name = \"{package_name}\"")),
            "pipeline directory {directory_name} must publish package {package_name}"
        );
    }
}

#[test]
fn psi_pipeline_directories_follow_route_order() {
    let mut directory_names: Vec<String> = pipeline_crates()
        .into_iter()
        .filter_map(|(parent, directory_name)| {
            (parent == "omega-rust/psi/pipeline").then_some(directory_name)
        })
        .collect();
    directory_names.sort();

    for (position, directory_name) in directory_names.iter().enumerate() {
        let (ordering_prefix, _) = directory_name.split_once('_').unwrap_or_else(|| {
            panic!("Psi pipeline directory {directory_name} is missing its two-digit prefix")
        });
        assert_eq!(
            ordering_prefix,
            format!("{position:02}"),
            "Psi pipeline directories must follow their connected route order"
        );
    }
}

#[test]
fn omega_pipeline_directories_follow_route_order() {
    const EXPECTED_DIRECTORIES: &[&str] = &[
        "03_terminal-psi-to-abstract-operations",
        "04_abstract-operations-to-abstract-operations",
        "05_abstract-operations-to-target-operations",
        "06_target-operations-to-selected-instructions",
        "07_selected-instructions-to-selected-instructions",
        "08_selected-instructions-to-register-homes",
        "09_register-homes-to-post-allocation-machine",
        "10_post-allocation-machine-to-selected-form-encoding",
        "11_selected-form-encoding-to-resolved-layout",
        "12_resolved-layout-to-resolved-layout",
    ];
    let mut directory_names: Vec<String> = pipeline_crates()
        .into_iter()
        .filter_map(|(parent, directory_name)| {
            (parent == "omega-rust/omega/pipeline").then_some(directory_name)
        })
        .collect();
    directory_names.sort();
    assert_eq!(directory_names, EXPECTED_DIRECTORIES);
}

/// The placement rule's other direction: `omega-rust/{psi,omega}/pipeline/` is
/// the only home for transform crates. An `X-to-Y`/`X-to-X`-named crate nested
/// under any other bucket — or deeper inside `pipeline/` than the direct
/// children the route enumerates — is a stage no route row can own. `target`
/// build trees and hidden directories never carry crate owners, so the scan
/// skips them rather than trusting every workspace subdirectory.
#[test]
fn transform_crates_live_only_in_pipeline_directories() {
    let root = repository();
    let mut misplaced = Vec::new();
    for half in ["psi", "omega"] {
        let pipeline = root.join("omega-rust").join(half).join("pipeline");
        let mut stack = vec![root.join("omega-rust").join(half)];
        while let Some(directory) = stack.pop() {
            for entry in std::fs::read_dir(&directory)
                .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
                .flatten()
            {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = entry.file_name().into_string().unwrap();
                if name == "target" || name.starts_with('.') {
                    continue;
                }
                if name.contains("-to-")
                    && path.join("Cargo.toml").is_file()
                    && path.parent() != Some(pipeline.as_path())
                {
                    misplaced.push(path.strip_prefix(&root).unwrap().to_owned());
                }
                stack.push(path);
            }
        }
    }
    assert!(
        misplaced.is_empty(),
        "transform-named crates outside the pipeline directories: {misplaced:?}"
    );
}
