//! Prove checksum-aware cache reuse against real compiler output, with every
//! cache and build directory owned by this invocation rather than the workspace.
use std::fs::{self, File, FileTimes};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use crate::execution;

#[cfg(unix)]
mod installer;

type Result<T> = std::result::Result<T, String>;

pub fn run(args: &[String]) -> std::result::Result<(), u8> {
    let result = match args {
        [command, binary, root] if command == "probe" => probe(Path::new(binary), Path::new(root)),
        #[cfg(unix)]
        [command, archive, root] if command == "check-installer" => {
            installer::check(Path::new(archive), Path::new(root))
        }
        _ => Err("usage: cargo xtask mbx-cache probe <mbx> <new-directory>\n       cargo xtask mbx-cache check-installer <source-archive> <new-directory> (Unix)".into()),
    };
    result.map_err(|error| {
        eprintln!("mbx-cache: {error}");
        1
    })
}

fn write(path: &Path, bytes: impl AsRef<[u8]>) -> Result<()> {
    fs::write(path, bytes).map_err(|e| format!("write {}: {e}", path.display()))
}

fn fresh(root: &Path) -> Result<PathBuf> {
    if let Some(parent) = root.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::create_dir(root).map_err(|e| format!("reserve fresh {}: {e}", root.display()))?;
    root.canonicalize().map_err(|e| e.to_string())
}

fn query(program: &Path, arguments: &[&str]) -> Result<String> {
    let result = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if !result.status.success() {
        return Err(format!("{} {arguments:?} failed", program.display()));
    }
    Ok(String::from_utf8_lossy(&result.stdout).trim().to_owned())
}

fn checksums(root: &Path, directory: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) -> Result<()> {
    for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            checksums(root, &path, files)?;
        } else if path.extension().is_some_and(|x| x == "d") {
            let bytes = fs::read(&path).map_err(|e| e.to_string())?;
            if bytes
                .windows(b"# checksum:".len())
                .any(|x| x == b"# checksum:")
            {
                files.push((
                    path.strip_prefix(root)
                        .map_err(|e| e.to_string())?
                        .to_owned(),
                    bytes,
                ));
            }
        }
    }
    Ok(())
}

fn count(stats: &Value, field: &str) -> Result<u64> {
    stats[field]
        .as_u64()
        .ok_or_else(|| format!("cache statistics lost {field}"))
}

