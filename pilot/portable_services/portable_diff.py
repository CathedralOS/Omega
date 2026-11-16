#!/usr/bin/env python3
"""Cross-family diff of the bundled std portable services.

For every member of a portable boundary trait (FilesystemService, TimeHost,
Clock, Console), find each target family's implementation, follow the
top-level machines it calls inside that family's files, and extract the
behaviour-shaping facts the adapter applies on its own:

  * refusals  - an ErrorKind::X the adapter returns itself (not an errno /
                Win32 translation table),
  * limits    - integer literals compared in a `transition` guard or a
                `requires` clause (`< 255`, `<= buffer.len`, ...),
  * constants - integer literals passed as call arguments (modes, budgets),
  * clamps    - min_u64 / saturating / wrapping narrowing calls.

It then prints, per member, the facts that appear in one family but not in
the other. A difference is a lead, not a verdict: each must be read by hand.

Usage: portable_diff.py <path to source/library/std>
"""
import re
import sys
from pathlib import Path

DECL = re.compile(r"^(?:public\s+)?machine\s+([A-Za-z_][A-Za-z0-9_]*)\s*[(<]")
PROVIDER = re.compile(r"^(?:public\s+)?([A-Za-z_][A-Za-z0-9_]*)\s+satisfies\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{")
MEMBER = re.compile(r"^\s+machine\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(")
CALL = re.compile(r"\b([a-z_][a-z0-9_]*)\s*(?:<[^>()]*>)?\s*\(")
NUM = r"-?\d+"
OTHER = re.compile(r"^(?:public\s+)?(?:data|use|boundary|domain|trait|enum|const|type)\b|^[A-Z]\w*\s+satisfies\b")
SATISFIES = re.compile(r"^(?:public\s+)?machine\s+([A-Za-z_]\w*)[^\n]*\n?\s*satisfies\s+([A-Z]\w*::\w+)", re.M)
QUALIFIED = re.compile(r"\b([A-Z]\w*::[a-z_]\w*)\s*(?:<[^>()]*>)?\s*\(")


def split_toplevel(text):
    """Yield (kind, name, body) for column-0 machines and provider blocks."""
    lines = text.splitlines()
    starts = []
    for i, line in enumerate(lines):
        m = DECL.match(line)
        if m:
            starts.append((i, "machine", m.group(1)))
            continue
        m = PROVIDER.match(line)
        if m:
            starts.append((i, "provider", (m.group(1), m.group(2))))
            continue
        if OTHER.match(line):
            starts.append((i, "other", None))
    for k, (i, kind, name) in enumerate(starts):
        end = starts[k + 1][0] if k + 1 < len(starts) else len(lines)
        yield kind, name, "\n".join(lines[i:end])


def strip_comments(body):
    return re.sub(r"//[^\n]*", "", body)


def provider_members(body):
    """Split a provider block into member name -> body."""
    out, cur, buf = {}, None, []
    for line in body.splitlines()[1:]:
        m = MEMBER.match(line)
        if m and line.startswith("    machine"):
            if cur:
                out[cur] = "\n".join(buf)
            cur, buf = m.group(1), [line]
        elif cur:
            buf.append(line)
    if cur:
        out[cur] = "\n".join(buf)
    return out


TRANSLATION = re.compile(r"(error|errno|kind_of|_kind\b|reconstruct)", re.I)


def facts(body):
    body = strip_comments(body)
    f = set()
    for m in re.finditer(r"ErrorKind::([A-Za-z]+)\s*\}", body):
        f.add(f"refuse {m.group(1)}")
    for m in re.finditer(r"(?:transition|requires|ensures)\s+([^{\n]*)", body):
        cond = m.group(1)
        for c in re.finditer(r"(<=|>=|<|>|==|!=)\s*(" + NUM + r"|[a-z_]+\.len)\b", cond):
            rhs = c.group(2)
            if rhs in ("0", "1", "-1"):
                continue
            f.add(f"limit {c.group(1)} {rhs}")
    for m in re.finditer(r"\b([a-z_][a-z0-9_]*)\s*\(([^()]*)\)", body):
        name, args = m.group(1), m.group(2)
        if name in ("transition", "match"):
            continue
        for a in re.split(r",", args):
            a = a.strip()
            if re.fullmatch(r"\d{3,}", a):
                f.add(f"const {name}(..{a}..)")
        if re.search(r"min_u64|saturating|wrapping", name):
            f.add(f"clamp {name}")
    return f


