"""Plan a codebase atlas: assign files to crates, crates to areas, and cut reading units.

Inputs (from git at one pinned commit):
  linecounts.txt   "path:count" for every .rs under source-rust/ and tools/ and every .omg under source/
  crate-names.tsv  "<dir>/Cargo.toml<TAB><crate name>" for every manifest

Outputs (tab-separated, header row first):
  crates.tsv  crate, root, area, files, lines
  areas.tsv   area, crates, files, lines, units, description
  units.tsv   unit, area, path, first_line, last_line, lines

A file belongs to the crate whose manifest directory is its longest path prefix. Omega files under
source/ have no manifest: each folder directly under source/ (and under source/library/) counts as one
package. An area is a stretch of the pipeline that one crate-map reader and one summary writer cover.
Units are cut in path order across the whole tree, as the 2026-11-10 atlas did: at most MAX_LINES lines
each, a longer file split by line range. A unit may hold files from two adjacent areas.
"""

import sys
from collections import defaultdict
from pathlib import PurePosixPath

MAX_LINES = 22_000
S2P = "source-rust/subsystems/compiler/source-to-psi/"
P2N = "source-rust/subsystems/compiler/psi-to-native/"

# (area, prefixes, description); the first area with a matching prefix wins.
AREAS = [
    ("01-cli-foundation", ["source-rust/application/", "source-rust/foundation/", "source-rust/subsystems/compiler/foundation/",
                           "source-rust/subsystems/compiler/compiler", "source-rust/subsystems/compiler/Cargo"],
     "The omega binary and its commands, shared host and compiler foundation crates, the public compiler entry"),
    ("02-source-stages-00-04", [S2P + "pipeline/00_", S2P + "pipeline/01_", S2P + "pipeline/02_", S2P + "pipeline/03_", S2P + "pipeline/04_"],
     "Source stages 00 to 04: tokens, syntax trees, name resolution, typing, preparation"),
    ("03-source-stage-05", [S2P + "pipeline/05_"], "Source stage 05: prepared typed trees to the lowered program"),
    ("04-source-stages-06-07", [S2P + "pipeline/06_", S2P + "pipeline/07_"], "Source stages 06 and 07: the lowered-form checker and Psi publication"),
    ("05-source-representations", [S2P + "representations/"], "The forms the source stages exchange"),
    ("06-source-driver-build", [S2P, "source-rust/subsystems/compiler/build/"], "Source driver and compile-time evaluation; Build planning, execution and settings"),
    ("07-psi-semantics", ["source-rust/subsystems/compiler/psi/semantics/"], "Psi semantics: storage flow, descriptors, replay, continuation checks"),
    ("08-psi-program", ["source-rust/subsystems/compiler/psi/"], "Portable Psi program, artifact encoding, identities, interfaces and the rest of psi/"),
    ("09-native-stages-00-03", [P2N + "pipeline/00_", P2N + "pipeline/01_", P2N + "pipeline/02_", P2N + "pipeline/03_"],
     "Native stages 00 to 03: admitted inputs, target binding, abstract operations and their optimization"),
    ("10-native-stages-04-10", [P2N + "pipeline/04_", P2N + "pipeline/05_", P2N + "pipeline/06_", P2N + "pipeline/07_",
                                P2N + "pipeline/08_", P2N + "pipeline/09_", P2N + "pipeline/10_"],
     "Native stages 04 to 10: target operations, instruction selection, register allocation, framing"),
    ("11-native-stages-11-15", [P2N + "pipeline/"], "Native stages 11 to 15: encoding, layout, placement, objects, images"),
    ("12-native-representations", [P2N + "representations/"], "The forms the native stages exchange"),
    ("13-native-targets-driver", [P2N], "Native targets and ABIs, the native driver and its admission crates"),
    ("14-interpreter", ["source-rust/subsystems/interpreter/"], "The Psi interpreter, its host services and memory"),
    ("15-package-manager", ["source-rust/subsystems/package-manager/", "source-rust/"], "Package acquisition, graphs, review and update"),
    ("16-tools", ["tools/"], "Repository tooling: xtask and benchmark"),
    ("17-omega-source", ["source/"], "The Omega-written compiler and application, and the bundled library packages"),
]


