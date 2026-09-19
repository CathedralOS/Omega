#!/usr/bin/env python3
"""Coordinate a wave of sessions, each advancing one board item.

Why pre-assignment exists: the repository's advance skill selects its own work
from the execution boards, which is correct for one agent and a collision when
N sessions start at once. The manifest is the coordinator's partitioning
decision: one session, one named board item, one set of owning paths. Sessions
then run the ordinary advance protocol end to end, including landing through
tools/landing.py; the swarm adds nothing to the boards themselves.

Sessions are either Devin Cloud agents (host "linux"; plan/launch/status/report)
or local agents on this machine (host "local"; the `local` subcommand). A local
session runs the identical claim/land/release protocol in a Git worktree under
.codex/worktrees/; `local` renders its prompts, optionally creates the
worktrees, and prints the launch table for the coordinator to spawn from.
Local waves have no session receipts, so status/report remain cloud-only —
tools/swarm/worktree_status.py is the local equivalent.

What this launcher deliberately does NOT do: it writes no board text, holds no
landing claim, creates no Git refs beyond the worktrees `local` is explicitly
asked for, and keeps no ownership state. Its receipts live only in the ignored
build/swarm/ directory so worker/session IDs stay off the boards. Partitioning
lives only in the manifest the coordinator wrote; overlaps with in-flight
human work are excluded there, not detected here.

Every subcommand prints one JSON object. Credentials come only from the
DEVIN_API_KEY and DEVIN_ORG_ID environment variables and are never printed.
"""

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request

# Sibling-tool import; claims.py lives in tools/.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import claims

DEFAULT_BASE_URL = "https://api.devin.ai/v3"
BOARDS = ("TASKS.md", "TASKS_BOOTSTRAP.md", "TASKS_OPTIMIZER.md")
MAX_ATTEMPTS = 3
PROMPT_FIELDS = ("name", "wave", "item", "board", "owner_label", "exclusions",
                 "max_acu_limit", "suggested_first_slice_block",
                 "probe_block", "claim_command", "structured_output_schema")
LOCAL_PROMPT_FIELDS = ("name", "wave", "item", "board", "owner_label",
                       "repository", "worktree", "branch", "claim_command",
                       "wave_items", "exclusions", "build_tool", "host_block",
                       "suggested_first_slice_block", "probe_block",
                       "continuation_block")
LOCAL_CLAIM_LEASE_MINUTES = 120

STRUCTURED_OUTPUT_SCHEMA = {
    "type": "object",
    "required": ["result", "commits", "checks", "remaining_dependency",
                 "unrelated_failures", "lease_expiries"],
    "properties": {
        "result": {"type": "string",
                   "enum": ["landed", "verification_only", "blocked",
                            "superseded", "budget_exhausted", "aborted"]},
        "commits": {"type": "array",
                    "items": {"type": "object",
                              "required": ["sha", "subject"],
                              "properties": {"sha": {"type": "string"},
                                             "subject": {"type": "string"}}}},
        "checks": {"type": "array",
                   "items": {"type": "object",
                             "required": ["command", "host", "exit"],
                             "properties": {"command": {"type": "string"},
                                            "host": {"type": "string"},
                                            "exit": {"type": "integer"}}}},
        "customer_before": {"type": "string"},
        "customer_after": {"type": "string"},
        "remaining_dependency": {"type": "string"},
        "unrelated_failures": {"type": "array", "items": {"type": "string"}},
        "lease_expiries": {"type": "integer"},
        "first_build_seconds": {"type": "integer"},
        "retained_worktree": {"type": "string"},
        "notes": {"type": "string"},
    },
}


class SwarmError(Exception):
    pass


def emit(record):
    print(json.dumps(record, ensure_ascii=True, indent=2))


def repository_root(arguments):
    if arguments.repository:
        return Path(arguments.repository).resolve()
    override = os.environ.get("OMEGA_SWARM_REPOSITORY")
    if override:
        return Path(override).resolve()
    return Path(__file__).resolve().parents[2]


def build_directory(repository, wave):
    return repository / "build" / "swarm" / wave


def git(repository, *arguments):
    result = subprocess.run(["git", "-C", str(repository), *arguments],
                            capture_output=True, text=True, encoding="utf-8",
                            errors="replace",
                            env={**os.environ, "GIT_TERMINAL_PROMPT": "0"})
    if result.returncode:
        raise SwarmError(f"Git failed ({result.returncode}): {result.stderr.strip()}")
    return result.stdout.strip()


def require_fields(record, fields, where):
    missing = [field for field in fields if field not in record]
    if missing:
        raise SwarmError(f"{where} is missing required fields: {', '.join(missing)}")


