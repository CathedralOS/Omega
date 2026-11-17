"""Shared textual evidence collection; this is not a Rust resolver.

Only Git-tracked Rust files belong to this inventory. Cargo's output directory
is excluded by its full path, never by a directory's name: backend/target is
source. A nested package owns its own files, so counts never double-count it.
"""
import collections
import json
from pathlib import Path
import re
import subprocess


SCOPES = {
    "foundation": ("omega-rust/psi/foundation/", "omega-rust/omega/foundation/"),
    "backend": ("omega-rust/omega/compilation/backend/",),
}
LIMITATIONS = [
    "Textual observations, not Rust name resolution or a dependency-removal proof.",
    "Normal and build dependencies include optional and target-conditional declarations; features and target cfg are not evaluated.",
    "Tests, benches, examples, conventional test filenames and exact cfg(test) items are excluded from usage, but complex cfg expressions are not evaluated.",
    "Macros are not expanded; generated and untracked source is excluded; reexports are credited to the named crate, not traced to their origin.",
    "Import aliases and glob presence are observed, but shadowing, glob contents and fully qualified type paths are not resolved.",
    "An unobserved dependency remains unverified until a suitable Cargo check and manual review establish whether it can be removed.",
]
CHAR_LITERAL = re.compile(r"'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^'\\\n])'")
SKIP_NAME = re.compile(r"(^tests\.rs$|_tests?\.rs$|^test_.*\.rs$|^source_guards\.rs$)")


def position(path):
    for prefix, label in [
        ("omega-rust/psi/compilation/pipeline/", "psi-stage "),
        ("omega-rust/omega/compilation/pipeline/", "omega-stage "),
        ("omega-rust/psi/compilation/representations/", "psi-repr "),
        ("omega-rust/omega/compilation/representations/", "omega-repr "),
        ("omega-rust/psi/foundation/", "psi-fdn "),
        ("omega-rust/omega/foundation/", "omega-fdn "),
        ("omega-rust/psi/compilation/", "psi-comp "),
        ("omega-rust/omega/compilation/backend/", "backend "),
        ("omega-rust/omega/compilation/realization/", "realization "),
        ("omega-rust/omega/compilation/", "omega-comp "),
        ("omega-rust/psi/interpreter", "psi-interp "),
        ("omega-rust/omega/interpreter/", "omega-interp "),
        ("omega-rust/omega/packages/", "packages "),
        ("omega-rust/omega/build-system/", "build "),
        ("omega-rust/omega/application", "cli "),
        ("tools/", "tools "),
        ("samples/", "samples "),
    ]:
        if path.startswith(prefix):
            rest = path[len(prefix):]
            if label.strip() in ("psi-stage", "omega-stage"):
                rest = rest.split("/")[0].split("_")[0]
            elif label.strip() in ("psi-repr", "omega-repr"):
                rest = rest.replace("/", ":")
            return (label + rest).strip()
    return path


