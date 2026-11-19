"""Show candidate defining locations for observed items, without resolving Rust.

Usage: report.py usage.json metadata.json crate [--by-file] [--depth N]
Candidates exclude the same test files/items as the usage scanner. Multiple
matches remain visible; a repeated name must not silently pick a test helper.
"""
import argparse
import collections
import json
from pathlib import Path
import re
from scanlib import Workspace, mask_non_code, strip_cfg_test


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("usage", type=Path)
    parser.add_argument("metadata", type=Path)
    parser.add_argument("crate")
    parser.add_argument("--by-file", action="store_true")
    parser.add_argument("--depth", type=int, default=1)
    args = parser.parse_args()
    usage = json.loads(args.usage.read_text())
    workspace = Workspace(json.loads(args.metadata.read_text()))
    src = workspace.root / workspace.dirs[args.crate] / "src"
    definitions = collections.defaultdict(set)
    definition = re.compile(r"^\s*pub(?:\([^)]*\))?\s+(?:unsafe\s+)?(?:struct|enum|fn|trait|type|const|static|mod|union)\s+([A-Za-z_][A-Za-z0-9_]*)")
    macro = re.compile(r"^\s*[a-z_]+!\s*[\({]\s*(?:pub\s+)?([A-Z][A-Za-z0-9_]*)")
    macro_rule = re.compile(r"macro_rules!\s+([A-Za-z_][A-Za-z0-9_]*)")
    module = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{")
    for path in workspace.production_files(args.crate):
        if not path.is_relative_to(src):
            continue
        stack, depth = [], 0
        code = strip_cfg_test(mask_non_code(path.read_text(encoding="utf-8")))
        for line in code.splitlines():
            location = path.relative_to(src).as_posix() + "".join("::" + name for name, _ in stack[:args.depth])
            for pattern in (definition, macro, macro_rule):
                match = pattern.search(line) if pattern is macro_rule else pattern.match(line)
                if match:
                    definitions[match.group(1)].add(location)
            if match := module.match(line):
                stack.append((match.group(1), depth))
            depth += line.count("{") - line.count("}")
            while stack and depth <= stack[-1][1]:
                stack.pop()
    locations = lambda item: " | ".join(sorted(definitions[item])) or "? unresolved"
    positions = lambda names: {usage["pos"].get(name, name) for name in names}
    items = usage["usage"].get(args.crate, {})
    print("# Definition candidates from source text; reexports/macros and ambiguous names require review.")
    if args.by_file:
        aggregate = collections.defaultdict(lambda: [set(), []])
        for item, names in items.items():
            row = aggregate[locations(item)]
            row[0].update(positions(names))
            row[1].append(item)
        for location, (places, names) in sorted(aggregate.items(), key=lambda pair: (-len(pair[1][0]), pair[0])):
            print(f"{location}: {len(places)} positions | {len(names)} items: {', '.join(sorted(names))}")
            print(f"    positions: {', '.join(sorted(places))}")
    else:
        for item, names in sorted(items.items(), key=lambda pair: (-len(pair[1]), pair[0])):
            places = positions(names)
            print(f"{item} [{locations(item)}] ({len(places)}): {', '.join(sorted(places))}")


if __name__ == "__main__":
    main()
