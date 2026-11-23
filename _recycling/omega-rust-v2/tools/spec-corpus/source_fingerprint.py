"""Identify the V2 source tree and the spec read by spec-corpus, including in an ignored copy."""
import hashlib
import stat
from pathlib import Path


def tree_digest(root, excluded):
    root = root.resolve(strict=True)
    digest = hashlib.sha256()
    files = 0

    def visit(directory, prefix, ancestors):
        nonlocal files
        for entry in sorted(directory.iterdir(), key=lambda path: path.name):
            name = prefix + entry.name
            if entry.name in {".git", "__pycache__"} or name in excluded:
                continue
            # Directory junctions can alias the workspace or its canonical spec.
            # Within a tree, never silently omit a link's input or hash a cycle.
            info = entry.lstat()
            reparse_point = getattr(info, "st_file_attributes", 0) & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0)
            linked = stat.S_ISLNK(info.st_mode) or reparse_point
            target = entry.resolve(strict=True) if linked else entry
            if not target.is_relative_to(root) or target in ancestors:
                raise ValueError(f"source link leaves or loops within {root}: {name}")
            if target.is_dir():
                visit(target, name + "/", ancestors | {target})
            elif target.is_file():
                digest.update(name.encode("utf-8", "surrogateescape") + b"\0")
                digest.update(hashlib.sha256(target.read_bytes()).digest())
                files += 1
            else:
                raise ValueError(f"expected a readable source file: {entry}")

    visit(root, "", {root})
    return {"sha256": digest.hexdigest(), "files": files}


def source_identity(workspace):
    workspace = Path(workspace)
    # Use the same location as spec-corpus/src/lib.rs::repository_spec after
    # resolving the junction. Git's tree at HEAD may describe different bytes.
    spec = workspace.resolve(strict=True).parent / "wiki" / "spec"
    workspace_tree = tree_digest(workspace, {"target", ".agents/worktree"})
    spec_tree = tree_digest(spec, set())
    combined = hashlib.sha256(b"omega-v2-readiness-sources-v1\0")
    for tree in (workspace_tree, spec_tree):
        combined.update(bytes.fromhex(tree["sha256"]))
    return {"fingerprint": combined.hexdigest(), "source_trees": {"v2": workspace_tree, "spec": spec_tree}}
