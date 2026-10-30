#!/usr/bin/env python3
"""Read existing Omega evidence; export one portable HTML snapshot or serve it locally.

No compiler is launched. Goldens remain historical observations, and fact-file
presence never establishes runtime acceptance. The UI and this collector share
one snapshot format so publishing needs no second progress ledger.
"""
from __future__ import annotations

import argparse
from collections import Counter
from datetime import datetime, timedelta, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import re
from pathlib import Path
import subprocess
import sys
from urllib.parse import urlsplit

import corpus_records
from corpus_scoreboard import native_scoreboard, oracle_lookup
from dashboard_performance import collect_performance

ROOT = Path(__file__).resolve().parents[1]
PLACES = ("local", "field", "parameter", "call_result", "match_join",
          "const_name", "other_state", "other_machine", "target_body")
TARGETS = ("windows_x86_64", "linux_x86_64", "macos_arm64", "linux_arm64")
# Same pairs as corpus_gate.HOST_TARGETS. The filename is the product target;
# a context line names the machine that recorded the run. Emulation makes
# those differ, and only the recorder is host evidence.
RECORDING_HOSTS = {
    ("windows", "amd64"): "windows_x86_64",
    ("windows", "x86_64"): "windows_x86_64",
    ("linux", "x86_64"): "linux_x86_64",
    ("linux", "aarch64"): "linux_arm64",
    ("darwin", "arm64"): "macos_arm64",
    ("darwin", "x86_64"): "macos_x86_64",
}
# Same host-specific September 30 goldens used by the original Claude dashboard.
COHORT_BASELINES = {"windows_x86_64": "5f73e2167f", "linux_x86_64": "03e852fb38",
                    "macos_arm64": "031f4d0ef6"}


def git(repository, *arguments):
    result = subprocess.run(["git", "-C", str(repository), *arguments],
                            capture_output=True, text=True, encoding="utf-8",
                            errors="replace", timeout=30)
    if result.returncode:
        raise ValueError(result.stderr.strip() or "Git read failed")
    return result.stdout.strip()


def instant(value):
    date = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if date.tzinfo is None:
        raise ValueError("Evidence timestamp has no timezone")
    return date


def source_base(repository, revision):
    remote = git(repository, "remote", "get-url", "origin")
    if remote.startswith("git@github.com:"):
        remote = "https://github.com/" + remote.split(":", 1)[1]
    if not remote.startswith("https://github.com/"):
        return ""
    return remote.removesuffix(".git") + "/blob/" + revision + "/"


def shared_evidence_root(repository):
    # sample_native_gate writes every worktree's run records beside the common .git.
    return Path(git(repository, "rev-parse", "--path-format=absolute", "--git-common-dir")).parent


def fact_records(records_dir, warnings):
    latest = {}
    for path in sorted(records_dir.glob("*.json")):
        try:
            record = json.loads(path.read_text(encoding="utf-8-sig"))
            started = instant(record["started"])
            target = record["target"]
            if target not in TARGETS:
                continue
            for result in record["results"]:
                sample = result["sample"].replace("\\", "/").rstrip("/")
                parts = sample.split("/")
                if len(parts) < 5 or parts[:3] != ["samples", "cli", "facts"]:
                    continue
                if parts[4] not in PLACES:
                    continue
                suffix = parts[5:]
                if suffix and suffix != ["invalid"] and not (len(suffix) == 2 and suffix[0] == "controls"):
                    continue
                key = (parts[3], parts[4], target, "/".join(suffix))
                observation = {
                    "state": result["state"], "detail": result.get("detail", ""),
                    "started": record["started"], "revision": record.get("revision", ""),
                    "dirty": bool(record.get("dirty", True)),
                    "stale_cli": bool(record.get("cli", {}).get("stale")),
                    "profile_verified": bool(record.get("compiler_profile")),
                    "record": "build/sample-gate-runs/" + path.name,
                }
                if key not in latest or started > instant(latest[key]["started"]):
                    latest[key] = observation
        except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
            warnings.append(f"Skipped sample record {path.name}: {error}")
    return latest


def collect_facts(repository, records):
    facts = []
    directory = repository / "samples/cli/facts"
    for folder in sorted(directory.iterdir() if directory.is_dir() else []):
        if not folder.is_dir() or folder.name == "build":
            continue
        places = []
        for place in PLACES:
            path = folder / place
            twins = (["invalid"] if (path / "invalid/main.omg").is_file() else [])
            twins += ["controls/" + control.name for control in sorted((path / "controls").glob("*"))
                      if (control / "main.omg").is_file()]
            observations = {}
            for target in TARGETS:
                positive = records.get((folder.name, place, target, ""))
                controls = [{"path": twin, "observation": records.get((folder.name, place, target, twin))}
                            for twin in twins]
                if positive or any(control["observation"] for control in controls):
                    observations[target] = {"positive": positive, "controls": controls}
            places.append({"name": place, "program": (path / "main.omg").is_file(),
                           "twins": twins, "observations": observations})
        facts.append({"name": folder.name, "path": folder.relative_to(repository).as_posix(),
                      "readme": (folder / "README.md").is_file(), "places": places})
    return facts


