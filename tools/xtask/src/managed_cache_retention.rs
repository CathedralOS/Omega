//! Retain completed managed-session builds between compilation and measurement.
//!
//! mbx rejects collection policy in workspace `.mbx.toml`. Managed setup instead
//! appends this checkout's shared worker pool to global `target.keep`, leaving
//! operator overrides and the rest of collection policy intact.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

fn output(command: &mut Command) -> Result<String, String> {
    let result = command.output().map_err(|e| e.to_string())?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).trim().to_owned());
    }
    String::from_utf8(result.stdout).map_err(|e| e.to_string())
}

/// Resolve a pool that setup has not created yet, including symlinked parents.
fn resolve_pool(path: &Path) -> Result<PathBuf, String> {
    let mut existing = path.to_owned();
    let mut missing: Vec<OsString> = Vec::new();
    loop {
        match fs::canonicalize(&existing) {
            Ok(mut root) => {
                for component in missing.into_iter().rev() {
                    root.push(component);
                }
                return Ok(root);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let component = existing.file_name().ok_or_else(|| e.to_string())?;
                missing.push(component.to_owned());
                if !existing.pop() {
                    return Err(e.to_string());
                }
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

fn shared_pool(repo: &Path) -> Result<PathBuf, String> {
    // Match warm_build.sh's common-root discovery, including a detached worker.
    let common = output(Command::new("git").arg("-C").arg(repo).args([
        "rev-parse",
        "--path-format=absolute",
        "--git-common-dir",
    ]))?;
    let common = Path::new(common.trim());
    let primary = common
        .parent()
        .ok_or("Git common directory has no parent")?;
    resolve_pool(&primary.join(".agents/worktree"))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Keep {
    keep: Vec<String>,
}

fn parse_keep(raw: &str) -> Result<Vec<String>, String> {
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    toml::from_str::<Keep>(&format!("keep = {raw}"))
        .map(|parsed| parsed.keep)
        .map_err(|e| format!("cannot read mbx target.keep: {e}"))
}

fn append_pool(keep: &mut Vec<String>, pool: &str) -> Result<bool, String> {
    if keep.iter().any(|entry| entry == pool) {
        return Ok(false);
    }
    // mbx's validated setter accepts a comma-separated list. Refuse a value
    // it cannot represent rather than silently changing existing path entries.
    if keep
        .iter()
        .map(String::as_str)
        .chain([pool])
        .any(|entry| entry.contains(',') || entry != entry.trim())
    {
        return Err("cannot preserve target.keep through mbx's comma-list setter; update its TOML list manually".to_owned());
    }
    keep.push(pool.to_owned());
    Ok(true)
}

fn configure(repo: &Path) -> Result<(), String> {
    let pool = shared_pool(repo)?;
    let pool = pool.to_str().ok_or("worker pool path is not UTF-8")?;
    let raw = output(Command::new("mbx").args(["settings", "get", "target.keep"]))?;
    let mut keep = parse_keep(&raw)?;
    if append_pool(&mut keep, pool)? {
        output(Command::new("mbx").args(["settings", "set", "target.keep", &keep.join(",")]))?;
        println!("managed cache retention: keeping shared pool: {pool}");
    } else {
        println!("managed cache retention: pool already kept: {pool}");
    }
    Ok(())
}

pub fn run(args: &[String]) -> Result<(), u8> {
    let repo = match args {
        [] => crate::repo_root(),
        [flag, path] if flag == "--repository" => PathBuf::from(path),
        _ => {
            eprintln!("usage: cargo xtask managed-cache-retention [--repository <dir>]");
            return Err(2);
        }
    };
    let managed = std::env::var_os("CLAUDE_CODE_REMOTE").as_deref() == Some(OsStr::new("true"))
        || std::env::var_os("CODEX_ENVIRONMENT_ID").is_some_and(|value| !value.is_empty());
    if !managed {
        return Ok(());
    }
    if std::env::var_os("MBX_TARGET_KEEP").is_some() {
        println!("managed cache retention: using explicit MBX_TARGET_KEEP override");
        return Ok(());
    }
    configure(&repo).map_err(|e| {
        eprintln!("managed cache retention: {e}");
        1
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_toml_lists_preserve_quoted_paths_and_reject_other_types() {
        assert_eq!(parse_keep("").unwrap(), Vec::<String>::new());
        assert_eq!(
            parse_keep(r#"["space path", 'path-with"quotes', 'C:\other\retained']"#).unwrap(),
            ["space path", "path-with\"quotes", r"C:\other\retained"]
        );
        assert!(parse_keep("[3]").is_err());
        assert!(parse_keep("false").is_err());
    }

    #[test]
    fn append_is_idempotent_and_never_loses_unrepresentable_existing_entries() {
        let mut keep = vec!["other path".to_owned()];
        assert!(append_pool(&mut keep, "/pool").unwrap());
        assert!(!append_pool(&mut keep, "/pool").unwrap());
        assert_eq!(keep, ["other path", "/pool"]);
        for entry in ["old,comma", " spaced "] {
            let mut keep = vec![entry.to_owned()];
            assert!(append_pool(&mut keep, "/pool").is_err());
            assert_eq!(keep, [entry]);
        }
    }
}