class Workspace:
    def __init__(self, metadata, tracked_files=None):
        self.root = Path(metadata["workspace_root"]).resolve()
        members = set(metadata["workspace_members"])
        self.packages = {p["name"]: p for p in metadata["packages"] if p["id"] in members}
        self.dirs = {name: Path(p["manifest_path"]).parent.resolve().relative_to(self.root).as_posix()
                     for name, p in self.packages.items()}
        self.by_dir = {directory: name for name, directory in self.dirs.items()}
        self.outputs = [Path(metadata[key]).resolve() for key in ("target_directory", "build_directory")
                        if metadata.get(key)]
        # Archive callers supply the source revision's inventory. Never fall
        # back to the surrounding checkout when an explicit list is present.
        tracked = tracked_files if tracked_files is not None else subprocess.check_output(
            ["git", "-C", str(self.root), "ls-files", "-z", "--", "*.rs"]
        ).decode().split("\0")
        self.files = collections.defaultdict(list)
        for relative in sorted(p for p in tracked if p and p.endswith(".rs")):
            path = self.root / relative
            if any(path.is_relative_to(output) for output in self.outputs):
                continue
            # Walk up from the file, so a nested crate wins over its parent.
            for directory in path.relative_to(self.root).parents:
                name = self.by_dir.get(directory.as_posix())
                if name:
                    self.files[name].append(path)
                    break

    def dependency(self, dep):
        if not dep.get("path"):
            return None
        path = Path(dep["path"]).resolve()
        if not path.is_relative_to(self.root):
            return None
        return self.by_dir.get(path.relative_to(self.root).as_posix())

    def edges(self):
        edges = []
        for name, package in sorted(self.packages.items()):
            for dep in package["dependencies"]:
                target = self.dependency(dep)
                if target:
                    edges.append({"from": name, "to": target, "kind": dep.get("kind") or "normal",
                                  "rename": dep.get("rename"), "optional": dep.get("optional", False),
                                  "target": dep.get("target")})
        return sorted(edges, key=lambda edge: json.dumps(edge, sort_keys=True))

    def production_files(self, name):
        directory = self.root / self.dirs[name]
        return [p for p in self.files[name]
                if not {"tests", "benches", "examples"}.intersection(p.relative_to(directory).parts[:-1])
                and not SKIP_NAME.search(p.name)]

    def line_counts(self):
        counts = {}
        for name in sorted(self.packages):
            files = self.files[name]
            # Physical lines, including a final non-newline-terminated line.
            def count(path):
                with path.open(encoding="utf-8") as source:
                    return sum(1 for _ in source)
            counts[name] = {
                "path": self.dirs[name], "rust_files": len(files),
                "all_rs_lines": sum(count(p) for p in files),
                "src_rs_lines": sum(count(p) for p in files
                                    if p.is_relative_to(self.root / self.dirs[name] / "src")),
            }
        return counts


def blank(text):
    return re.sub(r"[^\n]", " ", text)


def mask_non_code(text):
    """Keep offsets while masking comments and ordinary/raw string/char literals."""
    # Search for lexical boundaries in bulk; stepping through every source byte
    # in Python makes a whole-workspace observation unnecessarily expensive.
    starts = re.compile(r'//|/\*|(?:b|c)?r(?P<hashes>#{0,255})"|"|' + CHAR_LITERAL.pattern)
    comment_boundary = re.compile(r"/\*|\*/")
    out, i = [], 0
    while match := starts.search(text, i):
        start = match.start()
        out.append(text[i:start])
        if match.group() == "//":
            end = text.find("\n", start)
            i = len(text) if end < 0 else end
        elif match.group() == "/*":
            depth, i = 1, match.end()
            while depth:
                boundary = comment_boundary.search(text, i)
                if boundary is None:
                    i = len(text)
                    break
                depth += 1 if boundary.group() == "/*" else -1
                i = boundary.end()
        elif match.group("hashes") is not None:
            end_token = '"' + match.group("hashes")
            end = text.find(end_token, match.end())
            i = len(text) if end < 0 else end + len(end_token)
        elif match.group() == '"':
            i = match.end()
            while i < len(text):
                if text[i] == "\\":
                    i += 2
                elif text[i] == '"':
                    i += 1
                    break
                else:
                    i += 1
        else:
            i = match.end()
        out.append(blank(text[start:i]))
    out.append(text[i:])
    return "".join(out)


def balanced_end(text, start, opening, closing):
    depth = 0
    for i in range(start, len(text)):
        if text[i] == opening:
            depth += 1
        elif text[i] == closing:
            depth -= 1
            if depth == 0:
                return i + 1
    return len(text)