def validate_manifest(manifest, repository):
    if not isinstance(manifest, dict):
        raise SwarmError("Manifest must be a JSON object.")
    require_fields(manifest, ("wave", "max_acu_limit", "bypass_approval", "resumable",
                            "tags", "owner_label_prefix", "exclusions", "sessions"),
                   "manifest")
    exclusions = set(manifest["exclusions"])
    sessions = manifest["sessions"]
    if not isinstance(sessions, list) or not sessions:
        raise SwarmError("manifest.sessions must be a non-empty list.")
    claimed = []
    for session in sessions:
        where = f"session {session.get('name', '?')!r}"
        require_fields(session, ("name", "board", "item", "host", "owning_paths"), where)
        if session["item"] in exclusions or session["name"] in exclusions:
            raise SwarmError(f"{session['name']}: item {session['item']} is in exclusions.")
        if session["board"] not in BOARDS:
            raise SwarmError(f"{session['name']}: unknown board {session['board']!r}; "
                             f"expected one of {', '.join(BOARDS)}.")
        if session["host"] not in ("linux", "local"):
            raise SwarmError(f"{session['name']}: host must be linux or local, "
                             f"got {session['host']!r}.")
        if not isinstance(session["owning_paths"], list) or not session["owning_paths"]:
            raise SwarmError(f"{session['name']}: owning_paths must be a non-empty list.")
        normalized_paths = []
        for path in session["owning_paths"]:
            if not isinstance(path, str):
                raise SwarmError(f"{session['name']}: owning_paths entries must "
                                 f"be strings, got {path!r}.")
            try:
                normalized_paths.append(claims.normalize_path(path))
            except claims.ClaimsError as error:
                raise SwarmError(f"{session['name']}: {error}") from error
        session["owning_paths"] = normalized_paths
        if "host_gates" in session:
            gates = session["host_gates"]
            if (not isinstance(gates, list) or not gates
                    or any(not isinstance(command, str) or not command.strip()
                           for command in gates)):
                raise SwarmError(f"{session['name']}: host_gates must be a non-empty "
                                 "list of non-empty strings.")
        if "probe_only" in session and not isinstance(session["probe_only"], bool):
            raise SwarmError(f"{session['name']}: probe_only must be a boolean.")
        layer = session.get("layer", 0)
        if not isinstance(layer, int) or isinstance(layer, bool) or layer < 0:
            raise SwarmError(f"{session['name']}: layer must be a non-negative integer.")
        for path in session["owning_paths"]:
            for existing_path, existing_name, existing_layer in claimed:
                if (existing_layer == layer
                        and claims.paths_overlap(path, existing_path)):
                    raise SwarmError(f"Owning path {path} ({session['name']}) overlaps "
                                     f"{existing_path} claimed by {existing_name} in "
                                     f"layer {layer}; put the later session in a "
                                     "higher layer.")
            claimed.append((path, session["name"], layer))
        board_text_path = repository / session["board"]
        if not board_text_path.is_file():
            raise SwarmError(f"{session['name']}: board file {session['board']} not found.")
        marker = f"**{session['item']}.**"
        if marker not in board_text_path.read_text(encoding="utf-8", errors="replace"):
            raise SwarmError(f"{session['name']}: item {session['item']} not found "
                             f"in {session['board']} (no {marker!r} marker).")
    return sessions


def run_host_gates(repository, session):
    results = []
    for command in session.get("host_gates", []):
        try:
            result = subprocess.run(command, shell=True, cwd=repository,
                                    capture_output=True, text=True, timeout=600)
        except subprocess.TimeoutExpired:
            exit_code = -1
        else:
            exit_code = result.returncode
        results.append({"command": command, "exit": exit_code})
    return results


def host_gate_results(repository, sessions, skip=False):
    if skip:
        return {session["name"]: "skipped" for session in sessions}
    results = {session["name"]: run_host_gates(repository, session)
               for session in sessions}
    failures = []
    for session in sessions:
        if session.get("probe_only"):
            continue
        for result in results[session["name"]]:
            if result["exit"] != 0:
                failures.append(
                    f"{session['name']}: host gate failed on this host: "
                    f"{result['command']} (exit {result['exit']}); mark the slot "
                    "probe_only or drop it")
    if failures:
        raise SwarmError("\n".join(failures))
    return results


def route_crates(repository):
    try:
        result = subprocess.run(
            ["cargo", "metadata", "--format-version", "1"],
            cwd=repository, capture_output=True, text=True, timeout=300)
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return None
    if result.returncode:
        return None
    try:
        metadata = json.loads(result.stdout)
    except ValueError:
        return None
    packages = {
        package["id"]: package
        for package in metadata.get("packages", [])
        if package.get("id") in metadata.get("workspace_members", [])
    }
    workspace_members = set(packages)
    omega_id = next(
        (package_id for package_id, package in packages.items()
         if package.get("name") == "omega"),
        None)
    if omega_id is None:
        return {
            Path(package["manifest_path"]).parent.relative_to(repository).as_posix():
            {"name": package["name"], "on_route": False}
            for package in packages.values()
        }
    dependencies = {
        node["id"]: {
            dependency["pkg"]
            for dependency in node.get("deps", [])
            if dependency.get("pkg") in workspace_members
        }
        for node in metadata.get("resolve", {}).get("nodes", [])
    }
    reached = set()
    pending = [omega_id]
    while pending:
        package_id = pending.pop()
        if package_id in reached:
            continue
        reached.add(package_id)
        pending.extend(dependencies.get(package_id, ()))
    crates = {}
    for package_id, package in packages.items():
        crate_dir = Path(package["manifest_path"]).parent
        try:
            crate_key = crate_dir.relative_to(repository).as_posix()
        except ValueError:
            continue
        crates[crate_key] = {
            "name": package["name"],
            "on_route": package_id in reached,
        }
    return crates


def crate_directory(repository, path):
    source_path = repository / path
    if source_path.exists():
        candidate = source_path if source_path.is_dir() else source_path.parent
    else:
        candidate = source_path
        while candidate != repository and not candidate.exists():
            candidate = candidate.parent
        if candidate.is_file():
            candidate = candidate.parent
    while True:
        if (candidate / "Cargo.toml").is_file():
            return candidate
        if candidate == repository:
            return None
        candidate = candidate.parent


