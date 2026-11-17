"""List foundation dependency declarations: users.py metadata.json."""
import collections
import json
import sys
from scanlib import SCOPES, Workspace, position


def render(workspace):
    users, devusers, dependencies = (collections.defaultdict(set) for _ in range(3))
    for edge in workspace.edges():
        source, target = edge["from"], edge["to"]
        (devusers if edge["kind"] == "dev" else users)[target].add(source)
        if edge["kind"] != "dev":
            dependencies[source].add(target)
    lines = ["# Declared workspace dependencies; production = normal + build.",
             "# Optional and target-conditional declarations included; these are not observed calls.", ""]
    for name, directory in sorted(workspace.dirs.items()):
        if not directory.startswith(SCOPES["foundation"]):
            continue
        positions = sorted({position(workspace.dirs[user]) for user in users[name]})
        lines.extend([
            f"## {name}  ({directory})  users={len(users[name])} positions={len(positions)}",
            "   positions: " + ", ".join(positions),
            "   users: " + ", ".join(sorted(users[name])),
            "   dev-only users: " + ", ".join(sorted(devusers[name] - users[name])),
            "   deps: " + ", ".join(sorted(dependencies[name])),
        ])
    return "\n".join(line.rstrip() for line in lines) + "\n"


if __name__ == "__main__":
    with open(sys.argv[1], encoding="utf-8") as metadata:
        print(render(Workspace(json.load(metadata))), end="")
