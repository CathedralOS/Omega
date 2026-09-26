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

  - a repo-relative path, `omega-rust/omega/src/build_evaluation.rs`;
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


def completed_items(text):
    """Items that record finished work and name none that is left.

    AGENTS.md: the boards are execution boards, not changelogs -- "an item
    exists only while it names unfinished work and is deleted when acceptance
    passes". An entry whose whole body is a `Landed:` record has outlived
    that, and Git already preserves what it says. An entry that records what
    landed AND still names remaining work is doing its job, so the presence
    of any such word keeps it.
    """
    finished = []
    for item in re.split(r"\n(?=- \*\*[A-Z])", text):
        match = re.match(r"- \*\*([A-Z0-9-]+)\.\*\*\s*(.*)", item, re.S)
        if not match:
            continue
        name, body = match.group(1), match.group(2)
        if not body.lstrip().lower().startswith("landed:"):
            continue
        if re.search(r"\b(remaining|acceptance|owed|still needs|todo)\b", body, re.I):
            continue
        finished.append(name)
    return finished


def cited_links(text):
    """Every repo-relative markdown link target in one board.

    The boards link to specs, guides and crate documents as ordinary
    markdown. Those rot the same way code anchors do, and a reader following
    one gets nothing. External URLs and bare anchors are not ours to check.
    """
    found = set()
    for match in re.finditer(r"\]\(([^)#\s]+)(?:#[^)]*)?\)", text):
        target = match.group(1)
        if target.startswith(("http://", "https://", "mailto:")):
            continue
        found.add(target)
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


def strip_stage_numbers(path):
    """`02_abstract-operations/src/x.rs` -> `abstract-operations/src/x.rs`.

    Pipeline stage directories carry a numeric ordering prefix that has been
    added and renumbered over time, so a citation written before a renumber
    names a live file under a stale spelling.
    """
    return "/".join(
        re.sub(r"^\d+_", "", segment) for segment in path.split("/")
    )


def resolves_ignoring_stage_numbers(candidate, files):
    """The real file a citation names once stage prefixes are ignored.

    A citation that only misses a stage number is NOT dead: the anchor is
    alive and only its spelling is stale, which is a substitution rather than
    the restating a dead anchor calls for. Reporting the two together sends a
    reader hunting for a file that is right there.
    """
    wanted = strip_stage_numbers(candidate.lstrip("./"))
    for path in files:
        stripped = strip_stage_numbers(path)
        if stripped == wanted or stripped.endswith("/" + wanted):
            return path
    return None


def probable_successor(candidate, files):
    """The one file a dead citation most likely became, or `None`.

    Files move wholesale when crates are split or merged, and the citation
    keeps naming the old place. Matching the longest tail that still picks
    out exactly one real file recovers the move without guessing: the full
    path is tried first and segments are dropped from the front until one
    file matches. A tail that matches several files is ambiguous, and every
    shorter tail matches at least as many, so the search stops there rather
    than offering a coin flip.

    This is a suggestion, not a resolution. The citation is still dead and
    still counted as dead; only a reader can say whether the claim around it
    survived the move.
    """
    segments = candidate.lstrip("./").split("/")
    for start in range(len(segments) - 1):
        tail = "/".join(segments[start:])
        matches = [
            path for path in files if path == tail or path.endswith("/" + tail)
        ]
        if len(matches) == 1:
            return matches[0]
        if len(matches) > 1:
            return None
    return None


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


