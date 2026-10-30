#!/usr/bin/env python3
"""Discover project roles from the existing tree; no checked-in inventory.

Build declarations and existing control names provide conservative defaults.
An optional sample.json states ambiguous roles, non-main entry filenames and
authored rejection fragments. These are harness labels, never language or
package authority. Nested projects remain independent discovery results.
"""

from __future__ import annotations

import argparse
import configparser
from dataclasses import dataclass
import json
import os
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
ROLES = ("program", "example", "library-consumer", "library", "check-only", "intended-invalid")
# `invalid-control` names the same disposition as `intended-invalid`. Eighteen
# sample.json files under generic_content_transfer and linear_routed_label use it,
# and an unrecognized role aborts the unfiltered facts gate at discovery for every
# lane. Accepting the spelling here keeps those controls scored instead of
# crashing the gate; it is not a second role.
ROLE_SYNONYMS = {"invalid-control": "intended-invalid"}
REJECTION_FILES = ("expected_diagnostic.txt", "expected_error.txt", "expected.txt")


def work_kind(sample: str) -> str:
    """Work-queue taxonomy, separate from a project's compilation role.

    Discovery and queue consumption must omit the same probe/control trees.
    A program without a host entry or oracle still needs reporting; neither
    absence licenses silently dropping it from application discovery.
    """
    parts = Path(sample).parts
    if "probes" in parts:
        return "probe"
    name = Path(sample).name
    if "controls" in parts or name.startswith("invalid") \
            or name in ("intended_invalid", "out_of_bounds"):
        return "control"
    return "program"


@dataclass(frozen=True)
class Project:
    directory: Path
    entry: Path | None
    role: str
    rejection: tuple[str, ...] = ()
    rejection_any: tuple[str, ...] = ()
    rejection_casefold: bool = False


def external_paths(repository: Path) -> tuple[Path, ...]:
    modules = configparser.ConfigParser()
    modules.read(repository / ".gitmodules", encoding="utf-8")
    return tuple((repository / modules[section]["path"]).resolve()
                 for section in modules.sections() if "path" in modules[section])


def project_at(directory: Path, repository: Path = ROOT) -> Project:
    directory = directory.resolve()
    metadata_file = directory / "sample.json"
    metadata = json.loads(metadata_file.read_text(encoding="utf-8")) if metadata_file.exists() else {}
    if not isinstance(metadata, dict) or set(metadata) - {"role", "entry", "rejection_contains", "rejection_any_contains", "rejection_casefold"}:
        raise ValueError(f"{metadata_file}: unsupported project metadata")
    entry_name = metadata.get("entry")
    if entry_name is None:
        if (directory / "main.omg").is_file():
            entry_name = "main.omg"
        else:
            sources = sorted(path.name for path in directory.glob("*.omg") if path.name != "build.omg")
            entry_name = sources[0] if len(sources) == 1 else None
    entry = None
    if entry_name is not None:
        if not isinstance(entry_name, str) or Path(entry_name).name != entry_name or not entry_name.endswith(".omg"):
            raise ValueError(f"{metadata_file}: entry must name one local Omega source")
        entry = directory / entry_name
        if not entry.is_file():
            raise ValueError(f"{metadata_file}: missing entry {entry_name}")
    build = (directory / "build.omg").read_text(encoding="utf-8") if (directory / "build.omg").exists() else ""
    # Text hints classify existing projects, not source validity or entry admission.
    build = re.sub(r"//[^\n]*|/\*.*?\*/", "", build, flags=re.S)
    relative = directory.relative_to(repository.resolve())
    control_names = relative.parts
    name = directory.name
    negative = name in {"invalid", "intended_invalid", "out_of_bounds"} or name.startswith("invalid_")
    diagnostic_file = next((directory / name for name in REJECTION_FILES if (directory / name).is_file()), None)
    if diagnostic_file and "controls" in control_names:
        negative = True
    if negative:
        inferred = "intended-invalid"
    elif relative.parts[:2] == ("source", "library") or "Build::package" in build:
        inferred = "library"
    elif "ProgramEntry" not in build:
        inferred = "check-only"
    elif re.search(r"source/library/(?!std[\"/]|core[\"/]|alloc[\"/])", build):
        inferred = "library-consumer"
    elif "Build::application" in build:
        inferred = "program"
    else:
        inferred = "example"
    role = ROLE_SYNONYMS.get(metadata.get("role", inferred), metadata.get("role", inferred))
    if role not in ROLES:
        raise ValueError(f"{metadata_file}: unknown role {role!r}")
    fragments = metadata.get("rejection_contains")
    if fragments is None:
        fragments = [line for line in diagnostic_file.read_text(encoding="utf-8").splitlines() if line.strip()] if role == "intended-invalid" and diagnostic_file else []
    if not isinstance(fragments, list) or any(not isinstance(part, str) or not part.strip() for part in fragments):
        raise ValueError(f"{metadata_file}: rejection_contains must contain nonempty literal fragments")
    alternatives = metadata.get("rejection_any_contains", [])
    if not isinstance(alternatives, list) or any(not isinstance(part, str) or not part.strip() for part in alternatives):
        raise ValueError(f"{metadata_file}: rejection_any_contains must contain nonempty literal fragments")
    casefold = metadata.get("rejection_casefold", False)
    if not isinstance(casefold, bool):
        raise ValueError(f"{metadata_file}: rejection_casefold must be a boolean")
    if (fragments or alternatives or casefold) and role != "intended-invalid":
        raise ValueError(f"{metadata_file}: only intended-invalid projects carry rejection expectations")
    return Project(directory, entry, role, tuple(fragments), tuple(alternatives), casefold)