def load_manifest(path, repository):
    try:
        manifest = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise SwarmError(f"Cannot read manifest {path}: {error}") from error
    validate_manifest(manifest, repository)
    return manifest


def freshness_probe(repository, session, crates=None):
    """Informational only: how hot each owning path and the board file are."""
    lines = []
    paths = list(session["owning_paths"]) + [session["board"]]
    for path in paths:
        last = git(repository, "log", "-1", "--format=%ad,%an", "--date=short",
                   "--", path)
        recent = git(repository, "log", "--since=7.days.ago", "--format=%H",
                     "--", path)
        count = len(recent.splitlines()) if recent else 0
        crate = None
        crate_name = None
        on_route = "skipped" if crates == "skipped" else "unknown"
        crate_directory_path = crate_directory(repository, path)
        if crate_directory_path is not None:
            crate = crate_directory_path.relative_to(repository).as_posix()
            if crates not in (None, "skipped"):
                crate_record = crates.get(crate)
                if crate_record is not None:
                    crate_name = crate_record["name"]
                    on_route = crate_record["on_route"]
        lines.append({"path": path,
                      "last_commit": last or "no commits",
                      "commits_7d": count,
                      "crate": crate,
                      "crate_name": crate_name,
                      "on_route": on_route})
    return lines


def claims_report(repository, sessions, skip=False):
    """Live registry conflicts per session; "unavailable" when unreachable."""
    if skip:
        return {session["name"]: "skipped" for session in sessions}
    try:
        registry = claims.Claims(repository, "origin")
        snapshot = registry.snapshot()
    except Exception:
        return {session["name"]: "unavailable" for session in sessions}
    report = {}
    failures = []
    for session in sessions:
        found = registry.conflicts(snapshot["record"], session["item"],
                                   session["owning_paths"])
        report[session["name"]] = found
        for conflict in found:
            failures.append(
                f"{session['name']}: {session['item']} conflicts with a live "
                f"claim by {conflict['owner']} ({conflict['reason']}, ticket "
                f"{conflict['ticket']}, expires {conflict['expires_utc']})")
    if failures:
        raise SwarmError("\n".join(failures) +
                         "\nWait for those claims, coordinate a handoff, or "
                         "pass --skip-claims-check.")
    return report


def route_check(sessions, freshness_by_name):
    failures = []
    for session in sessions:
        if session.get("probe_only"):
            continue
        for line in freshness_by_name[session["name"]]:
            if not line["path"].startswith("omega-rust/"):
                continue
            if line["on_route"] is False:
                failures.append(
                    f"{session['name']}: owning path {line['path']} is in crate "
                    f"{line['crate']} which is not on the omega route; mark the "
                    "slot probe_only or drop it")
    if failures:
        raise SwarmError("\n".join(failures))


DEPENDENCY_LANGUAGE = ("depends on", "join", "joins", "preceding task",
                       "blocks on", "after the")
ITEM_TOKEN = re.compile(r"`?([A-Z][A-Z0-9]+(?:-[A-Z0-9]+)+)`?")
PATH_TOKEN = re.compile(r"`?([A-Za-z_][\w.-]*/[\w.-]+(?:/[\w.-]+)*)`?")
SCALE_CRATE_LIMIT = 4


def item_section(repository, board, item):
    """The item's board text block: marker line through the next top-level
    item or heading. Path/crate mentions and dependency language live here."""
    board_path = repository / board
    if not board_path.is_file():
        return ""
    text = board_path.read_text(encoding="utf-8", errors="replace")
    marker = f"**{item}.**"
    start = text.find(marker)
    if start == -1:
        return ""
    start = text.rfind("\n", 0, start) + 1
    kept = []
    for line in text[start:].splitlines():
        if kept and (line.startswith("- **") or line.startswith("#")):
            break
        kept.append(line)
    return "\n".join(kept)


def board_item_names(repository):
    names = set()
    for board in BOARDS:
        board_path = repository / board
        if not board_path.is_file():
            continue
        text = board_path.read_text(encoding="utf-8", errors="replace")
        names.update(re.findall(r"^- \*\*([A-Z][A-Z0-9-]+)\.\*\*",
                                text, flags=re.MULTILINE))
    return names


