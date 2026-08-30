mod declarations;
mod external_local;
mod git;
mod workspace;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should follow Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "omega-package-source-{name}-{}-{stamp}",
        std::process::id()
    ))
}

fn write_package(root: &Path, name: &str) {
    std::fs::create_dir_all(root).expect("create package root");
    std::fs::write(
        root.join("build.omg"),
        format!("machine build(builder: &mut Build) {{\n    builder.package(\"{name}\");\n}}\n"),
    )
    .expect("write package declaration");
    std::fs::write(root.join("main.omg"), "machine Main::main() {}\n")
        .expect("write package source");
}

#[cfg(unix)]
fn make_tree_owner_writable(root: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut directories = vec![root.to_path_buf()];
    let mut cursor = 0;
    while cursor < directories.len() {
        let directory = directories[cursor].clone();
        cursor += 1;
        if let Ok(entries) = std::fs::read_dir(&directory) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    directories.push(path);
                }
            }
        }
    }
    for directory in directories.into_iter().rev() {
        if let Ok(metadata) = std::fs::symlink_metadata(&directory) {
            let mut permissions = metadata.permissions();
            permissions.set_mode(permissions.mode() | 0o700);
            let _ = std::fs::set_permissions(directory, permissions);
        }
    }
}

#[cfg(not(unix))]
fn make_tree_owner_writable(_root: &Path) {}
