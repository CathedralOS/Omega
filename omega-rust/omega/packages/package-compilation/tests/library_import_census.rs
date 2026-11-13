// Repository-wide census over bundled-library imports in `.omg` sources. The
// standalone `omega::language::std` and `omega::language::alloc` mount paths are
// retired outside the libraries themselves: package-aware consumers must reach
// bundled libraries through declared `build.omg` dependency edges and their
// requester-local aliases (`omega_language_std`, `alloc`). Fail-tier fixtures
// (`tests/fixtures/fail`) and standalone roots without `build.omg` are excluded
// from missing-edge assertions because their rejection is tested by the
// compiler itself.

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
        .expect("package-compilation lives under the omega-rust workspace")
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

fn owning_build_root(file: &Path) -> Option<PathBuf> {
    file.parent()?.ancestors().find_map(|dir| {
        if dir.join("build.omg").is_file() {
            Some(dir.to_path_buf())
        } else {
            None
        }
    })
}

fn dependency_dirs(root: &Path) -> Vec<PathBuf> {
    let Ok(text) = fs::read_to_string(root.join("build.omg")) else {
        return Vec::new();
    };
    let mut dirs = Vec::new();
    let marker = "Source::Path";
    let mut rest = text.as_str();
    while let Some(at) = rest.find(marker) {
        rest = &rest[at + marker.len()..];
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
        let fail_tier = source.starts_with(repo.join("tests/fixtures/fail"))
            || source.starts_with(repo.join("tests/omega/fail"));
        for (head, is_mount) in imports_of(&text) {
            if is_mount && MOUNTED_LIBRARIES.contains(&head.as_str()) {
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
                continue;
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
            if is_under(source, package_dir) {
                continue;
            }
            if fail_tier {
                continue;
            }
            let Some(root) = owning_build_root(source) else {
                continue;
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