def partition_hints(repository, session, sessions, crates):
    """Coordinator-facing partitioning signals, advisory only.

    - dependency_language: phrases in the board text that suggest this item
      is sequenced after other work (layering candidates).
    - references_items / same_layer_reference: board items named in this
      item's text; flagged when one is another session at the same layer.
    - uncovered_mentions: path or crate-directory mentions in the item text
      that owning_paths do not cover — the fence would protect the wrong
      ground (the product-references drift pattern).
    - named_crate_count / scale_hint: how many crates the text spans; large
      items are multi-layer decompositions, not slices.
    """
    hints = {}
    text = item_section(repository, session["board"], session["item"])
    if not text:
        return hints
    lowered = text.lower()
    language = [phrase for phrase in DEPENDENCY_LANGUAGE
                if re.search(r"(?<![\w-])" + re.escape(phrase) + r"\b", lowered)]
    if language:
        hints["dependency_language"] = language
    known_items = board_item_names(repository)
    referenced = sorted(token for token in set(ITEM_TOKEN.findall(text))
                        if token in known_items and token != session["item"])
    if referenced:
        hints["references_items"] = referenced
        by_item = {other["item"]: other for other in sessions
                   if other["name"] != session["name"]}
        layer = session.get("layer", 0)
        same = [item for item in referenced
                if item in by_item and by_item[item].get("layer", 0) == layer]
        if same:
            hints["same_layer_reference"] = (
                f"item text references {', '.join(same)} which is a session at "
                f"the same layer {layer}; order them or split into layers")
    crate_dirs = {}
    if isinstance(crates, dict):
        crate_dirs = {record["name"]: directory
                      for directory, record in crates.items()}
    uncovered = set()
    named_crates = set()
    for token in PATH_TOKEN.findall(text):
        parts = token.split("/")
        if len(parts) < 2:
            continue
        resolved = None
        if (repository / token).exists() or parts[0] in (
                "omega-rust", "tests", "bootstrap", "tools", "wiki", "samples"):
            resolved = token
        elif parts[0] in crate_dirs:
            resolved = crate_dirs[parts[0]]
            named_crates.add(parts[0])
        if resolved is None:
            continue
        if not any(claims.paths_overlap(resolved, owning)
                   for owning in session["owning_paths"]):
            uncovered.add(token)
    for name, directory in crate_dirs.items():
        if name in named_crates:
            continue
        if f"`{name}`" not in text:
            continue
        named_crates.add(name)
        if not any(claims.paths_overlap(directory, owning)
                   for owning in session["owning_paths"]):
            uncovered.add(name)
    if uncovered:
        hints["uncovered_mentions"] = sorted(uncovered)
    if named_crates:
        hints["named_crate_count"] = len(named_crates)
        if len(named_crates) >= SCALE_CRATE_LIMIT:
            hints["scale_hint"] = (
                f"item text names {len(named_crates)} crates; consider "
                "decomposing into layers rather than one slice")
    return hints


def session_blocks(session):
    slice_block = ""
    if session.get("suggested_first_slice"):
        slice_block = (
            "Suggested first slice (a suggestion only; the board item's "
            "acceptance governs): " + session["suggested_first_slice"])
    probe_block = ""
    if session.get("probe_only"):
        probe_block = (
            "Probe-only slot: the coordinator expects this item may not have a "
            "bounded first slice on this host. `verification_only` with the "
            "witnessed rejection, the exact missing seam, and the next "
            "acceptance in `remaining_dependency` is a planned success here, "
            "not a failure; still land a bounded improvement if one exists.")
    return slice_block, probe_block


def claim_command(session, owner_label, lease_minutes=None):
    paths = " ".join(f"--path {path}" for path in session["owning_paths"])
    lease = (f" --lease-minutes {lease_minutes}" if lease_minutes else "")
    return (f"python3 tools/claims.py claim --board {session['board']} "
            f"--item {session['item']} --owner \"{owner_label}\""
            + (f" {paths}" if paths else "") + lease)


def sessions_for(manifest, command):
    want = "local" if command == "local" else "linux"
    selected = [s for s in manifest["sessions"] if s["host"] == want]
    skipped = [s["name"] for s in manifest["sessions"] if s["host"] != want]
    if not selected:
        other = "linux" if want == "local" else "local"
        raise SwarmError(f"Manifest has no {want}-host sessions "
                         f"({len(manifest['sessions'])} are {other}); use the "
                         "matching subcommand for them.")
    return selected, skipped


def filter_layer(sessions, layer):
    """Keep sessions at one dependency layer; all of them when unset."""
    if not isinstance(layer, int) or isinstance(layer, bool):
        return sessions
    selected = [session for session in sessions
                if session.get("layer", 0) == layer]
    if not selected:
        raise SwarmError(f"No sessions at layer {layer}.")
    return selected


def render_prompt(template, manifest, session):
    owner_label = f"{manifest['owner_label_prefix']}-{session['name']}"
    slice_block, probe_block = session_blocks(session)
    values = {
        "name": session["name"],
        "wave": manifest["wave"],
        "item": session["item"],
        "board": session["board"],
        "owner_label": owner_label,
        "claim_command": claim_command(session, owner_label),
        "exclusions": ", ".join(manifest["exclusions"]) or "(none)",
        "max_acu_limit": manifest["max_acu_limit"],
        "suggested_first_slice_block": slice_block,
        "probe_block": probe_block,
        "structured_output_schema": json.dumps(STRUCTURED_OUTPUT_SCHEMA, indent=2),
    }
    return template.format_map(values)


def local_host_block():
    system, machine = platform.system(), platform.machine()
    if system == "Darwin" and machine == "x86_64":
        return (
            "You are on Intel macOS (x86_64). `TargetProfile::host()` has no "
            "macos_x86_64 profile and `canary_suite`'s `native_hosted_target()` "
            "has no matching cfg arm: host-profiled tests panic with "
            "`unsupported host profile for Omega native planning` and the "
            "canary target does not compile here. See "
            "wiki/drafts/known_baseline_failures.md. Route omega invocations "
            "through `--target linux_x86_64` (or another declared target) and "
            "report native-host coverage as unavailable, never as passing.")
    if system == "Darwin":
        return ("You are on macOS arm64. Host-profiled checks and the canary "
                "suite run natively on this host.")
    return f"You are on {system} {machine}."


