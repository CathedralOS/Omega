//! Exercise installer ownership and replacement without compiling mbx. Only
//! curl, uname, rustup and the downloaded bootstrap are substituted; extraction,
//! patch application, checksums and the real installer remain unchanged.
use super::{Result, fresh, query, write};
use crate::execution;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn executable(path: &Path, contents: &str) -> Result<()> {
    write(path, contents)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())
}

struct Fixture<'a> {
    repo: &'a Path,
    root: PathBuf,
    version: String,
}

impl<'a> Fixture<'a> {
    fn new(repo: &'a Path, parent: &Path, name: &str, archive: &Path) -> Result<Self> {
        let root = fresh(&parent.join(name))?;
        let version = fs::read_to_string(repo.join("tools/mbx-version"))
            .map_err(|e| e.to_string())?
            .trim()
            .to_owned();
        fs::create_dir(root.join("bin")).map_err(|e| e.to_string())?;
        fs::create_dir(root.join("release")).map_err(|e| e.to_string())?;
        write(
            &root.join("source-archive-path"),
            archive.to_string_lossy().as_bytes(),
        )?;
        executable(
            &root.join("release/mbx"),
            r#"#!/usr/bin/env bash
set -eu
test "$1" = build
test "$CARGO_HOME" = "$MBX_INSTALL_WORK/cargo"
test "$MBX_CACHE_DIR" = "$MBX_INSTALL_WORK/cache"
test ! -e unexpected.rs
grep -q checksum-hash-algorithm crates/mbx-cache-rustc/src/lib.rs
grep -Fq "$OMEGA_MBX_TEST_VERSION" crates/mbx/Cargo.toml
mkdir -p "$CARGO_TARGET_DIR/release"
printf '#!/bin/sh\nprintf "mbx %s\\n"\n' "$OMEGA_MBX_TEST_VERSION" > "$CARGO_TARGET_DIR/release/mbx"
chmod +x "$CARGO_TARGET_DIR/release/mbx"
echo build >> "$OMEGA_MBX_TEST_ROOT/events"
"#,
        )?;
        executable(
            &root.join("bin/rustup"),
            "#!/bin/sh\necho nightly-test-x86_64-unknown-linux-gnu\n",
        )?;
        executable(
            &root.join("bin/uname"),
            "#!/bin/sh\nif [ \"$1\" = -s ]; then echo Linux; else echo x86_64; fi\n",
        )?;
        executable(
            &root.join("bin/curl"),
            r#"#!/usr/bin/env bash
set -eu
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) output=$2; shift 2 ;;
    https://*) url=$1; shift ;;
    *) shift ;;
  esac
done
echo download >> "$OMEGA_MBX_TEST_ROOT/events"
case "$url" in
  *codeload.github.com*)
    if [ -f "$OMEGA_MBX_TEST_ROOT/corrupt" ]; then printf corrupt > "$output";
    else cp "$OMEGA_MBX_TEST_ARCHIVE" "$output"; fi ;;
  */SHA256SUMS)
    if command -v sha256sum >/dev/null; then digest=$(sha256sum "$OMEGA_MBX_TEST_ROOT/release.tar.gz");
    else digest=$(shasum -a 256 "$OMEGA_MBX_TEST_ROOT/release.tar.gz"); fi
    printf '%s  mbx-x86_64-unknown-linux-gnu.tar.gz\n' "${digest%% *}" > "$output" ;;
  *) cp "$OMEGA_MBX_TEST_ROOT/release.tar.gz" "$output" ;;