def apply_moves(text, files):
    """Rewrite each citation whose file merely moved, and report the pairs.

    Only a citation with exactly one probable successor is touched, and only
    when it does not already resolve. A move leaves the item's claim intact
    -- the file is right there under another path -- which is the one case
    the boards' "restate rather than substitute" guidance does not cover.
    Anything genuinely gone is left for a reader.
    """
    applied = []
    for citation in sorted(cited_paths(text), key=len, reverse=True):
        alternatives = expand_braces(citation)
        if len(alternatives) != 1 or "*" in citation:
            continue
        if resolves(citation, files):
            continue
        successor = probable_successor(citation, files)
        if not successor or successor == citation.lstrip("./"):
            continue
        marked = "`" + citation + "`"
        if marked not in text:
            continue
        text = text.replace(marked, "`" + successor + "`")
        applied.append((citation, successor))
    return text, applied


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--board", action="append", dest="boards",
        help="check only this board; repeatable (default: every board)",
    )
    parser.add_argument(
        "--hints", action="store_true",
        help="for each dead path, name the file it probably moved to, or "
        "failing that real files sharing its basename",
    )
    parser.add_argument(
        "--quiet", action="store_true", help="print the counts only",
    )
    parser.add_argument(
        "--apply-moves", action="store_true",
        help="rewrite citations whose file merely moved, to its repo-relative "
             "path; leaves every other dead citation alone",
    )
    options = parser.parse_args()

    root = repository_root()
    files = source_files(root)
    by_basename = {}
    for path in files:
        by_basename.setdefault(os.path.basename(path), []).append(path)

    dead_total = 0
    stale_total = 0
    broken_total = 0
    finished_total = 0
    cited_total = 0
    skipped_total = 0
    for board in options.boards or BOARDS:
        board_path = os.path.join(root, board)
        if not os.path.isfile(board_path):
            print(f"board_paths: no such board {board}", file=sys.stderr)
            return 2
        with open(board_path, encoding="utf-8") as handle:
            board_text = handle.read()
        if options.apply_moves:
            board_text, applied = apply_moves(board_text, files)
            if applied:
                with open(board_path, "w", encoding="utf-8") as handle:
                    handle.write(board_text)
                print(f"{board}: rewrote {len(applied)} moved citation(s)")
                for before, after in applied:
                    print(f"  {before}\n      -> {after}")
        citations = cited_paths(board_text)
        cited_total += len(citations)
        dead = []
        stale = []
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
            if not missing:
                continue
            renamed = {
                alternative: resolves_ignoring_stage_numbers(alternative, files)
                for alternative in missing
            }
            if all(renamed.values()):
                stale.append((citation, renamed))
            else:
                dead.append((citation, missing))
        dead_total += len(dead)
        stale_total += len(stale)
        if options.quiet:
            continue
        print(
            f"{board}: {len(citations)} cited, {len(dead)} dead, "
            f"{len(stale)} stale stage prefix"
        )
        for citation, renamed in stale:
            print(f"  {citation}")
            for actual in renamed.values():
                print(f"      stale prefix, now: {actual}")
        for citation, missing in dead:
            print(f"  {citation}")
            if missing != [citation]:
                for alternative in missing:
                    print(f"      missing: {alternative}")
            if options.hints:
                for alternative in missing:
                    successor = probable_successor(alternative, files)
                    if successor:
                        print(f"      probably moved to: {successor}")
                        continue
                    for hint in by_basename.get(
                        os.path.basename(alternative), []
                    )[:3]:
                        print(f"      same name: {hint}")

        finished = completed_items(board_text)
        finished_total += len(finished)
        if finished and not options.quiet:
            print(f"{board}: {len(finished)} completed item(s) still listed")
            for name in finished:
                print(f"  {name}")

        broken = sorted(
            target
            for target in cited_links(board_text)
            if not os.path.exists(os.path.join(root, target))
        )
        broken_total += len(broken)
        if broken and not options.quiet:
            print(f"{board}: {len(broken)} broken link(s)")
            for target in broken:
                print(f"  {target}")

    print(
        f"board_paths: {cited_total} cited, {dead_total} dead, "
        f"{stale_total} stale stage prefix, "
        f"{broken_total} broken link(s), "
        f"{finished_total} completed item(s), "
        f"{skipped_total} glob citation(s) skipped"
    )
    return 1 if dead_total or stale_total or broken_total or finished_total else 0


if __name__ == "__main__":
    raise SystemExit(main())