def local_session_state(repository, wave, session):
    worktree = repository / ".codex" / "worktrees" / f"{wave}-{session['name']}"
    branch = f"swarm/{wave}-{session['name']}"
    try:
        git(repository, "rev-parse", "--verify", f"refs/heads/{branch}")
        ahead = int(git(repository, "rev-list", "--count",
                        f"origin/main..{branch}") or 0)
    except (SwarmError, ValueError):
        ahead = 0
        branch_exists = False
    else:
        branch_exists = True
    if worktree.is_dir():
        state = "resumable" if ahead else "existing"
    elif ahead or branch_exists:
        state = "resumable_no_worktree"
    else:
        state = "absent"
    return {"worktree": worktree, "branch": branch, "ahead": ahead,
            "branch_exists": branch_exists, "state": state}


def render_local_prompt(template, manifest, session, repository, state):
    owner_label = f"{manifest['owner_label_prefix']}-{session['name']}"
    slice_block, probe_block = session_blocks(session)
    continuation_block = ""
    if state["ahead"]:
        continuation_block = (
            "Continuation slot: this branch already carries "
            f"{state['ahead']} unpublished commit(s) from an interrupted "
            "session. Inspect `git log origin/main..HEAD` and `git status` "
            "first and continue the existing work; do not restart or discard "
            "it without recording why in your report.")
    values = {
        "name": session["name"],
        "wave": manifest["wave"],
        "item": session["item"],
        "board": session["board"],
        "owner_label": owner_label,
        "repository": str(repository),
        "worktree": str(state["worktree"]),
        "branch": state["branch"],
        "claim_command": claim_command(session, owner_label,
                                       LOCAL_CLAIM_LEASE_MINUTES),
        "wave_items": ", ".join(s["item"] for s in manifest["sessions"]),
        "exclusions": ", ".join(manifest["exclusions"]) or "(none)",
        "build_tool": "mbx" if shutil.which("mbx") else "cargo",
        "host_block": local_host_block(),
        "suggested_first_slice_block": slice_block,
        "probe_block": probe_block,
        "continuation_block": continuation_block,
    }
    return template.format_map(values)


