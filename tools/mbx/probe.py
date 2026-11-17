#!/usr/bin/env python3
"""Exercise real checksum-aware cache replay in an exclusively owned directory."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


def main():
    if len(sys.argv) != 3:
        raise SystemExit("usage: tools/mbx/probe.py <mbx-binary> <new-scratch-directory>")
    binary = Path(sys.argv[1]).resolve(strict=True)
    root = Path(sys.argv[2]).absolute()
    root.mkdir(exist_ok=False, parents=True)
    project = root / "project"
    (project / "src").mkdir(parents=True)
    (project / ".cargo").mkdir()
    (project / "Cargo.toml").write_text(
        '[package]\nname="checksum_probe"\nversion="0.0.0"\nedition="2024"\n'
        '[workspace]\n[profile.dev]\nincremental=false\n'
    )
    (project / ".cargo/config.toml").write_text(
        '[unstable]\nchecksum-freshness=true\n[build]\nfingerprint="content"\n'
    )
    source = project / "src/lib.rs"
    source.write_text("pub fn answer() -> u8 { 7 }\n")
    (project / "src/main.rs").write_text(
        "fn main() { std::process::exit(checksum_probe::answer().into()); }\n"
    )
    env = dict(os.environ)
    env.update(
        MBX_CACHE_DIR=str(root / "cache"),
        MBX_SHIMS_DIR=str(root / "cache/shims"),
        MBX_TARGET_ROOT=str(root / "targets"),
        MBX_GC_AUTO="false",
        MBX_REMOTE_MODE="read-only",
        CARGO_TARGET_DIR=str(root / "target"),
        CARGO_HOME=str(root / "cargo"),
        CARGO_INCREMENTAL="0",
        XDG_CONFIG_HOME=str(root / "config"),
    )
    env["RUSTUP_TOOLCHAIN"] = subprocess.check_output(
        ["rustup", "show", "active-toolchain"], text=True
    ).split()[0]
    reports = []
    receipt = {
        "mbx": subprocess.check_output([str(binary), "--version"], text=True).strip(),
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "rustc": subprocess.check_output(["rustc", "-vV"], text=True),
        "steps": reports,
    }

    def build(label, expected):
        env["MBX_STATS_REPORT"] = str(root / f"{label}.json")
        env["MBX_BYPASS_LOG"] = str(root / f"{label}.bypasses.jsonl")
        result = subprocess.run(
            [str(binary), "build", "--message-format=json"],
            cwd=project, env=env, stdin=subprocess.DEVNULL, capture_output=True, text=True,
        )
        (root / f"{label}.stdout").write_text(result.stdout)
        (root / f"{label}.stderr").write_text(result.stderr)
        if result.returncode:
            raise RuntimeError(f"{label} build failed; inspect {root}/{label}.stderr")
        artifacts = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
        executable = next(row["executable"] for row in artifacts if row.get("executable"))
        actual = subprocess.run([executable], stdin=subprocess.DEVNULL).returncode
        assert actual == expected, (label, actual, expected)
        stats = json.loads(Path(env["MBX_STATS_REPORT"]).read_text())
        reports.append({"step": label, "exit": actual, "stats": stats})
        (root / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
        return stats

    first = build("cold", 7)
    target = (root / "target").resolve(strict=True)
    assert target.is_relative_to(root.resolve()), "refusing to remove an unowned build directory"
    checksum_files = {
        str(path.relative_to(target)): path.read_bytes()
        for path in target.rglob("*.d") if b"# checksum:" in path.read_bytes()
    }
    assert checksum_files, "rustc did not emit checksum dependency records"
    # Force a real cache lookup, rather than letting Cargo skip a fresh unit.
    shutil.rmtree(target)
    replay = build("replay", 7)
    assert replay["hits"] > first["hits"], "unchanged compilation did not hit the cache"
    for relative, contents in checksum_files.items():
        assert (target / relative).read_bytes() == contents, relative
    before = source.stat()
    source.write_text(source.read_text().replace("{ 7 }", "{ 9 }"))
    os.utime(source, ns=(before.st_atime_ns, before.st_mtime_ns))
    assert source.stat().st_size == before.st_size
    assert source.stat().st_mtime_ns == before.st_mtime_ns
    changed = build("disguised-edit", 9)
    assert changed["misses"] > 0, "changed source did not miss the cache"
    print(f"cache replay preserved checksum records; same-size/same-mtime edit ran 9: {root}")


if __name__ == "__main__":
    main()
