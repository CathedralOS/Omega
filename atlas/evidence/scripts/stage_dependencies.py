"""List non-stage crates declaring stage dependencies; metadata comes on stdin.

A Cargo dependency edge is not evidence that a function is called at runtime.
"""
import json
import sys
from scanlib import Workspace


def render(workspace):
    stages = {name for name, path in workspace.dirs.items() if "/pipeline/" in path}
    lines = ["# Normal + build declarations from non-stage crates to numbered stages.",
             "# Optional and target-conditional edges included. This is not a call graph."]
    for edge in workspace.edges():
        if edge["from"] in stages or edge["to"] not in stages or edge["kind"] == "dev":
            continue
        flags = [edge["kind"]]
        if edge["optional"]:
            flags.append("optional")
        if edge["target"]:
            flags.append(edge["target"])
        lines.append(f"{workspace.dirs[edge['from']]} -> {workspace.dirs[edge['to']]} [{'; '.join(flags)}]")
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    print(render(Workspace(json.load(sys.stdin))), end="")