def command_local(arguments, repository):
    manifest = load_manifest(arguments.manifest, repository)
    sessions, skipped = sessions_for(manifest, "local")
    sessions = filter_layer(sessions, arguments.layer)
    if arguments.sessions:
        wanted = {name.strip() for name in arguments.sessions.split(",")
                  if name.strip()}
        known = {session["name"] for session in sessions}
        unknown = sorted(wanted - known)
        if unknown:
            raise SwarmError(
                f"Unknown local session(s) {', '.join(unknown)}; manifest "
                f"local sessions: {', '.join(sorted(known)) or '(none)'}")
        sessions = [session for session in sessions
                    if session["name"] in wanted]
    gate_results = host_gate_results(repository, sessions,
                                     skip=arguments.skip_host_gates)
    route_data = "skipped" if arguments.skip_route_check else route_crates(repository)
    freshness = {
        session["name"]: freshness_probe(repository, session, route_data)
        for session in sessions
    }
    if not arguments.skip_route_check:
        route_check(sessions, freshness)
    claims_state = claims_report(repository, sessions,
                                 skip=arguments.skip_claims_check)
    template = (Path(__file__).resolve().parent
                / "prompt_template_local.md").read_text(encoding="utf-8")
    wave_directory = build_directory(repository, manifest["wave"])
    prompts_directory = wave_directory / "prompts"
    prompts_directory.mkdir(parents=True, exist_ok=True)
    if arguments.create_worktrees:
        # New slots branch from `origin/main`, so fetch it first: spawning on
        # the cached ref converts main's drift into landing rebase debt for
        # every worker in the wave. A failed fetch keeps the cached ref and
        # is reported in `base` rather than silently spawning on staleness.
        try:
            git(repository, "fetch", "origin", "main")
            base = git(repository, "rev-parse", "--verify", "origin/main")
        except SwarmError as error:
            base = (f"origin/main fetch failed; new worktrees use the "
                    f"cached ref ({error})")
    else:
        base = None
    rows = []
    for session in sessions:
        state = local_session_state(repository, manifest["wave"], session)
        prompt = render_local_prompt(template, manifest, session, repository,
                                     state)
        unresolved = [field for field in LOCAL_PROMPT_FIELDS
                      if "{" + field + "}" in prompt]
        if unresolved:
            raise SwarmError(f"{session['name']}: local prompt template left "
                             f"placeholders unresolved: {unresolved}")
        prompt_path = prompts_directory / f"{session['name']}.md"
        with open(prompt_path, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(prompt)
        if arguments.create_worktrees and state["state"] in (
                "absent", "resumable_no_worktree"):
            worktree_arg = state["worktree"].relative_to(repository).as_posix()
            if state["branch_exists"]:
                git(repository, "worktree", "add", worktree_arg,
                    state["branch"])
            else:
                git(repository, "worktree", "add", worktree_arg, "-b",
                    state["branch"], "origin/main")
            state["state"] = ("resumable" if state["ahead"] else "existing")
        rows.append({
            "name": session["name"],
            "item": session["item"],
            "board": session["board"],
            "prompt": str(prompt_path.relative_to(repository)),
            "worktree": str(state["worktree"].relative_to(repository)),
            "branch": state["branch"],
            "worktree_state": state["state"],
            "owner_label":
                f"{manifest['owner_label_prefix']}-{session['name']}",
            "host_gates": gate_results[session["name"]],
            "claims": claims_state[session["name"]],
            "probe_only": bool(session.get("probe_only", False)),
            "freshness": freshness[session["name"]],
            "partition_hints": partition_hints(repository, session, sessions,
                                               route_data),
        })
    emit({"command": "local", "wave": manifest["wave"], "base": base,
          "skipped_non_local_sessions": skipped, "sessions": rows})
    return 0


def request_body(manifest, session, prompt):
    body = {
        "prompt": prompt,
        "title": f"swarm {manifest['wave']} {session['name']}: {session['item']}",
        "tags": manifest["tags"],
        "max_acu_limit": manifest["max_acu_limit"],
        "bypass_approval": manifest["bypass_approval"],
        "resumable": manifest["resumable"],
        "structured_output_schema": STRUCTURED_OUTPUT_SCHEMA,
        "structured_output_required": True,
    }
    if manifest.get("devin_mode") is not None:
        body["devin_mode"] = manifest["devin_mode"]
    return body


def credentials():
    key = os.environ.get("DEVIN_API_KEY", "")
    organization = os.environ.get("DEVIN_ORG_ID", "")
    if not key or not organization:
        raise SwarmError("DEVIN_API_KEY and DEVIN_ORG_ID must be set in the environment.")
    return key, organization


def api_request(method, url, body=None):
    key, _ = credentials()
    payload = json.dumps(body).encode("utf-8") if body is not None else None
    delay = 0.5
    for attempt in range(1, MAX_ATTEMPTS + 1):
        request = urllib.request.Request(url, data=payload, method=method)
        request.add_header("Authorization", f"Bearer {key}")
        request.add_header("Content-Type", "application/json")
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                text = response.read().decode("utf-8")
                return json.loads(text) if text.strip() else {}
        except urllib.error.HTTPError as error:
            if error.code in (401, 403):
                raise SwarmError(f"API authentication failed ({error.code}); "
                                 "check DEVIN_API_KEY and DEVIN_ORG_ID.") from error
            if error.code == 429 or error.code >= 500:
                if attempt == MAX_ATTEMPTS:
                    raise SwarmError(f"API {method} {url} failed with {error.code} "
                                     f"after {MAX_ATTEMPTS} attempts.") from error
                time.sleep(delay)
                delay *= 2
                continue
            detail = error.read().decode("utf-8", errors="replace")[:500]
            raise SwarmError(f"API {method} {url} failed ({error.code}): {detail}") from error
        except urllib.error.URLError as error:
            if attempt == MAX_ATTEMPTS:
                raise SwarmError(f"API {method} {url} unreachable: {error.reason}") from error
            time.sleep(delay)
            delay *= 2
    raise SwarmError("unreachable")


def session_url(session_id):
    return f"https://app.devin.ai/sessions/{session_id}"


def read_receipts(directory):
    path = directory / "receipts.json"
    if not path.is_file():
        return {}
    try:
        records = json.loads(path.read_text(encoding="utf-8"))
    except ValueError as error:
        raise SwarmError(f"Cannot parse {path}: {error}") from error
    return {record["name"]: record for record in records}


def write_receipts(directory, receipts):
    directory.mkdir(parents=True, exist_ok=True)
    ordered = [receipts[name] for name in sorted(receipts)]
    (directory / "receipts.json").write_text(
        json.dumps(ordered, ensure_ascii=True, indent=2) + "\n", encoding="utf-8")


def command_plan(arguments, repository):
    manifest = load_manifest(arguments.manifest, repository)
    sessions, skipped = sessions_for(manifest, "plan")
    sessions = filter_layer(sessions, arguments.layer)
    gate_results = host_gate_results(repository, sessions,
                                     skip=arguments.skip_host_gates)
    skip_route_check = arguments.skip_route_check
    route_data = "skipped" if skip_route_check else route_crates(repository)
    freshness = {
        session["name"]: freshness_probe(repository, session, route_data)
        for session in sessions
    }
    if not skip_route_check:
        route_check(sessions, freshness)
    claims_state = claims_report(repository, sessions,
                                 skip=arguments.skip_claims_check)
    template = (Path(__file__).resolve().parent / "prompt_template.md").read_text(
        encoding="utf-8")
    wave_directory = build_directory(repository, manifest["wave"])
    prompts_directory = wave_directory / "prompts"
    prompts_directory.mkdir(parents=True, exist_ok=True)
    planned = []
    for session in sessions:
        prompt = render_prompt(template, manifest, session)
        unresolved = [field for field in PROMPT_FIELDS
                      if "{" + field + "}" in prompt]
        if unresolved:
            raise SwarmError(f"{session['name']}: prompt template left placeholders "
                             f"unresolved: {unresolved}")
        prompt_path = prompts_directory / f"{session['name']}.md"
        with open(prompt_path, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(prompt)
        planned.append({
            "name": session["name"],
            "item": session["item"],
            "board": session["board"],
            "prompt": str(prompt_path.relative_to(repository)),
            "host_gates": gate_results[session["name"]],
            "claims": claims_state[session["name"]],
            "probe_only": bool(session.get("probe_only", False)),
            "freshness": freshness[session["name"]],
            "partition_hints": partition_hints(repository, session, sessions,
                                               route_data),
            "body": request_body(manifest, session, prompt),
        })
    emit({"command": "plan", "wave": manifest["wave"],
          "skipped_non_linux_sessions": skipped, "sessions": planned})
    return 0


def command_launch(arguments, repository):
    manifest = load_manifest(arguments.manifest, repository)
    sessions, skipped = sessions_for(manifest, "launch")
    sessions = filter_layer(sessions, arguments.layer)
    if not arguments.dry_run:
        host_gate_results(repository, sessions,
                          skip=arguments.skip_host_gates)
        route_data = "skipped" if arguments.skip_route_check else route_crates(repository)
        freshness = {
            session["name"]: freshness_probe(repository, session, route_data)
            for session in sessions
        }
        if not arguments.skip_route_check:
            route_check(sessions, freshness)
        claims_report(repository, sessions,
                      skip=arguments.skip_claims_check)
    template = (Path(__file__).resolve().parent / "prompt_template.md").read_text(
        encoding="utf-8")
    wave_directory = build_directory(repository, manifest["wave"])
    receipts = read_receipts(wave_directory)
    organization = ""
    if not arguments.dry_run:
        _, organization = credentials()
    launched = []
    for session in sessions:
        name = session["name"]
        if name in receipts and arguments.relaunch != name:
            launched.append({"name": name, "skipped": "receipt_exists",
                             "session_id": receipts[name]["session_id"]})
            continue
        prompt = render_prompt(template, manifest, session)
        body = request_body(manifest, session, prompt)
        if arguments.dry_run:
            launched.append({"name": name, "dry_run": True, "body": body})
            continue
        response = api_request(
            "POST",
            f"{arguments.base_url}/organizations/{organization}/sessions", body)
        session_id = response.get("session_id") or response.get("id")
        if not session_id:
            raise SwarmError(f"{name}: create response had no session id: {response}")
        receipts[name] = {
            "name": name,
            "session_id": session_id,
            "url": response.get("url") or session_url(session_id),
            "created_utc": datetime.now(timezone.utc).isoformat(),
            "body": body,
        }
        write_receipts(wave_directory, receipts)
        launched.append({"name": name, "session_id": session_id,
                         "url": receipts[name]["url"]})
    emit({"command": "launch", "wave": manifest["wave"],
          "dry_run": bool(arguments.dry_run),
          "skipped_non_linux_sessions": skipped, "sessions": launched})
    return 0


def fetch_sessions(arguments, repository):
    wave_directory = build_directory(repository, arguments.wave)
    receipts = read_receipts(wave_directory)
    if not receipts:
        raise SwarmError(f"No receipts under {wave_directory}; run launch first.")
    _, organization = credentials()
    sessions = []
    for name in sorted(receipts):
        receipt = receipts[name]
        response = api_request(
            "GET",
            f"{arguments.base_url}/organizations/{organization}"
            f"/sessions/{receipt['session_id']}")
        sessions.append({"receipt": receipt, "status": response})
    return sessions


def command_status(arguments, repository):
    rows = []
    for entry in fetch_sessions(arguments, repository):
        status = entry["status"]
        rows.append({
            "name": entry["receipt"]["name"],
            "session_id": entry["receipt"]["session_id"],
            "status": status.get("status"),
            "status_detail": status.get("status_detail"),
            "acus_consumed": status.get("acus_consumed"),
            "has_structured_output": bool(status.get("structured_output")),
            "url": entry["receipt"]["url"],
        })
    emit({"command": "status", "wave": arguments.wave, "sessions": rows})
    return 0


def wave_assignments(repository, wave):
    waves = repository / "tools" / "swarm" / "waves"
    if not waves.is_dir():
        return {}
    for candidate in sorted(waves.glob("*.json")):
        if candidate.name.endswith(".outcomes.json"):
            continue
        try:
            manifest = json.loads(candidate.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue
        if manifest.get("wave") == wave:
            return {session.get("name"): session
                    for session in manifest.get("sessions", [])}
    return {}


def item_closed(repository, assignment):
    board = assignment.get("board")
    item = assignment.get("item")
    if not board or not item:
        return None
    board_path = repository / board
    if not board_path.is_file():
        return None
    marker = f"**{item}.**"
    return marker not in board_path.read_text(encoding="utf-8",
                                              errors="replace")


def report_summary(rows):
    results = {}
    for row in rows:
        key = str(row.get("result"))
        results[key] = results.get(key, 0) + 1
    return {"sessions": len(rows), "results": results,
            "items_closed": sum(1 for row in rows
                                if row.get("item_closed") is True),
            "acus_consumed": sum(row["acus_consumed"] for row in rows
                                 if isinstance(row.get("acus_consumed"),
                                               (int, float)))}


def save_outcomes(repository, wave, rows, summary):
    outcomes_path = (repository / "tools" / "swarm" / "waves"
                     / f"{wave}.outcomes.json")
    fields = ("name", "board", "item", "result", "acus_consumed",
              "item_closed", "commits", "remaining_dependency")
    sessions = [{key: row[key] for key in fields if key in row}
                for row in rows]
    record = {"wave": wave,
              "recorded_utc": datetime.now(timezone.utc).isoformat(),
              "summary": summary, "sessions": sessions}
    outcomes_path.write_text(json.dumps(record, ensure_ascii=True, indent=2)
                             + "\n", encoding="utf-8")
    return outcomes_path


def command_report(arguments, repository):
    fetched = fetch_sessions(arguments, repository)
    wave_directory = build_directory(repository, arguments.wave)
    wave_directory.mkdir(parents=True, exist_ok=True)
    assignments = wave_assignments(repository, arguments.wave)
    rows = []
    missing = []
    for entry in fetched:
        receipt, status = entry["receipt"], entry["status"]
        output = status.get("structured_output")
        if not output:
            missing.append(receipt["name"])
            row = {"name": receipt["name"],
                   "result": f"no structured output (status: "
                             f"{status.get('status')})",
                   "acus_consumed": status.get("acus_consumed"),
                   "url": receipt["url"]}
        else:
            row = {
                "name": receipt["name"],
                "result": output.get("result"),
                "acus_consumed": status.get("acus_consumed"),
                "commits": [f"{c.get('sha', '')[:10]} {c.get('subject', '')}"
                            for c in output.get("commits", [])],
                "checks": [f"{c.get('command')} ({c.get('host')}) exit {c.get('exit')}"
                           for c in output.get("checks", [])],
                "remaining_dependency": output.get("remaining_dependency"),
                "unrelated_failures": output.get("unrelated_failures", []),
                "lease_expiries": output.get("lease_expiries"),
                "first_build_seconds": output.get("first_build_seconds"),
                "url": receipt["url"],
            }
        assignment = assignments.get(receipt["name"], {})
        if assignment.get("item"):
            row["board"] = assignment["board"]
            row["item"] = assignment["item"]
            closed = item_closed(repository, assignment)
            if closed is not None:
                row["item_closed"] = closed
        rows.append(row)
    lines = [f"# Swarm wave {arguments.wave} report", ""]
    if missing:
        lines.append("Sessions without structured output: "
                     + ", ".join(missing) + ".")
        lines.append("")
    for row in rows:
        lines.append(f"## {row['name']}")
        lines.append(f"- result: {row['result']}")
        lines.append(f"- acus_consumed: {row.get('acus_consumed')}")
        for key in ("item", "item_closed", "commits", "checks",
                    "remaining_dependency", "unrelated_failures",
                    "lease_expiries", "first_build_seconds", "url"):
            if key in row:
                value = row[key]
                if isinstance(value, list):
                    lines.append(f"- {key}:")
                    lines.extend(f"  - {item}" for item in value)
                else:
                    lines.append(f"- {key}: {value}")
        lines.append("")
    report_path = wave_directory / "report.md"
    with open(report_path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write("\n".join(lines))
    summary = report_summary(rows)
    record = {"command": "report", "wave": arguments.wave,
              "report": str(report_path.relative_to(repository)),
              "sessions_without_structured_output": missing,
              "summary": summary, "sessions": rows}
    if arguments.save:
        outcomes_path = save_outcomes(repository, arguments.wave, rows, summary)
        record["outcomes"] = str(outcomes_path.relative_to(repository))
    emit(record)
    return 0


class Parser(argparse.ArgumentParser):
    def error(self, message):
        raise SwarmError(message)


def main(argv=None):
    parser = Parser(description=__doc__)
    parser.add_argument("--repository", help="repository root (default: this checkout)")
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL)
    subparsers = parser.add_subparsers(dest="command", required=True)
    plan = subparsers.add_parser("plan")
    plan.add_argument("--manifest", required=True)
    plan.add_argument("--layer", type=int,
                      help="only process sessions at this dependency layer "
                           "(default: all layers)")
    plan.add_argument("--skip-host-gates", action="store_true")
    plan.add_argument("--skip-route-check", action="store_true")
    plan.add_argument("--skip-claims-check", action="store_true")
    launch = subparsers.add_parser("launch")
    launch.add_argument("--manifest", required=True)
    launch.add_argument("--layer", type=int,
                        help="only launch sessions at this dependency layer "
                             "(default: all layers)")
    launch.add_argument("--dry-run", action="store_true")
    launch.add_argument("--skip-host-gates", action="store_true")
    launch.add_argument("--skip-route-check", action="store_true")
    launch.add_argument("--skip-claims-check", action="store_true")
    launch.add_argument("--relaunch")
    local = subparsers.add_parser(
        "local", help="render local-wave prompts and print the launch table; "
                      "the coordinator spawns the agents itself")
    local.add_argument("--manifest", required=True)
    local.add_argument("--create-worktrees", action="store_true",
                       help="create .codex/worktrees/<wave>-<name> and "
                            "swarm/<wave>-<name> branches before spawning")
    local.add_argument("--layer", type=int,
                       help="only render sessions at this dependency layer "
                            "(default: all layers); launch layer N only after "
                            "layer N-1 has landed or parked")
    local.add_argument("--sessions",
                       help="comma-separated session names to render "
                            "(default: all local sessions in the manifest); "
                            "use it to relaunch a subset without editing the "
                            "manifest")
    local.add_argument("--skip-host-gates", action="store_true")
    local.add_argument("--skip-route-check", action="store_true")
    local.add_argument("--skip-claims-check", action="store_true")
    status = subparsers.add_parser("status")
    status.add_argument("--wave", required=True)
    report = subparsers.add_parser("report")
    report.add_argument("--wave", required=True)
    report.add_argument("--save", action="store_true",
                        help="also write the tracked outcome record "
                             "tools/swarm/waves/<wave>.outcomes.json")
    arguments = None
    try:
        arguments = parser.parse_args(argv)
        repository = repository_root(arguments)
        handler = {"plan": command_plan, "launch": command_launch,
                   "local": command_local, "status": command_status,
                   "report": command_report}[arguments.command]
        return handler(arguments, repository)
    except (SwarmError, OSError) as error:
        print(f"swarm: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
