#!/usr/bin/env python3
"""Coordinate a wave of Devin Cloud sessions, each advancing one board item.

Why pre-assignment exists: the repository's advance skill selects its own work
from the execution boards, which is correct for one agent and a collision when
N cloud sessions start at once. The manifest is the coordinator's partitioning
decision: one session, one named board item, one set of owning paths. Sessions
then run the ordinary advance protocol end to end, including landing through
tools/landing.py; the swarm adds nothing to the boards themselves.

What this launcher deliberately does NOT do: it writes no board text, holds no
landing claim, creates no Git refs, and keeps no ownership state. Its receipts
live only in the ignored build/swarm/ directory so worker/session IDs stay off
the boards. Partitioning lives only in the manifest the coordinator wrote;
overlaps with in-flight human work are excluded there, not detected here.

Every subcommand prints one JSON object. Credentials come only from the
DEVIN_API_KEY and DEVIN_ORG_ID environment variables and are never printed.
"""

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.error
import urllib.request

DEFAULT_BASE_URL = "https://api.devin.ai/v3"
BOARDS = ("TASKS.md", "TASKS_BOOTSTRAP.md", "TASKS_OPTIMIZER.md")
MAX_ATTEMPTS = 3
PROMPT_FIELDS = ("name", "wave", "item", "board", "owner_label", "exclusions",
                 "max_acu_limit", "suggested_first_slice_block",
                 "probe_block", "structured_output_schema")

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
    claimed = {}
    for session in sessions:
        where = f"session {session.get('name', '?')!r}"
        require_fields(session, ("name", "board", "item", "host", "owning_paths"), where)
        if session["item"] in exclusions or session["name"] in exclusions:
            raise SwarmError(f"{session['name']}: item {session['item']} is in exclusions.")
        if session["board"] not in BOARDS:
            raise SwarmError(f"{session['name']}: unknown board {session['board']!r}; "
                             f"expected one of {', '.join(BOARDS)}.")
        if session["host"] != "linux":
            raise SwarmError(f"{session['name']}: host must be linux, got {session['host']!r}.")
        if not isinstance(session["owning_paths"], list) or not session["owning_paths"]:
            raise SwarmError(f"{session['name']}: owning_paths must be a non-empty list.")
        if "host_gates" in session:
            gates = session["host_gates"]
            if (not isinstance(gates, list) or not gates
                    or any(not isinstance(command, str) or not command.strip()
                           for command in gates)):
                raise SwarmError(f"{session['name']}: host_gates must be a non-empty "
                                 "list of non-empty strings.")
        if "probe_only" in session and not isinstance(session["probe_only"], bool):
            raise SwarmError(f"{session['name']}: probe_only must be a boolean.")
        for path in session["owning_paths"]:
            if path in claimed:
                raise SwarmError(f"Owning path {path} is claimed by both "
                                 f"{claimed[path]} and {session['name']}.")
            claimed[path] = session["name"]
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


def render_prompt(template, manifest, session):
    owner_label = f"{manifest['owner_label_prefix']}-{session['name']}"
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
    values = {
        "name": session["name"],
        "wave": manifest["wave"],
        "item": session["item"],
        "board": session["board"],
        "owner_label": owner_label,
        "exclusions": ", ".join(manifest["exclusions"]),
        "max_acu_limit": manifest["max_acu_limit"],
        "suggested_first_slice_block": slice_block,
        "probe_block": probe_block,
        "structured_output_schema": json.dumps(STRUCTURED_OUTPUT_SCHEMA, indent=2),
    }
    return template.format_map(values)


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
    gate_results = host_gate_results(repository, manifest["sessions"],
                                     skip=arguments.skip_host_gates)
    skip_route_check = arguments.skip_route_check
    route_data = "skipped" if skip_route_check else route_crates(repository)
    freshness = {
        session["name"]: freshness_probe(repository, session, route_data)
        for session in manifest["sessions"]
    }
    if not skip_route_check:
        route_check(manifest["sessions"], freshness)
    template = (Path(__file__).resolve().parent / "prompt_template.md").read_text(
        encoding="utf-8")
    wave_directory = build_directory(repository, manifest["wave"])
    prompts_directory = wave_directory / "prompts"
    prompts_directory.mkdir(parents=True, exist_ok=True)
    planned = []
    for session in manifest["sessions"]:
        prompt = render_prompt(template, manifest, session)
        unresolved = [field for field in PROMPT_FIELDS
                      if "{" + field + "}" in prompt]
        if unresolved:
            raise SwarmError(f"{session['name']}: prompt template left placeholders "
                             f"unresolved: {unresolved}")
        prompt_path = prompts_directory / f"{session['name']}.md"
        prompt_path.write_text(prompt, encoding="utf-8", newline="\n")
        planned.append({
            "name": session["name"],
            "item": session["item"],
            "board": session["board"],
            "prompt": str(prompt_path.relative_to(repository)),
            "host_gates": gate_results[session["name"]],
            "probe_only": bool(session.get("probe_only", False)),
            "freshness": freshness[session["name"]],
            "body": request_body(manifest, session, prompt),
        })
    emit({"command": "plan", "wave": manifest["wave"], "sessions": planned})
    return 0


