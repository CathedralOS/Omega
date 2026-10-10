//! Census over every real library import in the repository. The standalone
//! `omega::language::std`/`alloc` mount is retired: package-aware consumers
//! reach bundled libraries through ordinary dependency edges and the
//! requester-local alias the edge grants (`omega_language_std`, `alloc`).
//! This inventory is the replacement for the deleted build-file census test:
//! it covers build-scope sources, fail-tier fixtures and imports below member
//! directories. A root without `build.omg` is not migration debt — its
//! imports are compilation errors, not missing edges — and a library needs
//! no edge to itself.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const SCAN_ROOTS: &[&str] = &["tests", "samples", "source", "bootstrap"];
const SKIPPED_DIRS: &[&str] = &["target", "build", ".git", "node_modules"];
const MOUNTED_LIBRARIES: &[&str] = &["std", "alloc"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|directory| directory.join("rust-toolchain.toml").is_file())
        .expect("compilation-inputs lives under the omega-rust workspace")
        .to_path_buf()
}

fn collect_omega_files(root: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if SKIPPED_DIRS
                .iter()
                .any(|name| path.file_name().map(|file| file == *name).unwrap_or(false))
            {
                continue;
            }
            collect_omega_files(&path, out);
        } else if path.extension().map(|ext| ext == "omg").unwrap_or(false) {
            out.push(path);
        }
    }
}

/// Maps every bundled library package name to its package directory,
/// discovered from each library's own `Build::package` declaration.
fn bundled_packages(repo: &Path) -> BTreeMap<String, PathBuf> {
    let mut packages = BTreeMap::new();
    let library_root = repo.join("source/library");
    let mut package_dirs = Vec::new();
    if let Ok(entries) = fs::read_dir(&library_root) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                package_dirs.push(entry.path());
            }
        }
    }
    for package_dir in package_dirs {
        let build = package_dir.join("build.omg");
        let Ok(text) = fs::read_to_string(&build) else {
            continue;
        };
        if let Some(name) = package_name(&text) {
            packages.insert(name, package_dir);
        }
    }
    packages
}

fn package_name(build_text: &str) -> Option<String> {
    let marker = "Build::package(builder,";
    let start = build_text.find(marker)? + marker.len();
    let rest = build_text[start..].trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Every `use <head>::` and every `omega::language::<name>` mount import in
/// one source file, as (head, is_mount_path) pairs in order of appearance.
fn imports_of(text: &str) -> Vec<(String, bool)> {
    let mut imports = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("use ") else {
            continue;
        };
        if let Some(path) = rest.strip_prefix("omega::language::") {
            let head: String = path
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            imports.push((head, true));
        } else {
            let head: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            imports.push((head, false));
        }
    }
    imports
}

/// The nearest ancestor directory carrying `build.omg`, or `None` for a
/// standalone root. Member directories fall through to the root that owns
/// their package — the root's edges grant the aliases they import through.
fn owning_build_root(file: &Path) -> Option<PathBuf> {
    file.parent()?.ancestors().find_map(|dir| {
        if dir.join("build.omg").is_file() {
            Some(dir.to_path_buf())
        } else {
            None
        }
    })
}

/// Package directories this root's `build.omg` depends on, resolved against
/// the root directory.
fn dependency_dirs(root: &Path) -> Vec<PathBuf> {
    let Ok(text) = fs::read_to_string(root.join("build.omg")) else {
        return Vec::new();
    };
    let mut dirs = Vec::new();
    let marker = "Source::Path";
    let mut rest = text.as_str();
    while let Some(at) = rest.find(marker) {
        rest = &rest[at + marker.len()..];
        // The location path is the first quoted string inside the record.
        let Some(from) = rest.find('"') else {
            break;
        };
        let quoted = &rest[from + 1..];
        let Some(end) = quoted.find('"') else {
            break;
        };
        dirs.push(root.join(&quoted[..end]));
        rest = &quoted[end..];
    }
    dirs
}

fn is_under(dir: &Path, ancestor: &Path) -> bool {
    dir.canonicalize()
        .ok()
        .zip(ancestor.canonicalize().ok())
        .map(|(dir, ancestor)| dir.starts_with(ancestor))
        .unwrap_or_else(|| dir.starts_with(ancestor))
}

#[test]
fn library_imports_ride_declared_edges_not_the_mount() {
    let repo = repo_root();
    let packages = bundled_packages(&repo);
    assert!(
        packages.contains_key("omega_language_std"),
        "the census must see the std package declaration"
    );

    let mut sources = Vec::new();
    for root in SCAN_ROOTS {
        collect_omega_files(&repo.join(root), &mut sources);
    }
    sources.sort();

    let mut mount_path_offenders = Vec::new();
    let mut missing_edge_offenders = Vec::new();
    let mut inventoried = 0usize;

    for source in &sources {
        let Ok(text) = fs::read_to_string(source) else {
            continue;
        };
        // fail-tier fixtures author rejections — including the missing-edge
        // and mount-path pins — so they join the inventory but carry no
        // assertions.
        let fail_tier = source.starts_with(repo.join("tests/omega/fail"));
        for (head, is_mount) in imports_of(&text) {
            if is_mount && MOUNTED_LIBRARIES.contains(&head.as_str()) {
                // The retired mount survives only inside the library it names:
                // std's own files may keep importing `omega::language::std`
                // because a package needs no edge to itself.
                let self_dir = repo.join("source/library").join(&head);
                if !fail_tier && !is_under(source, &self_dir) {
                    mount_path_offenders.push(format!(
                        "{} imports `omega::language::{head}`",
                        source.display()
                    ));
                }
                continue;
            }
            let Some(package_dir) = packages.get(&head) else {
                continue; // not a bundled-library alias
            };
            if is_mount {
                if !fail_tier {
                    mount_path_offenders.push(format!(
                        "{} imports `omega::language::{head}`",
                        source.display()
                    ));
                }
                continue;
            }
            inventoried += 1;
            // A library's own sources need no edge to themselves.
            if is_under(source, package_dir) {
                continue;
            }
            if fail_tier {
                continue;
            }
            let Some(root) = owning_build_root(source) else {
                continue; // a root without build.omg is not debt
            };
            let reached = dependency_dirs(&root)
                .iter()
                .any(|dir| is_under(package_dir, dir) || is_under(dir, package_dir));
            if !reached {
                missing_edge_offenders.push(format!(
                    "{} imports `{head}` but {} declares no edge to {}",
                    source.display(),
                    root.display(),
                    package_dir.display()
                ));
            }
        }
    }

    assert!(
        mount_path_offenders.is_empty(),
        "standalone std/alloc mount imports retired; offending sources:\n{}",
        mount_path_offenders.join("\n")
    );
    assert!(inventoried > 0, "the census inventoried no alias imports");
    assert!(
        missing_edge_offenders.is_empty(),
        "alias imports without a declared dependency edge:\n{}",
        missing_edge_offenders.join("\n")
    );
}