def execution_target(path):
    match = re.fullmatch(r"corpus_(?:native|both)_([a-z0-9_]+)\.txt", Path(path).name)
    if match is None or match.group(1) not in TARGETS:
        return None
    return match.group(1)


def recorded_hosts(text):
    """Hosts named by `# context:` lines, or None when this golden has none.

    An unreadable or unrecognized context is an empty set: the line is
    present, and it does not name one of the catalogued hosts.
    """
    hosts = set()
    saw = False
    for line in text.splitlines():
        if not line.startswith(corpus_records.CONTEXT_PREFIX):
            continue
        saw = True
        try:
            context = json.loads(line[len(corpus_records.CONTEXT_PREFIX):])
        except ValueError:
            return set()
        if not isinstance(context, dict):
            return set()
        host = RECORDING_HOSTS.get((str(context.get("platform") or "").lower(),
                                    str(context.get("machine") or "").lower()))
        if host is None:
            return set()
        hosts.add(host)
    return hosts if saw else None


def evidence_kind(path, text):
    """How `text` counts for the host named by `path`: host, emulated, or None.

    A golden with no context line predates host stamping and stays host
    evidence. A `linux_arm64` product recorded on Linux x86_64 is the
    qemu-aarch64 run of that product. Any other recorder mismatch is not
    evidence for the filename host.
    """
    target = execution_target(path)
    if target is None:
        return "host"
    hosts = recorded_hosts(text)
    if hosts is None or hosts == {target}:
        return "host"
    if target == "linux_arm64" and hosts == {"linux_x86_64"}:
        return "emulated"
    return None


def choose_execution_sources(classified):
    """Prefer a matching-host golden. Emulated rows remain when they are the only evidence."""
    host_sources = [(path, records) for kind, path, records in classified if kind == "host"]
    if host_sources:
        return host_sources, False
    emulated = [(path, records) for kind, path, records in classified if kind == "emulated"]
    return emulated, bool(emulated)


def execution_revision_dates(repository):
    return {revision: instant(date) for revision, date in
            (line.split(" ", 1) for line in git(repository, "log", "--exclude=refs/coordination/*", "--all", "--format=%H %cI").splitlines())}


def merge_execution_records(repository, sources, dates):
    """One observation per fixture; file updates never restamp carried rows."""
    latest = {}
    for path, records in sources:
        for row in records:
            stamp = (row.get("recorded_at") or "").removesuffix("+wt")
            if stamp not in dates:
                matches = [date for revision, date in dates.items()
                           if len(revision) == 40 and revision.startswith(stamp)] \
                    if re.fullmatch(r"[0-9a-f]{7,40}", stamp) else []
                dates[stamp] = matches[0] if len(matches) == 1 else datetime.min.replace(tzinfo=timezone.utc)
            priority = (dates[stamp], "corpus_both_" in path)
            previous = latest.get(row["fixture"])
            if previous is None or priority > previous[0]:
                latest[row["fixture"]] = (priority, dict(row, golden_path=path))
    return [entry[1] for entry in latest.values()]


def collect_native(repository, warnings):
    hosts = []
    dates = execution_revision_dates(repository)
    for target in TARGETS:
        relative = f"tests/omega/corpus_native_{target}.txt"
        paths = [f"tests/omega/corpus_{mode}_{target}.txt" for mode in ("native", "both")
                 if (repository / f"tests/omega/corpus_{mode}_{target}.txt").is_file()]
        host = {"target": target, "path": relative, "available": bool(paths),
                "golden_paths": paths, "emulated": False}
        if host["available"]:
            try:
                classified = []
                for path in paths:
                    text = (repository / path).read_text(encoding="utf-8")
                    kind = evidence_kind(path, text)
                    if kind is None:
                        recorded = recorded_hosts(text)
                        where = ", ".join(sorted(recorded)) if recorded else "an unrecognized host"
                        warnings.append(
                            f"Skipped {path}: recorded on {where}, so it is not {target} host evidence")
                        continue
                    classified.append((kind, path, corpus_records.parse(text)))
                sources, emulated = choose_execution_sources(classified)
                if not sources:
                    host.update(available=False, golden_paths=[])
                    hosts.append(host)
                    continue
                host["emulated"] = emulated
                records = [row for row in merge_execution_records(repository, sources, dates)
                           if row["tier"] in ("pass", "run")]
                golden = repository / sources[0][0]
                host["path"] = sources[0][0]
                host["golden_paths"] = [path for path, _ in sources]
                host["counts"] = native_scoreboard(records, oracle_lookup(repository, golden, None, target))
                host["stamps"] = dict(Counter(row.get("recorded_at") or "unstamped" for row in records))
                host["source_counts"] = dict(Counter(row["golden_path"] for row in records))
                host["golden_updated"] = git(repository, "log", "-1", "--format=%cI", "--",
                                             *host["golden_paths"])
                host["cohort"] = collect_cohort(repository, target, records)
                if target in COHORT_BASELINES and not host["cohort"]["available"]:
                    warnings.append(f"Cohort baseline unavailable for {target}: {host['cohort']['error']}")
            except (OSError, ValueError) as error:
                host.update(available=False, error=str(error))
                warnings.append(f"Cannot read native golden {target}: {error}")
        elif target in COHORT_BASELINES:
            warnings.append(f"Native golden missing for {target}; cohort comparison unavailable")
        hosts.append(host)
    return hosts


