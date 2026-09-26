#!/usr/bin/env python3
"""Board code-path checker — which `.rs` anchors the boards cite no longer exist.

`TASKS.md` and its siblings anchor items to code by citing paths in backticks.
Those anchors rot: a crate is split, a stage directory is renumbered, a test
target is deleted, and the item still points at the old file. An item planned
against a gone anchor wastes the next reader's time before they discover the
claim around it needs restating.

This reports the dead ones. It does not repair them, on purpose: per the
boards' own guidance, a gone anchor usually means the claim needs restating
rather than a path substitution, and that judgement belongs to whoever picks
the item up.

Usage:

    python3 tools/board_paths.py                    # every board
    python3 tools/board_paths.py --board TASKS.md   # just one
    python3 tools/board_paths.py --hints            # suggest same-basename files
    python3 tools/board_paths.py --quiet            # count only

Exits 1 when any cited path is dead, so it can gate a board sweep.

Citation forms it understands, because the boards use all three:

  - a repo-relative path, `omega-rust/omega/build/build-evaluation/src/lib.rs`;
  - a path relative to some enclosing directory, resolved as a suffix of a
    real file, `checks/ranges/incoming_guards.rs`;
  - brace alternatives, `admission/{declarations,behavior_exclusions}.rs`,
    which count as dead when ANY alternative is missing.

A citation broken across lines by paragraph wrapping is rejoined before
matching, and one carrying a `*` glob is skipped rather than guessed at.
"""

import argparse
import os
import re
import sys

BOARDS = ("TASKS.md", "TASKS_BOOTSTRAP.md", "TASKS_OPTIMIZER.md")
SOURCE_ROOTS = ("omega-rust", "tools", "bootstrap", "tests", "source")


def repository_root():
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def cited_paths(text):
    """Every backticked `.rs` path in one board, wrapped or not.

    The boards wrap long paths across lines, so the same text is scanned
    twice: once as written, and once with newline+indent collapsed. A path
    that only survives the second pass was wrapped.
    """
    collapsed = re.sub(r"\n\s*", "", text)
    found = set()
    for source in (text, collapsed):
        for match in re.finditer(r"`([^`\s]+\.rs)`", source):
            found.add(match.group(1))
    return found


def expand_braces(path):
    """`a/{b,c}.rs` -> [`a/b.rs`, `a/c.rs`]; nested braces expand too."""
    match = re.search(r"\{([^{}]*)\}", path)
    if not match:
        return [path]
    expanded = []
    for alternative in match.group(1).split(","):
        expanded += expand_braces(
            path[: match.start()] + alternative.strip() + path[match.end() :]
        )
    return expanded


def source_files(root):
    """Every `.rs` file under the source roots, repo-relative."""
    files = []
    for top in SOURCE_ROOTS:
        base = os.path.join(root, top)
        if not os.path.isdir(base):
            continue
        for directory, children, names in os.walk(base):
            children[:] = [
                child for child in children if child not in ("target", ".git")
            ]
            for name in names:
                if name.endswith(".rs"):
                    files.append(
                        os.path.relpath(os.path.join(directory, name), root)
                    )
    return files


def resolves(candidate, files):
    """A citation resolves when a real file IS it or ENDS WITH it.

    Suffix matching is what admits the boards' directory-relative form. It
    can match more than one file; the citation still names a real anchor, so
    that is a resolution, not an ambiguity to report.
    """
    wanted = candidate.lstrip("./")
    return any(
        path == wanted or path.endswith("/" + wanted) for path in files
    )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--board", action="append", dest="boards",
        help="check only this board; repeatable (default: every board)",
    )
    parser.add_argument(
        "--hints", action="store_true",
        help="for each dead path, list real files sharing its basename",
    )
    parser.add_argument(
        "--quiet", action="store_true", help="print the counts only",
    )
    options = parser.parse_args()

    root = repository_root()
    files = source_files(root)
    by_basename = {}
    for path in files:
        by_basename.setdefault(os.path.basename(path), []).append(path)

    dead_total = 0
    cited_total = 0
    skipped_total = 0
    for board in options.boards or BOARDS:
        board_path = os.path.join(root, board)
        if not os.path.isfile(board_path):
            print(f"board_paths: no such board {board}", file=sys.stderr)
            return 2
        with open(board_path, encoding="utf-8") as handle:
            citations = cited_paths(handle.read())
        cited_total += len(citations)
        dead = []
        for citation in sorted(citations):
            alternatives = expand_braces(citation)
            if any("*" in alternative for alternative in alternatives):
                skipped_total += 1
                continue
            missing = [
                alternative
                for alternative in alternatives
                if not resolves(alternative, files)
            ]
            if missing:
                dead.append((citation, missing))
        dead_total += len(dead)
        if options.quiet:
            continue
        print(f"{board}: {len(citations)} cited, {len(dead)} dead")
        for citation, missing in dead:
            print(f"  {citation}")
            if missing != [citation]:
                for alternative in missing:
                    print(f"      missing: {alternative}")
            if options.hints:
                for alternative in missing:
                    for hint in by_basename.get(
                        os.path.basename(alternative), []
                    )[:3]:
                        print(f"      same name: {hint}")

    print(
        f"board_paths: {cited_total} cited, {dead_total} dead, "
        f"{skipped_total} glob citation(s) skipped"
    )
    return 1 if dead_total else 0


if __name__ == "__main__":
    raise SystemExit(main())