def load(root, files):
    machines, providers = {}, {}
    for path in files:
        for kind, name, body in split_toplevel(path.read_text()):
            if kind == "machine":
                machines[name] = (path, body)
                sm = SATISFIES.match(body)
                if sm:
                    machines[sm.group(2)] = (path, body)
            elif kind == "provider":
                providers[name[0]] = (path, body)
    return machines, providers


def closure(start_body, machines, depth=8):
    seen, frontier, bodies = set(), [start_body], [start_body]
    for _ in range(depth):
        nxt = []
        for body in frontier:
            text = strip_comments(body)
            names = [m.group(1) for m in CALL.finditer(text)] + [m.group(1) for m in QUALIFIED.finditer(text)]
            for n in names:
                if n in machines and n not in seen and not TRANSLATION.search(n):
                    seen.add(n)
                    nxt.append(machines[n][1])
                    bodies.append(machines[n][1])
        frontier = nxt
    return bodies, seen


def family_facts(member_body, machines):
    bodies, seen = closure(member_body, machines)
    out = set()
    for b in bodies:
        out |= facts(b)
    return out, seen


def families(std):
    t = std / "targets"
    common = [std / f for f in ("filesystem.omg", "time.omg", "time_host.omg", "console.omg", "clock.omg")]
    posix = [std / "posix_filesystem_adapter.omg", std / "posix_filesystem_host.omg"]
    return {
        "linux_x86_64": common + posix + sorted((t / "linux_x86_64").rglob("*.omg")),
        "linux_arm64": common + posix + sorted((t / "linux_arm64").rglob("*.omg")),
        "macos_arm64": common + posix + sorted((t / "macos_arm64").rglob("*.omg")),
        "windows_x86_64": common + [std / "windows_filesystem_host.omg"] + sorted((t / "windows_x86_64").rglob("*.omg")),
    }


SERVICES = {
    # trait -> provider name per target (from std/build.omg selection rows)
    "FilesystemService": {"linux_x86_64": "FilesystemPosixProvider", "linux_arm64": "FilesystemPosixProvider",
                          "macos_arm64": "FilesystemPosixProvider", "windows_x86_64": "FilesystemWindowsProvider"},
    "TimeHost": {"linux_x86_64": "LinuxX8664TimeHost", "linux_arm64": "LinuxArm64TimeHost",
                 "macos_arm64": "DarwinTimeHost", "windows_x86_64": "WindowsTimeHost"},
    "Clock": {"linux_x86_64": "LinuxX8664Clock", "linux_arm64": "LinuxArm64Clock",
              "macos_arm64": "MacosArm64Clock", "windows_x86_64": None},  # bound straight to kernel32 Sleep
    "Console": {"linux_x86_64": "LinuxX86Console", "linux_arm64": "LinuxArm64Console",
                "macos_arm64": "DarwinArm64Console", "windows_x86_64": "WindowsConsole"},
}


def main():
    std = Path(sys.argv[1])
    fams = families(std)
    loaded = {t: load(std, files) for t, files in fams.items()}
    total = 0
    for trait, per_target in SERVICES.items():
        table = {}
        for target, prov in per_target.items():
            if prov is None:
                table[target] = {"*": ({"binding kernel32!Sleep (no adapter)"}, set())}
                continue
            machines, providers = loaded[target]
            _, body = providers[prov]
            table[target] = {m: family_facts(b, machines) for m, b in provider_members(body).items()}
        members = sorted({m for t in table.values() for m in t if m != "*"})
        print(f"\n=== {trait} ===")
        for member in members:
            got = {t: (table[t].get(member) or table[t].get("*") or (set(), set()))[0] for t in table}
            union = set().union(*got.values())
            if all(got[t] == union for t in got):
                continue
            rows = []
            for fact in sorted(union):
                have = [t for t in got if fact in got[t]]
                if len(have) != len(got):
                    rows.append(f"    {fact:<46} only on: {', '.join(have)}")
            if rows:
                total += 1
                print(f"  {member}")
                print("\n".join(rows))
    print(f"\n{total} portable members whose adapter facts differ across targets")


if __name__ == "__main__":
    main()