def command_launch(arguments, repository):
    manifest = load_manifest(arguments.manifest, repository)
    if not arguments.dry_run:
        host_gate_results(repository, manifest["sessions"],
                          skip=arguments.skip_host_gates)
        route_data = "skipped" if arguments.skip_route_check else route_crates(repository)
        freshness = {
            session["name"]: freshness_probe(repository, session, route_data)
            for session in manifest["sessions"]
        }
        if not arguments.skip_route_check:
            route_check(manifest["sessions"], freshness)
    template = (Path(__file__).resolve().parent / "prompt_template.md").read_text(
        encoding="utf-8")
    wave_directory = build_directory(repository, manifest["wave"])
    receipts = read_receipts(wave_directory)
    organization = ""
    if not arguments.dry_run:
        _, organization = credentials()
    launched = []
    for session in manifest["sessions"]:
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
          "dry_run": bool(arguments.dry_run), "sessions": launched})
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


def command_report(arguments, repository):
    fetched = fetch_sessions(arguments, repository)
    wave_directory = build_directory(repository, arguments.wave)
    wave_directory.mkdir(parents=True, exist_ok=True)
    rows = []
    missing = []
    for entry in fetched:
        receipt, status = entry["receipt"], entry["status"]
        output = status.get("structured_output")
        if not output:
            missing.append(receipt["name"])
            rows.append({"name": receipt["name"],
                         "result": f"no structured output (status: "
                                   f"{status.get('status')})",
                         "acus_consumed": status.get("acus_consumed"),
                         "url": receipt["url"]})
            continue
        rows.append({
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
        })
    lines = [f"# Swarm wave {arguments.wave} report", ""]
    if missing:
        lines.append("Sessions without structured output: "
                     + ", ".join(missing) + ".")
        lines.append("")
    for row in rows:
        lines.append(f"## {row['name']}")
        lines.append(f"- result: {row['result']}")
        lines.append(f"- acus_consumed: {row.get('acus_consumed')}")
        for key in ("commits", "checks", "remaining_dependency",
                    "unrelated_failures", "lease_expiries",
                    "first_build_seconds", "url"):
            if key in row:
                value = row[key]
                if isinstance(value, list):
                    lines.append(f"- {key}:")
                    lines.extend(f"  - {item}" for item in value)
                else:
                    lines.append(f"- {key}: {value}")
        lines.append("")
    report_path = wave_directory / "report.md"
    report_path.write_text("\n".join(lines), encoding="utf-8", newline="\n")
    emit({"command": "report", "wave": arguments.wave,
          "report": str(report_path.relative_to(repository)),
          "sessions_without_structured_output": missing, "sessions": rows})
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
    plan.add_argument("--skip-host-gates", action="store_true")
    plan.add_argument("--skip-route-check", action="store_true")
    launch = subparsers.add_parser("launch")
    launch.add_argument("--manifest", required=True)
    launch.add_argument("--dry-run", action="store_true")
    launch.add_argument("--skip-host-gates", action="store_true")
    launch.add_argument("--skip-route-check", action="store_true")
    launch.add_argument("--relaunch")
    status = subparsers.add_parser("status")
    status.add_argument("--wave", required=True)
    report = subparsers.add_parser("report")
    report.add_argument("--wave", required=True)
    arguments = None
    try:
        arguments = parser.parse_args(argv)
        repository = repository_root(arguments)
        handler = {"plan": command_plan, "launch": command_launch,
                   "status": command_status, "report": command_report}[arguments.command]
        return handler(arguments, repository)
    except (SwarmError, OSError) as error:
        print(f"swarm: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