fn build(
    binary: &Path,
    root: &Path,
    toolchain: &str,
    label: &str,
    expected: i32,
    receipt: &mut Value,
) -> Result<Value> {
    let stats_path = root.join(format!("{label}-cache.json"));
    let mut command = Command::new(binary);
    command
        .args(["build", "--message-format=json"])
        .current_dir(root.join("project"))
        .env("MBX_CACHE_DIR", root.join("cache"))
        .env("MBX_SHIMS_DIR", root.join("cache/shims"))
        .env("MBX_TARGET_ROOT", root.join("targets"))
        .env("MBX_STATS_REPORT", &stats_path)
        .env(
            "MBX_BYPASS_LOG",
            root.join(format!("{label}-bypasses.jsonl")),
        )
        .env("MBX_GC_AUTO", "false")
        .env("MBX_REMOTE_MODE", "read-only")
        .env("CARGO_TARGET_DIR", root.join("target"))
        .env("CARGO_HOME", root.join("cargo"))
        .env("CARGO_INCREMENTAL", "0")
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("RUSTUP_TOOLCHAIN", toolchain);
    if !execution::run(&mut command, root, label)?.success() {
        return Err(format!(
            "{label} compilation failed; inspect {}",
            root.join(format!("{label}.stderr")).display()
        ));
    }
    let messages =
        fs::read_to_string(root.join(format!("{label}.stdout"))).map_err(|e| e.to_string())?;
    let executable = messages
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find_map(|row| {
            row.get("executable")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .ok_or("Cargo reported no executable")?;
    let status = execution::run(
        &mut Command::new(&executable),
        root,
        &format!("{label}-executable"),
    )?;
    if status.code() != Some(expected) {
        return Err(format!(
            "{label} exited {:?}; expected {expected}",
            status.code()
        ));
    }
    let stats: Value = serde_json::from_slice(&fs::read(&stats_path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    receipt["steps"]
        .as_array_mut()
        .ok_or("probe lost its steps")?
        .push(json!({"step": label, "exit": expected, "stats": stats}));
    execution::write_json(&root.join("receipt.json"), receipt)?;
    Ok(stats)
}

fn probe(binary: &Path, root: &Path) -> Result<()> {
    let binary = binary.canonicalize().map_err(|e| e.to_string())?;
    let root = fresh(root)?;
    let project = root.join("project");
    fs::create_dir_all(project.join("src")).map_err(|e| e.to_string())?;
    fs::create_dir_all(project.join(".cargo")).map_err(|e| e.to_string())?;
    write(
        &project.join("Cargo.toml"),
        "[package]\nname=\"checksum_probe\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[profile.dev]\nincremental=false\n",
    )?;
    write(
        &project.join(".cargo/config.toml"),
        "[unstable]\nchecksum-freshness=true\n[build]\nfingerprint=\"content\"\n",
    )?;
    let source = project.join("src/lib.rs");
    write(&source, "pub fn answer() -> u8 { 7 }\n")?;
    write(
        &project.join("src/main.rs"),
        "fn main() { std::process::exit(checksum_probe::answer().into()); }\n",
    )?;
    let toolchain = query(Path::new("rustup"), &["show", "active-toolchain"])?;
    let toolchain = toolchain
        .split_whitespace()
        .next()
        .ok_or("rustup returned no toolchain")?;
    let mut receipt = json!({"mbx": query(&binary, &["--version"])?, "binary_sha256": execution::digest(&binary)?, "rustc": query(Path::new("rustc"), &["-vV"])?, "steps": []});
    let cold = build(&binary, &root, toolchain, "cold", 7, &mut receipt)?;
    let target = root
        .join("target")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if target == root || !target.starts_with(&root) {
        return Err("refusing to remove an unowned build directory".into());
    }
    let mut files = Vec::new();
    checksums(&target, &target, &mut files)?;
    if files.is_empty() {
        return Err("rustc emitted no checksum dependency records".into());
    }
    // Only the unchanged replay loses its target. The edit below must exercise
    // Cargo's real content freshness with the replayed fingerprints still live.
    fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
    let replay = build(&binary, &root, toolchain, "replay", 7, &mut receipt)?;
    if count(&replay, "hits")? <= count(&cold, "hits")? {
        return Err("unchanged compilation did not hit the cache".into());
    }
    for (relative, bytes) in files {
        if fs::read(target.join(&relative)).map_err(|e| e.to_string())? != bytes {
            return Err(format!(
                "restored dependency file differs: {}",
                relative.display()
            ));
        }
    }
    let before = fs::metadata(&source).map_err(|e| e.to_string())?;
    let modified = before.modified().map_err(|e| e.to_string())?;
    write(&source, "pub fn answer() -> u8 { 9 }\n")?;
    File::options()
        .write(true)
        .open(&source)
        .map_err(|e| e.to_string())?
        .set_times(FileTimes::new().set_modified(modified))
        .map_err(|e| e.to_string())?;
    let after = fs::metadata(&source).map_err(|e| e.to_string())?;
    if before.len() != after.len() || modified != after.modified().map_err(|e| e.to_string())? {
        return Err("edit did not preserve length and modification time".into());
    }
    let changed = build(&binary, &root, toolchain, "disguised-edit", 9, &mut receipt)?;
    if count(&changed, "misses")? == 0 {
        return Err("changed source did not miss the cache".into());
    }
    println!(
        "cache replay preserved checksum records; same-size/same-mtime edit ran 9: {}",
        root.display()
    );
    Ok(())
}
