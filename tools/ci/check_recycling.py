"""Reject Cargo and Rust file references from live code into a _recycling directory."""

import glob
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib


def dependency_paths(table):
    for name, spec in table.items():
        if isinstance(spec, dict) and "path" in spec:
            yield name, spec["path"]


def cargo_paths(manifest):
    doc = tomllib.loads(manifest.read_text())
    workspace = doc.get("workspace", {})
    excluded = {
        Path(match).resolve()
        for pattern in workspace.get("exclude", [])
        for match in glob.iglob(str(manifest.parent / pattern), recursive=True)
    }
    for pattern in workspace.get("members", []):
        member = manifest.parent / pattern
        if not glob.has_magic(pattern):
            # Cargo keeps explicitly listed members even when exclude matches.
            yield "workspace member", str(member)
            continue
        for match in glob.iglob(str(member), recursive=True):
            if not any(Path(match).resolve().is_relative_to(path) for path in excluded):
                yield "workspace member", match

    for table in [doc, workspace, *doc.get("target", {}).values()]:
        for kind in ("dependencies", "build-dependencies", "dev-dependencies",
                     "build_dependencies", "dev_dependencies"):
            yield from dependency_paths(table.get(kind, {}))
    for table in doc.get("patch", {}).values():
        yield from dependency_paths(table)
    yield from dependency_paths(doc.get("replace", {}))

    build = doc.get("package", {}).get("build")
    if isinstance(build, str):
        yield "build script", build
    targets = [doc.get("lib", {})]
    for kind in ("bin", "test", "bench", "example"):
        targets.extend(doc.get(kind, []))
    for target in targets:
        if "path" in target:
            yield "Cargo target", target["path"]


# Keep strings whole, including raw strings and quoted code in test expectations.
# Rust block comments can nest.
TOKEN = re.compile(r'''//[^\n]*|/\*|(?:br|cr|r)(?P<hash>\#*)".*?"(?P=hash)|"(?:[^"\\]|\\.)*"|
                        '(?:[^'\\\n]|\\(?:u\{[\da-fA-F_]+\}|x[\da-fA-F]{2}|.))'|
                        (?:r\#)?\w+|[^\s]''', re.DOTALL | re.VERBOSE)
COMMENT = re.compile(r"/\*|\*/")
ESCAPE = re.compile(r'\\(?:x(?P<byte>[0-9a-fA-F]{2})|u\{(?P<char>[0-9a-fA-F_]+)\}|'
                    r'(?P<continued>\r?\n\s*)|(?P<escaped>.))', re.DOTALL)


def rust_tokens(source):
    offset = 0
    while match := TOKEN.search(source, offset):
        word = match.group()
        offset = match.end()
        if word == "/*":
            depth = 1
            for marker in COMMENT.finditer(source, offset):
                depth += 1 if marker.group() == "/*" else -1
                if not depth:
                    offset = marker.end()
                    break
            else:
                return
        elif not word.startswith("//"):
            yield word


def string_value(word):
    if word.startswith('"'):
        def unescape(match):
            if digits := match['byte'] or match['char']:
                return chr(int(digits.replace('_', ''), 16))
            if match['continued']:
                return ''
            return {'n': '\n', 'r': '\r', 't': '\t', '0': '\0'}.get(match['escaped'], match['escaped'])
        return ESCAPE.sub(unescape, word[1:-1])
    if match := re.fullmatch(r'r(\#*)"(.*)"\1', word, re.DOTALL):
        return match[2]
    return None


def arguments(tokens, start):
    """Split a Rust token group on top-level commas."""
    depth, first = 0, start + 1
    for i in range(first, len(tokens)):
        word = tokens[i]
        if word in ("(", "[", "{"):
            depth += 1
        elif word in (")", "]", "}"):
            if depth == 0:
                if first < i:
                    yield tokens[first:i]
                return
            depth -= 1
        elif word == "," and not depth:
            yield tokens[first:i]
            first = i + 1


