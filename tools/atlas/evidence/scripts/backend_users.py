"""List backend line counts and dependency users; Cargo metadata comes on stdin."""
import collections
import json
import sys
from scanlib import SCOPES, Workspace, position


def render(workspace, counts=None):
    counts = counts if counts is not None else workspace.line_counts()
    users = collections.defaultdict(set)
    for edge in workspace.edges():
        if edge["kind"] != "dev":
            users[edge["to"]].add(position(workspace.dirs[edge["from"]]))
    lines = ["# Git-tracked Rust physical lines; all includes tests, benches, examples and build.rs.",
             "# src-only is a separate comparison column. Nested package files are counted once.",
             "# Users are normal + build declarations, including optional and target-conditional edges.",
             f"{'crate':45s} {'all .rs':>8s} {'src .rs':>8s}  declared user positions"]
    total, src_total = 0, 0
    for name, directory in sorted(workspace.dirs.items(), key=lambda pair: pair[1]):
        if not directory.startswith(SCOPES["backend"]):
            continue
        row = counts[name]
        total += row["all_rs_lines"]
        src_total += row["src_rs_lines"]
        lines.append(f"{name:45s} {row['all_rs_lines']:8d} {row['src_rs_lines']:8d}  {', '.join(sorted(users[name]))}")
    lines.append(f"{'total':45s} {total:8d} {src_total:8d}")
    return "\n".join(line.rstrip() for line in lines) + "\n"


if __name__ == "__main__":
    print(render(Workspace(json.load(sys.stdin))), end="")
