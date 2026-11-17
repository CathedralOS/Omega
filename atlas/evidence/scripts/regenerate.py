"""Regenerate the Atlas evidence with real Cargo metadata and tracked source.

Run from any directory: python3 atlas/evidence/scripts/regenerate.py
Use --source-revision to reproduce the recorded snapshot after atlas-only edits.
The source is read from a verified Git archive; this does not switch Git.
"""
import argparse
import collections
from contextlib import contextmanager
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile

import backend_users
import stage_dependencies
import users
from scanlib import LIMITATIONS, SCOPES, Workspace, scan, write_json


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


@contextmanager
def archived_source(repo, revision):
    """Read exactly one Git tree, independently of the working directory.

    Checking every blob after extraction detects export-ignore/export-subst
    attributes instead of silently scanning an incomplete or altered archive.
    Symlinks are refused: this inventory has none, and source reads must stay
    inside the temporary root. Gitlinks contribute no archived source files.
    """
    records = subprocess.check_output(
        ["git", "ls-tree", "-r", "-z", revision], cwd=repo).split(b"\0")
    blobs = {}
    for record in filter(None, records):
        info, raw_path = record.split(b"\t", 1)
        mode, kind, oid = info.decode().split()
        if kind == "blob":
            if mode == "120000":
                raise RuntimeError("archived evidence does not support source symlinks")
            blobs[raw_path.decode()] = oid
    algorithm = subprocess.check_output(
        ["git", "rev-parse", "--show-object-format"], cwd=repo, text=True).strip()
    with tempfile.TemporaryDirectory(prefix="omega-atlas-source-") as temporary:
        directory = Path(temporary)
        archive = directory / "source.tar"
        root = directory / "source"
        root.mkdir()
        root = root.resolve()
        subprocess.run(["git", "archive", "--format=tar", "--output", str(archive), revision],
                       cwd=repo, check=True)
        with tarfile.open(archive) as source:
            for member in source.getmembers():
                target = (root / member.name).resolve()
                if not target.is_relative_to(root) or not (member.isfile() or member.isdir()):
                    raise RuntimeError(f"unsupported archive member: {member.name}")
            source.extractall(root)
        for name, expected in blobs.items():
            path = root / name
            if not path.is_file():
                raise RuntimeError(f"Git archive omitted tracked source: {name}")
            data = path.read_bytes()
            actual = hashlib.new(algorithm, b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
            if actual != expected:
                raise RuntimeError(f"Git archive changed tracked source: {name}")
        yield root, sorted(blobs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-revision", default="HEAD")
    parser.add_argument("--output-dir", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[3]
    resolved = subprocess.run(
        ["git", "rev-parse", "--verify", args.source_revision + "^{commit}"], cwd=root, text=True, capture_output=True)
    if resolved.returncode:
        raise SystemExit(f"Required source commit {args.source_revision} is unavailable. Fetch it in this repository (git fetch origin {args.source_revision}) before regenerating evidence.")
    revision = resolved.stdout.strip()
    output = args.output_dir or root / "atlas/evidence/data"
    with archived_source(root, revision) as (source, tracked):
        generate(root, source, tracked, revision, output)


def generate(root, source, tracked, revision, output):
    command = ["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"]
    try:
        metadata = json.loads(subprocess.check_output(command, cwd=source))
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"Cargo metadata failed for archived source {revision}; install Cargo and the toolchain recorded in that snapshot. No evidence was written.") from error
    workspace = Workspace(metadata, tracked_files=tracked)
    if workspace.root != source:
        raise RuntimeError("Cargo metadata refers to a different archived workspace")
    output.mkdir(parents=True, exist_ok=True)
    counts = workspace.line_counts()
    groups = {}
    for name, prefixes in {
        **SCOPES, "psi_foundation": ("omega-rust/psi/foundation/",),
        "omega_foundation": ("omega-rust/omega/foundation/",),
        "realization": ("omega-rust/omega/compilation/realization/",), "workspace": ("",),
    }.items():
        members = sorted(n for n, path in workspace.dirs.items() if path.startswith(prefixes))
        groups[name] = {"packages": members, "package_count": len(members),
                        **{key: sum(counts[n][key] for n in members)
                           for key in ("rust_files", "all_rs_lines", "src_rs_lines")}}
    count_policy = "Git-tracked .rs physical lines including blank lines, comments, tests, benches, examples and build.rs; nested packages own their files; actual Cargo output paths excluded. src_rs_lines counts only each package's src subtree, not production-only code."
    write_json(output / "counts.json", {"policy": count_policy, "packages": counts, "groups": groups})
    edges = workspace.edges()
    kinds = collections.Counter(edge["kind"] for edge in edges)
    summary = {"workspace_packages": len(workspace.packages),
               "workspace_members": len(metadata["workspace_members"]),
               "path_dependency_declarations": len(edges), "by_kind": dict(kinds),
               "production_path_dependency_declarations": kinds["normal"] + kinds["build"]}
    write_json(output / "graph.json", {
        "summary": summary, "packages": workspace.dirs, "edges": edges,
        "policy": "Cargo metadata --no-deps workspace path dependency declarations, not resolved active edges or runtime calls. Normal + build are production; dev is separate. Optional and target-conditional declarations are retained.",
    })
    for scope, filename in (("foundation", "usage.json"), ("backend", "usage_be.json")):
        write_json(output / filename, scan(workspace, scope))
    for filename, content in (
        ("users.txt", users.render(workspace)),
        ("backend_users.txt", backend_users.render(workspace, counts)),
        ("stage_dependencies.txt", stage_dependencies.render(workspace)),
    ):
        (output / filename).write_text(content, encoding="utf-8")
    script_dir = Path(__file__).resolve().parent
    generated = ("counts.json", "graph.json", "usage.json", "usage_be.json", "users.txt",
                 "backend_users.txt", "stage_dependencies.txt")
    write_json(output / "manifest.json", {
        "schema_version": 2, "source_revision": revision,
        "source_policy": "Full Git archive of source_revision; every tracked blob is checked against its Git object identity before Cargo metadata or scanning. The pinned Git tree supplies the file inventory; current checkout changes, untracked and generated files are excluded. Symlinks are refused; Gitlink contents are outside the inventory.",
        "metadata_command": " ".join(command),
        "metadata_summary": summary, "counting_policy": count_policy,
        "scanner_sha256": {p.relative_to(root).as_posix(): digest(p) for p in sorted(script_dir.glob("*.py"))},
        "output_sha256": {name: digest(output / name) for name in generated},
        "limitations": LIMITATIONS,
    })
    print(json.dumps({"source_revision": revision, "metadata": summary,
                      "all_rs_lines": {name: group["all_rs_lines"] for name, group in groups.items()}}, sort_keys=True))


if __name__ == "__main__":
    main()