esac
"#,
        )?;
        let mut pack = Command::new("tar");
        pack.arg("-czf")
            .arg(root.join("release.tar.gz"))
            .arg("-C")
            .arg(root.join("release"))
            .arg("mbx");
        if !execution::run(&mut pack, &root, "pack-bootstrap")?.success() {
            return Err("could not prepare bootstrap fixture".into());
        }
        Ok(Self {
            repo,
            root,
            version,
        })
    }

    fn install(&self, archive: &Path, name: &str, success: bool) -> Result<()> {
        let path = std::env::join_paths(std::iter::once(self.root.join("bin")).chain(
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
        ))
        .map_err(|e| e.to_string())?;
        let mut command = Command::new("bash");
        command
            .arg(self.repo.join("tools/mbx/install.sh"))
            .arg(self.root.join("installed"))
            .env("PATH", path)
            .env("MBX_INSTALL_WORK", self.root.join("work"))
            .env("OMEGA_MBX_TEST_ROOT", &self.root)
            .env("OMEGA_MBX_TEST_ARCHIVE", archive)
            .env("OMEGA_MBX_TEST_VERSION", &self.version);
        if execution::run(&mut command, &self.root, name)?.success() != success {
            return Err(format!(
                "unexpected installer result; inspect {}",
                self.root.join(format!("{name}.stderr")).display()
            ));
        }
        Ok(())
    }

    fn events(&self) -> Result<String> {
        fs::read_to_string(self.root.join("events")).map_err(|e| e.to_string())
    }

    fn receipt(&self) -> Result<()> {
        let text = fs::read_to_string(self.root.join("installed/mbx.source"))
            .map_err(|e| e.to_string())?;
        let mut rows = text.lines();
        if !rows.next().is_some_and(|row| row.contains(&self.version))
            || rows.next() != Some(execution::digest(&self.root.join("installed/mbx"))?.as_str())
        {
            return Err("installer receipt lost its source or binary identity".into());
        }
        Ok(())
    }
}

pub(super) fn check(archive: &Path, output: &Path) -> Result<()> {
    let repo = crate::repo_root().map_err(|_| "no Omega checkout")?;
    let archive = archive.canonicalize().map_err(|e| e.to_string())?;
    let root = fresh(output)?;
    let reused = Fixture::new(&repo, &root, "idempotence", &archive)?;
    reused.install(&archive, "first", true)?;
    reused.receipt()?;
    let events = reused.events()?;
    reused.install(&archive, "second", true)?;
    if reused.events()? != events {
        return Err("matching installation downloaded or rebuilt".into());
    }
    if query(&reused.root.join("installed/mbx"), &["--version"])?
        != format!("mbx {}", reused.version)
    {
        return Err("installed version differs".into());
    }

    let damaged = Fixture::new(&repo, &root, "damaged-source", &archive)?;
    fs::create_dir(damaged.root.join("installed")).map_err(|e| e.to_string())?;
    write(&damaged.root.join("installed/mbx"), "existing executable\n")?;
    write(&damaged.root.join("corrupt"), "")?;
    damaged.install(&archive, "rejected", false)?;
    if fs::read_to_string(damaged.root.join("installed/mbx")).map_err(|e| e.to_string())?
        != "existing executable\n"
        || damaged.root.join("installed/mbx.source").exists()
        || damaged.events()?.lines().any(|line| line == "build")
        || !fs::read_to_string(damaged.root.join("rejected.stderr"))
            .map_err(|e| e.to_string())?
            .contains("checksum mismatch")
    {
        return Err("damaged source altered the installation or reached compilation".into());
    }

    let changed = Fixture::new(&repo, &root, "changed-binary", &archive)?;
    changed.install(&archive, "first", true)?;
    let binary = changed.root.join("installed/mbx");
    let original = fs::read_to_string(&binary).map_err(|e| e.to_string())?;
    write(&binary, format!("{original}# locally changed\n"))?;
    changed.install(&archive, "second", true)?;
    if fs::read_to_string(&binary).map_err(|e| e.to_string())? != original
        || changed
            .events()?
            .lines()
            .filter(|line| *line == "build")
            .count()
            != 2
    {
        return Err("same-version binary edit escaped rebuilding".into());
    }
    changed.receipt()?;

    let stale = Fixture::new(&repo, &root, "stale-source", &archive)?;
    fs::create_dir_all(stale.root.join("work/source")).map_err(|e| e.to_string())?;
    let unexpected = stale.root.join("work/source/unexpected.rs");
    write(&unexpected, "caller-owned diagnostic source\n")?;
    stale.install(&archive, "fresh", true)?;
    if fs::read_to_string(&unexpected).map_err(|e| e.to_string())?
        != "caller-owned diagnostic source\n"
    {
        return Err("previous source was deleted or modified".into());
    }
    execution::write_json(
        &root.join("receipt.json"),
        &serde_json::json!({"controls": ["idempotence", "damaged-source", "changed-binary", "stale-source"], "source_archive_sha256": execution::digest(&archive)?}),
    )?;
    println!("four installer controls passed: {}", root.display());
    Ok(())
}