def strip_cfg_test(text):
    """Drop exact cfg(test) items after comments/literals have been masked."""
    pattern = re.compile(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]")
    while match := pattern.search(text):
        end = match.end()
        while True:
            attr = re.match(r"\s*#\s*\[", text[end:])
            if not attr:
                break
            end = balanced_end(text, end + attr.end() - 1, "[", "]")
        item = re.search(r"[;{]", text[end:])
        if item is None:
            end = len(text)
        else:
            end += item.start()
            end = balanced_end(text, end, "{", "}") if text[end] == "{" else end + 1
        text = text[:match.start()] + blank(text[match.start():end]) + text[end:]
    return text


def path_names(text):
    """Read enough of a Rust use tree to retain names and explicit glob markers."""
    token = re.match(r"\s*(r#)?([A-Za-z_][A-Za-z_0-9]*|\*)", text)
    if token:
        name = token.group(2)
        rest = text[token.end():]
        separator = re.match(r"\s*::", rest)
        if separator:
            nested = path_names(rest[separator.end():])
            return [(name,) + tail for tail in nested] or [(name,)]
        return [(name,)]
    group = re.match(r"\s*\{", text)
    if not group:
        return []
    start = group.end() - 1
    end = balanced_end(text, start, "{", "}")
    names, depth, cursor = [], 0, start + 1
    for i in range(start + 1, end):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
        if (text[i] == "," and depth == 0) or i == end - 1:
            names.extend(path_names(text[cursor:i]))
            cursor = i + 1
    return names


def references(text, ident):
    names = set()
    aliases = {ident}
    # A bare import is evidence of a reference even without an item path.
    bare = r"\b(?:use|extern\s+crate)\s+(?:::)?" + re.escape(ident) + r"(?:\s+as\s+(\w+))?\s*;"
    for match in re.finditer(bare, text):
        names.add(("<crate>",))
        if match.group(1):
            aliases.add(match.group(1))
    for alias in aliases:
        prefix = r"(?<![\w:])(?:::)?(?:r#)?" + re.escape(alias) + r"\s*::"
        for match in re.finditer(prefix, text):
            names.update(path_names(text[match.end():]))
    return names


def scan(workspace, scope):
    selected = {name for name, directory in workspace.dirs.items()
                if directory.startswith(SCOPES[scope])}
    usage = collections.defaultdict(lambda: collections.defaultdict(set))
    usage2 = collections.defaultdict(lambda: collections.defaultdict(set))
    scanned = {}
    for name, package in sorted(workspace.packages.items()):
        deps = []
        for dep in package["dependencies"]:
            target = workspace.dependency(dep)
            if target not in selected or dep.get("kind") == "dev":
                continue
            libraries = [t["name"] for t in workspace.packages[target]["targets"]
                         if set(t["kind"]) & {"lib", "rlib", "dylib", "proc-macro"}]
            ident = dep.get("rename") or (libraries[0] if libraries else dep["name"])
            deps.append((target, ident.replace("-", "_")))
        files = workspace.production_files(name)
        scanned[name] = len(files)
        if not deps:
            continue
        for path in files:
            text = strip_cfg_test(mask_non_code(path.read_text(encoding="utf-8")))
            for target, ident in deps:
                for segments in references(text, ident):
                    usage[target][segments[0]].add(name)
                    if len(segments) > 1:
                        usage2[target]["::".join(segments[:2])].add(name)
    unobserved = collections.defaultdict(set)
    for edge in workspace.edges():
        target, name = edge["to"], edge["from"]
        if target in selected and edge["kind"] != "dev" and not any(name in users for users in usage[target].values()):
            unobserved[target].add(name)
    normalize = lambda value: {crate: {item: sorted(users) for item, users in items.items()}
                               for crate, items in value.items()}
    return {
        "scope": scope, "limitations": LIMITATIONS,
        "usage": normalize(usage), "usage2": normalize(usage2),
        "pos": {name: position(path) for name, path in workspace.dirs.items()},
        "scanned_file_counts": scanned,
        "unobserved_dependencies": {name: sorted(users) for name, users in unobserved.items()},
    }


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=1, sort_keys=True) + "\n", encoding="utf-8")
