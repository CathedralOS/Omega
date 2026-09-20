//! Conformance audit for the documented pipeline route.
//!
//! `omega-rust/pipeline.md` declares the connected program route and the Omega
//! frontend stages as labelled owner links, and states that private analyses and
//! target setup are not additional public program stages. These checks pin the
//! map to the disk: every declared owner resolves inside its named crate, and
//! every pipeline crate on disk is a declared route owner — no orphan stages,
//! no duplicate claims on the route.

use super::repository;
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
                    label, crate_dir,
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
    for (parent, name) in pipeline_crates() {
        let parts: Vec<&str> = name.split("-to-").collect();
        assert_eq!(
            parts.len(),
            2,
            "pipeline crate {name} must keep the followable X-to-Y shape"
        );
        for part in parts {
            assert!(
                !part.is_empty()
                    && part
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "pipeline crate {name} has a malformed route segment"
            );
        }
        let manifest =
            std::fs::read_to_string(repository().join(parent).join(&name).join("Cargo.toml"))
                .unwrap_or_else(|_| panic!("read {name}/Cargo.toml"));
        assert!(
            manifest.contains(&format!("name = \"{name}\"")),
            "pipeline crate {name} must publish a package of the same name"
        );
    }
}
