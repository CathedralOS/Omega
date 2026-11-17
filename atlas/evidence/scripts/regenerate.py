"""Regenerate the Atlas evidence with real Cargo metadata and tracked source.

Run from any directory: python3 atlas/evidence/scripts/regenerate.py
Use --source-revision to reproduce the recorded snapshot after atlas-only edits.
The checkout must match that revision outside atlas; this does not switch Git.
"""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import subprocess

import backend_users
import stage_dependencies
import users
from scanlib import LIMITATIONS, SCOPES, Workspace, scan, write_json


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-revision", default="HEAD")
    parser.add_argument("--output-dir", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[3]
    revision = subprocess.check_output(
        ["git", "rev-parse", args.source_revision + "^{commit}"], cwd=root, text=True).strip()
    subprocess.run(["git", "diff", "--exit-code", revision, "--", ".", ":(exclude)atlas"],
                   cwd=root, stdout=subprocess.DEVNULL, check=True)
    command = ["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"]
    metadata = json.loads(subprocess.check_output(command, cwd=root))
    workspace = Workspace(metadata)
    if workspace.root != root:
        raise RuntimeError("Cargo metadata refers to a different workspace")
    output = args.output_dir or root / "atlas/evidence/data"
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
        "source_policy": "Tracked checkout content outside atlas must match source_revision. Untracked/generated files do not enter the inventory.",
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
