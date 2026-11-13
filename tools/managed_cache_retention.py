#!/usr/bin/env python3
"""Keep this managed session's shared worker pool without disabling mbx GC."""

import os
from pathlib import Path
import subprocess
import sys


def configure(repo: Path) -> None:
    if os.environ.get("CLAUDE_CODE_REMOTE") != "true" and not os.environ.get(
        "CODEX_ENVIRONMENT_ID"
    ):
        return
    if "MBX_TARGET_KEEP" in os.environ:
        print("managed cache retention: using explicit MBX_TARGET_KEEP override")
        return
    if sys.version_info < (3, 11):
        raise RuntimeError("managed cache retention requires Python 3.11 or newer")
    import tomllib

    # Match warm_build.sh's common-root discovery. A setup script inside a
    # detached worker must protect the shared pool, not a nested pool in itself.
    common = subprocess.check_output(
        ["git", "-C", str(repo), "rev-parse", "--path-format=absolute", "--git-common-dir"],
        text=True,
    ).strip()
    # The pool itself can be a symlink. Keep the physical path mbx may record,
    # while allowing setup before the first worker has created the pool.
    pool = str(
        (Path(common).parent.resolve(strict=True) / ".agents" / "worktree").resolve(
            strict=False
        )
    )
    raw = subprocess.check_output(["mbx", "settings", "get", "target.keep"], text=True).strip()
    keep = tomllib.loads(f"keep = {raw}")["keep"] if raw else []
    if not isinstance(keep, list) or not all(isinstance(item, str) for item in keep):
        raise RuntimeError("mbx target.keep is not a list of paths")
    if pool in keep:
        print(f"managed cache retention: pool already kept: {pool}")
        return
    # mbx's setter accepts a comma-separated list. Refuse an unrepresentable
    # existing entry rather than silently changing another checkout's policy.
    entries = keep + [pool]
    if any("," in item or item != item.strip() for item in entries):
        raise RuntimeError(
            "cannot preserve target.keep through mbx's comma-list setter; "
            "update its TOML list manually"
        )
    subprocess.run(["mbx", "settings", "set", "target.keep", ",".join(entries)], check=True)
    print(f"managed cache retention: keeping shared pool: {pool}")


if __name__ == "__main__":
    try:
        configure(Path(sys.argv[1]))
    except (OSError, RuntimeError, subprocess.CalledProcessError, ValueError) as error:
        print(f"managed cache retention: {error}", file=sys.stderr)
        sys.exit(1)
