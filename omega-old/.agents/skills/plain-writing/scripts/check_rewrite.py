#!/usr/bin/env python3
"""Check that a plain-language rewrite kept every identifier and fact marker.

Compares each file with its version at --base and reports what the rewrite
lost: single-line code spans, link targets, bold task IDs and double-quoted
strings. It also reports the word count before and after and, for source
files, any changed line that is not a comment. Exit status is 1 when a file
grew or a source file changed outside its comments.
"""

import argparse
import re
import subprocess
import sys

COMMENT_PREFIXES = {".rs": ("//",), ".omg": ("//",)}


def at_base(base, path):
    result = subprocess.run(
        ["git", "show", f"{base}:{path}"], capture_output=True, text=True
    )
    return result.stdout if result.returncode == 0 else None


def markers(text):
    found = {"code": set(), "link": set(), "task id": set(), "quoted": set()}
    in_fence = False
    for line in text.splitlines():
        if line.lstrip().startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            found["code"].add(line.strip())
            continue
        found["code"].update(re.findall(r"`([^`]+)`", line))
        found["link"].update(re.findall(r"\]\(([^)\s]+)\)", line))
        found["task id"].update(re.findall(r"\*\*([A-Z][A-Z0-9]+(?:-[A-Z0-9]+)+)", line))
        found["quoted"].update(q for q in re.findall(r'"([^"\n]{3,})"', line))
    found["code"].discard("")
    return found


def code_lines(text, prefixes):
    """Non-comment lines, so a comment-only rewrite leaves them identical."""
    kept = []
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith(prefixes):
            continue
        kept.append(line)
    return kept


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True, help="commit the rewrite started from")
    parser.add_argument("files", nargs="+")
    args = parser.parse_args()

    failed = False
    for path in args.files:
        old = at_base(args.base, path)
        if old is None:
            print(f"{path}: new file, nothing to compare")
            continue
        with open(path, encoding="utf-8") as handle:
            new = handle.read()

        suffix = "." + path.rsplit(".", 1)[-1] if "." in path else ""
        prefixes = COMMENT_PREFIXES.get(suffix)
        if prefixes:
            if code_lines(old, prefixes) != code_lines(new, prefixes):
                print(f"{path}: CODE CHANGED outside comments")
                failed = True
            old_words = sum(len(l.split()) for l in old.splitlines() if l.strip().startswith(prefixes))
            new_words = sum(len(l.split()) for l in new.splitlines() if l.strip().startswith(prefixes))
        else:
            old_words, new_words = len(old.split()), len(new.split())
            before, after = markers(old), markers(new)
            for kind in before:
                lost = sorted(before[kind] - after[kind])
                if lost:
                    print(f"{path}: lost {kind}: {lost}")

        grew = new_words > old_words
        failed |= grew
        print(f"{path}: words {old_words} -> {new_words}{'  GREW' if grew else ''}")

    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
