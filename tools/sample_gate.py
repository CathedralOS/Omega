#!/usr/bin/env python3
"""Sample check gate — what every `samples/` program does on THIS host.

`omega refresh-samples` builds every sample natively, which is the release
gate and costs a native build each. This is the cheap counterpart: it runs
`omega --check` over each sample in parallel, into a scratch build directory
so the working tree is untouched, and reports the first diagnostic per
sample grouped by cause.

A sample is the product's own customer, so a cluster here names a capability
the language owes rather than a fixture to repair. The clusters are printed
with their sample lists so a lane can pick up its own.

Usage:

    python3 tools/sample_gate.py                       # every sample
    python3 tools/sample_gate.py --filter text,gui/    # matching paths only
    python3 tools/sample_gate.py --write-results out.json

`--cli` skips the build and uses an existing `omega` binary; otherwise the
development CLI is built once with `mbx` when present and `cargo` otherwise.
`samples/apps/squalr` is excluded: it is a separate application checkout with
its own gate.

Exit status is 0 when every selected sample checks and 1 otherwise, so this
runs as an ordinary gate. On a host missing a provider a sample declares,
the refusal is the host's, not the sample's — read the `provider absent on
this host` cluster before treating it as breakage.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
SAMPLES = ROOT / "samples"
EXCLUDED = (SAMPLES / "apps" / "squalr",)

# First matching rule names the cluster. Order is deliberate: a more specific
# refusal is listed before the general shape it would otherwise fall into.
CLUSTERS: list[tuple[str, str]] = [
    (r"rejoins 0 Terminal attachment identities", "unit-plan omission"),
    (r"implicitly converts", "implicit store conversion"),
    (r"implicit domain weakening", "implicit domain weakening"),
    (r"requires a selected Fused provider", "provider absent on this host"),
    (r"whose selected provider has no requirement-level Terminal route",
     "boundary provider route unimplemented"),
    (r"value-call argument cannot itself be a machine call",
     "nested value call"),
    (r"NON-TAIL self-recursion", "non-tail self recursion"),
    (r"cannot prove default-domain field requirement",
     "default-domain field requirement"),
]

NOISE = re.compile(r"linker stderr|warn\(linker|^\s*\|\s*$|^\s*=\s*note:|^$")


def sample_roots(filters: list[str]) -> list[Path]:
    roots = []
    for build in sorted(SAMPLES.rglob("build.omg")):
        directory = build.parent
        if any(excluded in directory.parents or excluded == directory
               for excluded in EXCLUDED):
            continue
        if not (directory / "main.omg").is_file():
            continue
        relative = directory.relative_to(ROOT).as_posix()
        if filters and not any(fragment in relative for fragment in filters):
            continue
        roots.append(directory)
    return roots


def build_cli() -> Path:
    tool = "mbx" if shutil.which("mbx") else "cargo"
    args = [tool, "build", "--message-format=json", "-p", "omega"]
    proc = subprocess.run(args, cwd=ROOT, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.stderr.write(proc.stdout[-4000:])
        sys.stderr.write(proc.stderr[-4000:])
        raise SystemExit(f"sample_gate: CLI build failed ({proc.returncode})")
    executable = None
    for line in proc.stdout.splitlines():
        try:
            message = json.loads(line)
        except ValueError:
            continue
        if (message.get("reason") == "compiler-artifact"
                and message.get("target", {}).get("name") == "omega"
                and message.get("executable")):
            executable = message["executable"]
    if executable is None:
        raise SystemExit("sample_gate: build produced no omega binary")
    return Path(executable)


def first_error(output: str) -> str:
    """The sample's own first diagnostic, with the linker noise dropped."""
    lines = [line.rstrip() for line in output.splitlines()
             if not NOISE.search(line)]
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("error:"):
            return stripped[len("error:"):].strip()
    for line in lines:
        if line.strip():
            return line.strip()
    return "(no output)"


def cluster_of(message: str) -> str:
    for pattern, name in CLUSTERS:
        if re.search(pattern, message):
            return name
    return "other"


def check_one(cli: Path, scratch: Path, directory: Path) -> dict:
    relative = directory.relative_to(ROOT).as_posix()
    build_dir = scratch / relative.replace("/", "__")
    proc = subprocess.run(
        [str(cli), "--check", "--build-dir", str(build_dir),
         str(directory / "main.omg")],
        cwd=ROOT, capture_output=True, text=True)
    output = proc.stdout + proc.stderr
    checked = proc.returncode == 0
    record = {"sample": relative, "checked": checked}
    if not checked:
        message = first_error(output)
        record["diagnostic"] = message
        record["cluster"] = cluster_of(message)
    return record


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--filter",
                        help="comma-separated path fragments — check only "
                             "samples whose repo-relative directory matches")
    parser.add_argument("--jobs", type=int, default=0,
                        help="parallel checks (default: the machine's "
                             "parallelism, capped at the sample count)")
    parser.add_argument("--cli", type=Path,
                        help="use this omega binary instead of building one")
    parser.add_argument("--write-results", type=Path, metavar="PATH",
                        help="write every sample's record to PATH as JSON")
    options = parser.parse_args()

    filters = [fragment.strip()
               for fragment in (options.filter or "").split(",")
               if fragment.strip()]
    roots = sample_roots(filters)
    if not roots:
        print("sample_gate: no samples selected")
        return 0
    cli = options.cli or build_cli()
    if not os.access(cli, os.X_OK):
        raise SystemExit(f"sample_gate: missing CLI {cli}")

    jobs = options.jobs or min(len(roots), os.cpu_count() or 4)
    records = []
    with tempfile.TemporaryDirectory(prefix="omega-sample-gate-") as scratch:
        scratch_path = Path(scratch)
        with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as pool:
            futures = [pool.submit(check_one, cli, scratch_path, directory)
                       for directory in roots]
            for done, future in enumerate(
                    concurrent.futures.as_completed(futures), start=1):
                records.append(future.result())
                if done % 25 == 0 or done == len(roots):
                    print(f"sample_gate: {done}/{len(roots)} checked",
                          flush=True)
    records.sort(key=lambda record: record["sample"])

    if options.write_results:
        options.write_results.parent.mkdir(parents=True, exist_ok=True)
        options.write_results.write_text(
            json.dumps(records, indent=2) + "\n", encoding="utf-8")
        print(f"sample_gate: wrote {len(records)} record(s) to "
              f"{options.write_results}")

    failures = [record for record in records if not record["checked"]]
    clusters: dict[str, list[dict]] = {}
    for record in failures:
        clusters.setdefault(record["cluster"], []).append(record)
    for name, members in sorted(clusters.items(),
                                key=lambda item: (-len(item[1]), item[0])):
        print(f"\n== {name}: {len(members)}")
        for record in members:
            print(f"   {record['sample']}\n       {record['diagnostic'][:160]}")
    checked = len(records) - len(failures)
    print(f"\nsample_gate: {checked}/{len(records)} check on "
          f"{platform.system()} {platform.machine()}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
