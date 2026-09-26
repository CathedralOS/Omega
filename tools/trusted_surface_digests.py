#!/usr/bin/env python3
"""Re-record trusted-surface implementation-site digests.

The trusted-surface ledger
(`omega-rust/psi/semantics/terminal-verifier/src/trusted_surface/sites.rs`)
pins each bound implementation file's SHA-256; the suite's
`recorded_digests_match_the_working_tree` fails when a bound file changes.

This tool makes the re-record step mechanical. It never decides whether a
change is sound — before running `--write`, revalidate the justification of
every citing entry listed by the coverage failure (the test prints them as
`dependents`), exactly as the ledger's header contract requires.

    python3 tools/trusted_surface_digests.py            # report drift
    python3 tools/trusted_surface_digests.py --write    # re-record digests

Report mode prints each mismatch as `path`, recorded digest, actual digest,
exits 1 while any disagree, and leaves `sites.rs` untouched. `--write`
rewrites only `sha256:` string literals inside `ImplementationSite` rows —
never site paths, never test-only sources, never inventory machinery (which
binds by path alone). Missing bound files are reported and never written.
"""

import argparse
import hashlib
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
SITES_RS = REPO_ROOT.joinpath(
    "omega-rust/psi/semantics/terminal-verifier/src/trusted_surface/sites.rs"
)

SITE_BLOCK = re.compile(
    r"ImplementationSite\s*\{[^}]*?path:\s*\"(?P<path>[^\"]+)\"[^}]*?"
    r"sha256:\s*(?P<digest>None|Some\(\"(?P<hex>[0-9a-f]{64})\"\))[^}]*?\}",
    re.DOTALL,
)
DIGEST_SPAN = re.compile(r"sha256:\s*Some\(\"([0-9a-f]{64})\"\)")


def collect_sites(text):
    """Yield (path, recorded digest or None) for every ImplementationSite."""
    for match in SITE_BLOCK.finditer(text):
        yield match.group("path"), match.group("hex")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--write",
        action="store_true",
        help="update sha256 literals in sites.rs to match the working tree",
    )
    args = parser.parse_args()

    text = SITES_RS.read_text(encoding="utf-8")
    stale = []
    missing = []
    for path, recorded in collect_sites(text):
        if recorded is None:
            continue
        target = REPO_ROOT.joinpath(path)
        if not target.is_file():
            missing.append(path)
            continue
        actual = hashlib.sha256(target.read_bytes()).hexdigest()
        if actual != recorded:
            stale.append((path, recorded, actual))

    for path in missing:
        print(f"missing bound file: {path}")
    for path, recorded, actual in stale:
        print(f"stale digest: {path}\n  recorded {recorded}\n  actual   {actual}")

    if stale or missing:
        print(
            f"\n{len(stale)} stale digest(s), {len(missing)} missing bound file(s). "
            "Revalidate every citing entry's justification, then run with --write."
        )
    if args.write and stale and not missing:
        digests = {path: actual for path, _, actual in stale}
        current_path = None
        out = []
        for line in text.splitlines(keepends=True):
            path_match = re.search(r'path:\s*"([^"]+)"', line)
            if path_match:
                current_path = path_match.group(1)
            digest_match = DIGEST_SPAN.search(line)
            if (
                digest_match
                and current_path in digests
                and digest_match.group(1) != digests[current_path]
            ):
                line = line.replace(digest_match.group(1), digests[current_path])
            out.append(line)
        SITES_RS.write_text("".join(out), encoding="utf-8")
        print(f"re-recorded {len(stale)} digest(s) in {SITES_RS}")
    elif args.write and missing:
        print("refusing --write while a bound file is missing")

    if not stale and not missing:
        print("all recorded digests match the working tree")
    return 1 if stale or missing else 0


if __name__ == "__main__":
    sys.exit(main())