def cohort_comparison(baseline, current):
    """Freeze fixture IDs; missing rows stay in the denominator, not in regressions."""
    def indexed(records):
        rows = {}
        for row in records:
            if row["tier"] not in ("pass", "run"):
                continue
            if row["fixture"] in rows:
                raise ValueError(f"Duplicate cohort fixture: {row['fixture']}")
            rows[row["fixture"]] = row
        return rows
    def ran(row):
        return row is not None and row["status"] != "unimplemented" and "exit" in row["facts"]
    old, now = indexed(baseline), indexed(current)
    if not old:
        raise ValueError("Baseline has no valid fixture rows")
    missing = old.keys() - now.keys()
    added = now.keys() - old.keys()
    return {
        "baseline_total": len(old), "baseline_runs": sum(ran(row) for row in old.values()),
        "same_now_runs": sum(ran(now.get(key)) for key in old),
        "current_total": len(now), "current_runs": sum(ran(row) for row in now.values()),
        "recovered": sum(not ran(row) and ran(now.get(key)) for key, row in old.items()),
        "lost": sum(ran(old[key]) and not ran(now[key]) for key in old.keys() & now.keys()),
        "missing": len(missing), "missing_previously_running": sum(ran(old[key]) for key in missing),
        "new_running": sum(ran(now[key]) for key in added), "new_total": len(added),
    }


def collect_cohort(repository, target, current):
    baseline = COHORT_BASELINES.get(target)
    if baseline is None:
        return {"available": False, "error": "No September 30 baseline for this host"}
    try:
        revision = git(repository, "rev-parse", baseline + "^{commit}")
        git(repository, "merge-base", "--is-ancestor", revision, "HEAD")
        path = f"tests/omega/corpus_native_{target}.txt"
        rows = corpus_records.parse(git(repository, "show", f"{revision}:{path}"))
        return dict(cohort_comparison(rows, current), available=True, baseline_revision=revision,
                    baseline_date=git(repository, "show", "-s", "--format=%cI", revision))
    except (OSError, ValueError) as error:
        return {"available": False, "error": str(error)}


def native_history(repository, relative, warnings=None):
    """Last committed golden per recorded day; never invent observations in gaps."""
    paths = [relative] if isinstance(relative, str) else relative
    history = []
    dates = execution_revision_dates(repository)
    anchor = instant(git(repository, "show", "-s", "--format=%cI", "HEAD"))
    since = (anchor - timedelta(days=30)).isoformat()
    commits = git(repository, "log", f"--since={since}", "--format=%H %cI", "--", *paths)
    days = {}
    for line in commits.splitlines():
        revision, committed = line.split(" ", 1)
        day = instant(committed).astimezone(timezone.utc).date()
        if day not in days or instant(committed) > instant(days[day][1]):
            days[day] = (revision, committed)
    for revision, committed in sorted(days.values(), key=lambda entry: instant(entry[1])):
        sources = []
        malformed = False
        for path in paths:
            try:
                text = git(repository, "show", f"{revision}:{path}")
            except (OSError, ValueError) as error:
                if "does not exist in" in str(error) or "exists on disk, but not in" in str(error):
                    continue
                malformed = True
                if warnings is not None:
                    warnings.append(f"Skipped historical golden {path} at {revision[:11]}: {error}")
                continue
            kind = evidence_kind(path, text)
            if kind is None:
                continue
            try:
                sources.append((kind, path, corpus_records.parse(text)))
            except (OSError, ValueError) as error:
                malformed = True
                if warnings is not None:
                    warnings.append(f"Skipped historical golden {path} at {revision[:11]}: {error}")
        chosen, emulated = choose_execution_sources(sources)
        if malformed or not chosen:
            continue
        records = merge_execution_records(repository, chosen, dates)
        valid = [row for row in records if row["tier"] in ("pass", "run")]
        counts = native_scoreboard(valid)
        history.append({"revision": revision, "date": committed, "paths": [path for path, _ in chosen],
                        "emulated": emulated, "executed": counts["executed"], "fixtures": counts["fixtures"]})
    return history