def static_path(expr, manifest_dir):
    if len(expr) == 1:
        return string_value(expr[0])
    if len(expr) >= 4 and expr[1:3] == ["!", "("]:
        args = list(arguments(expr, 2))
        if expr[0] == "env" and len(args) == 1 and args[0] == ['"CARGO_MANIFEST_DIR"']:
            return str(manifest_dir)
        if expr[0] == "concat":
            parts = [static_path(arg, manifest_dir) for arg in args]
            if all(part is not None for part in parts):
                return "".join(parts)
    return None


def attribute_paths(attr):
    if len(attr) == 3 and attr[:2] == ["path", "="]:
        value = string_value(attr[2])
        if value is not None:
            yield value
    elif attr[:2] == ["cfg_attr", "("]:
        for item in list(arguments(attr, 1))[1:]:
            yield from attribute_paths(item)


def rust_paths(file, source, manifests):
    if not re.search(r'\b(?:include(?:_bytes|_str)?|path|mod)\b', source):
        return
    tokens = list(rust_tokens(source))
    manifest_dir = next((p for p in file.parents if p in manifests), file.parent)
    # Out-of-line modules declared in foo.rs normally live under foo/. A
    # nonstandard crate or #[path] root can also own modules beside its file.
    roots = [file.parent]
    if file.name not in {"lib.rs", "main.rs", "mod.rs"}:
        roots.append(file.with_suffix(""))
    directories = [roots]
    inline = {}
    explicit = None
    for i, word in enumerate(tokens):
        if word == "#" and tokens[i + 1:i + 2] == ["["]:
            for attr in arguments(tokens, i + 1):
                for value in attribute_paths(attr):
                    if len(directories) == 1:
                        yield "Rust path attribute", value
                    else:
                        for directory in directories[-1]:
                            yield "Rust path attribute", str(directory / value)
                    if attr[0] == "path":
                        explicit = value
        if word == "mod" and i + 2 < len(tokens):
            name, delimiter = tokens[i + 1:i + 3]
            name = name.removeprefix("r#")
            if delimiter == "{":
                inline[i + 2] = [d / (explicit if explicit is not None else name)
                                 for d in directories[-1]]
            elif delimiter == ";" and explicit is None:
                for directory in directories[-1]:
                    for target in (directory / (name + ".rs"), directory / name / "mod.rs"):
                        if target.is_file():
                            yield "Rust module", str(target)
        if word == "{":
            directories.append(inline.pop(i, directories[-1]))
        elif word == "}" and len(directories) > 1:
            directories.pop()
        if word in {"{", "}", ";"}:
            explicit = None
        if word in ("include", "include_bytes", "include_str") and tokens[i + 1:i + 2] == ["!"]:
            if tokens[i + 2:i + 3] in (["("], ["["], ["{"]):
                for arg in arguments(tokens, i + 2):
                    if (value := static_path(arg, manifest_dir)) is not None:
                        yield word + "!", value


def violations(root, files):
    manifests = {file.parent for file in files if file.name == "Cargo.toml"}
    for file in sorted(files):
        if "_recycling" in file.relative_to(root).parts:
            continue
        if "_recycling" in Path(os.path.relpath(file.resolve(), root)).parts:
            yield f"{file.relative_to(root)} links into _recycling"
            continue
        if file.name == "Cargo.toml":
            edges = cargo_paths(file)
        else:
            edges = rust_paths(file, file.read_text(), manifests)
        for kind, value in edges:
            # Cargo accepts slash-separated paths on all hosts. Recognize
            # Windows-authored paths when admission runs on Linux as well.
            dest = (file.parent / value.replace("\\", "/")).resolve()
            if "_recycling" in Path(os.path.relpath(dest, root)).parts:
                yield f"{file.relative_to(root)}: {kind} points into _recycling: {value}"


def main():
    root = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
    tracked = subprocess.check_output(["git", "-C", root, "ls-files", "-z", "--", "Cargo.toml", "*/Cargo.toml", "*.rs"])
    files = {root / os.fsdecode(name) for name in tracked.split(b"\0") if name}
    failed = False
    for message in violations(root, files):
        print(message, file=sys.stderr)
        failed = True
    return int(failed)


if __name__ == "__main__":
    sys.exit(main())