def area_of(path: str) -> str:
    # The `compiler` crate's own files sit directly in its folder, beside the subsystem folders.
    if str(PurePosixPath(path).parent) == "source-rust/subsystems/compiler":
        return "01-cli-foundation"
    for name, prefixes, _ in AREAS:
        if any(path.startswith(p) for p in prefixes):
            return name
    raise SystemExit(f"no area for {path}")


def main(linecounts: str, crate_names: str, out_dir: str) -> None:
    manifests = {}
    for row in open(crate_names, encoding="utf-8"):
        toml, name = row.rstrip("\n").split("\t")
        manifests[str(PurePosixPath(toml).parent)] = name
    roots = sorted(manifests, key=len, reverse=True)

    files = []
    for row in open(linecounts, encoding="utf-8"):
        path, count = row.rstrip("\n").rsplit(":", 1)
        files.append((path, int(count)))
    files.sort()

    def crate_of(path: str) -> tuple[str, str]:
        for root in roots:
            if path.startswith(root + "/"):
                return manifests[root], root
        depth = 3 if path.startswith("source/library/") else 2
        root = "/".join(path.split("/")[:depth])
        return f"omega:{root.split('/')[-1]}", root

    crates = defaultdict(lambda: ["", 0, 0])
    areas = defaultdict(lambda: [set(), 0, 0])
    for path, count in files:
        name, root = crate_of(path)
        area = area_of(path)
        c = crates[(name, root)]
        c[0] = area_of(root + "/lib.rs")  # a crate's area is its root's, whatever subfolder a file sits in
        c[1] += 1
        c[2] += count
        a = areas[area]
        areas[c[0]][0].add((name, root))  # count each crate once, in its root's area
        a[1] += 1
        a[2] += count

    rows = []
    unit, current = 1, 0
    for path, count in files:
        first = 1
        while first <= count:
            rest = count - first + 1
            if current >= MAX_LINES or (current > 0 and current + rest > MAX_LINES and rest <= MAX_LINES):
                unit, current = unit + 1, 0
            take = min(rest, MAX_LINES - current)
            rows.append((unit, area_of(path), path, first, first + take - 1, take))
            current += take
            first += take

    units_per_area = defaultdict(set)
    for row in rows:
        units_per_area[row[1]].add(row[0])
    descriptions = {name: text for name, _, text in AREAS}

    with open(f"{out_dir}/crates.tsv", "w", encoding="utf-8", newline="\n") as out:
        out.write("crate\troot\tarea\tfiles\tlines\n")
        for (name, root), (area, n, lines) in sorted(crates.items(), key=lambda kv: kv[0][1]):
            out.write(f"{name}\t{root}\t{area}\t{n}\t{lines}\n")
    with open(f"{out_dir}/areas.tsv", "w", encoding="utf-8", newline="\n") as out:
        out.write("area\tcrates\tfiles\tlines\tunits\tdescription\n")
        for area in sorted(areas):
            names, n, lines = areas[area]
            out.write(f"{area}\t{len(names)}\t{n}\t{lines}\t{len(units_per_area[area])}\t{descriptions[area]}\n")
    with open(f"{out_dir}/units.tsv", "w", encoding="utf-8", newline="\n") as out:
        out.write("unit\tarea\tpath\tfirst_line\tlast_line\tlines\n")
        for row in rows:
            out.write("\t".join(str(x) for x in row) + "\n")

    sizes = defaultdict(int)
    for row in rows:
        sizes[row[0]] += row[5]
    assert max(sizes.values()) <= MAX_LINES, "a unit is over the limit"
    assert sum(sizes.values()) == sum(c for _, c in files), "lines lost or doubled"
    print(f"files {len(files)}, lines {sum(c for _, c in files)}, crates {len(crates)}, areas {len(areas)}, units {unit}")
    print(f"largest unit {max(sizes.values())} lines, smallest {min(sizes.values())}")
    for area in sorted(areas):
        names, n, lines = areas[area]
        print(f"  {area:28s} crates {len(names):3d} files {n:5d} lines {lines:8d} units {len(units_per_area[area])}")


if __name__ == "__main__":
    main(*sys.argv[1:4])