def discover(root: Path, repository: Path = ROOT, filters: tuple[str, ...] = ()) -> list[Project]:
    repository = repository.resolve()
    excluded = external_paths(repository)
    found = []
    for name, subdirectories, files in os.walk(root):
        directory = Path(name).resolve()
        subdirectories[:] = sorted(child for child in subdirectories if child not in {"build", ".git"})
        if any(path == directory or path in directory.parents for path in excluded):
            subdirectories[:] = []
            continue
        if "build.omg" in files:
            relative = directory.relative_to(repository).as_posix()
            if filters and not any(fragment in relative for fragment in filters):
                continue
            found.append(project_at(directory, repository))
            # Do not prune: another build root below this one owns its own entry.
    return sorted(found, key=lambda project: project.directory.as_posix())


def rejection_verdict(project: Project, exit_code: int, output: str, expected_exit: int = 1) -> tuple[bool, str]:
    if not project.rejection and not project.rejection_any:
        return False, "missing authored rejection expectation"
    # Checking rejects with 1; run admission refuses with 200. Signals and panic exits
    # cannot satisfy a source rule even if their output echoes a fragment.
    if exit_code != expected_exit:
        return False, f"expected rejection exit {expected_exit}, got {exit_code}"
    normalize = str.casefold if project.rejection_casefold else lambda text: text
    output = normalize(output)
    missing = [fragment for fragment in project.rejection if normalize(fragment) not in output]
    if missing:
        return False, f"wrong rejection reason: missing {missing!r}"
    if project.rejection_any and not any(normalize(fragment) in output for fragment in project.rejection_any):
        return False, f"wrong rejection reason: expected one of {project.rejection_any!r}"
    return True, "intended rejection matched"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT / "samples")
    parser.add_argument("--role", choices=ROLES)
    options = parser.parse_args()
    projects = discover(options.root)
    if options.root.resolve() == (ROOT / "source/library").resolve():
        seen = {project.directory for project in projects}
        projects += [project_at(directory) for directory in sorted(options.root.iterdir())
                     if directory.is_dir() and directory.resolve() not in seen and any(directory.glob("*.omg"))]
    records = [{"path": project.directory.relative_to(ROOT).as_posix(), "role": project.role,
                "entry": project.entry.name if project.entry else None,
                "rejection_contains": list(project.rejection),
                "rejection_any_contains": list(project.rejection_any),
                "rejection_casefold": project.rejection_casefold}
               for project in projects if not options.role or project.role == options.role]
    if not options.role:
        records += [{"path": path.relative_to(ROOT).as_posix(), "role": "external", "entry": None}
                    for path in external_paths(ROOT) if options.root.resolve() in path.parents]
    print(json.dumps(sorted(records, key=lambda record: record["path"]), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