def collect_claims(repository):
    try:
        command = [sys.executable, str(repository / "tools/claims.py"),
                   "--repository", str(repository), "--timeout-seconds", "15", "status"]
        result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8",
                                errors="replace", timeout=20)
        if result.returncode:
            return {"available": False, "error": "Coordination registry unavailable; active work is unknown."}
        record = json.loads(result.stdout)
        allowed = ("ticket", "item", "owner", "board", "paths", "note", "claimed_utc", "expires_utc")
        def entries(key):
            rows = record.get(key, [])
            for row in rows:
                instant(row["expires_utc"])
            return [{key: row.get(key) for key in allowed} for row in rows]
        return {"available": True, "version": record["version"],
                "active": entries("claims"), "expired": entries("expired")}
    except (OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired):
        return {"available": False, "error": "Coordination registry unavailable; active work is unknown."}


def collect(repository, evidence_root):
    revision = git(repository, "rev-parse", "HEAD")
    warnings = []
    records = fact_records(evidence_root / "build/sample-gate-runs", warnings)
    data = {"schema": "omega-dashboard-v1", "revision": revision,
            "generated": datetime.now(timezone.utc).isoformat(timespec="seconds"),
            "source_base": source_base(repository, revision),
            "dirty": bool(git(repository, "status", "--porcelain", "--untracked-files=no")),
            "places": PLACES, "targets": TARGETS,
            "facts": collect_facts(repository, records),
            "native": collect_native(repository, warnings),
            "claims": collect_claims(repository), "warnings": warnings,
            "performance": collect_performance(repository, warnings)}
    for host in data["native"]:
        host["history"] = []
        if host["available"]:
            try:
                host["history"] = native_history(repository, host["golden_paths"], warnings)
            except (OSError, ValueError) as error:
                warnings.append(f"Cannot read native history {host['target']}: {error}")
    if git(repository, "rev-parse", "HEAD") != revision:
        raise ValueError("Checkout revision changed during collection; refresh again.")
    return data


def render(data, live=False):
    template = (ROOT / "tools/dashboard/index.html").read_text(encoding="utf-8")
    payload = json.dumps(data, ensure_ascii=False).replace("<", "\\u003c").replace("\u2028", "\\u2028").replace("\u2029", "\\u2029")
    return template.replace("/*__SNAPSHOT__*/", payload).replace("/*__LIVE__*/false", str(live).lower())


def serve(repository, evidence_root, port):
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            path = urlsplit(self.path).path
            if path not in ("/", "/index.html", "/api/snapshot"):
                self.send_error(404)
                return
            try:
                data = collect(repository, evidence_root)
                body = (json.dumps(data) if path == "/api/snapshot" else render(data, live=True)).encode("utf-8")
                self.send_response(200)
                self.send_header("Content-Type", "application/json; charset=utf-8" if path == "/api/snapshot" else "text/html; charset=utf-8")
                self.send_header("Cache-Control", "no-store")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
            except (OSError, ValueError, subprocess.TimeoutExpired) as error:
                self.send_error(503, "Evidence refresh failed")
                print(f"Dashboard refresh: {error}", file=sys.stderr)
    server = ThreadingHTTPServer(("127.0.0.1", port), Handler)
    print(f"Omega dashboard: http://127.0.0.1:{server.server_port}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--repository", type=Path, default=ROOT)
    parser.add_argument("--evidence-root", type=Path, help="checkout holding ignored build/sample-gate-runs records")
    parser.add_argument("--output", type=Path, default=ROOT / "build/compiler-dashboard/index.html")
    parser.add_argument("--serve", action="store_true", help="serve a read-only local dashboard with Refresh evidence")
    parser.add_argument("--port", type=int, default=8765)
    args = parser.parse_args()
    repository = args.repository.resolve()
    try:
        evidence_root = (args.evidence_root or shared_evidence_root(repository)).resolve()
        if args.serve:
            serve(repository, evidence_root, args.port)
        else:
            html = render(collect(repository, evidence_root))
            args.output.parent.mkdir(parents=True, exist_ok=True)
            temporary = args.output.with_suffix(".tmp")
            temporary.write_text(html, encoding="utf-8")
            temporary.replace(args.output)
            print(args.output.resolve())
        return 0
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        print(f"Dashboard: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
