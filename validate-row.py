import hashlib, json, os, subprocess, time
from pathlib import Path
root = Path.cwd()
producer = json.loads((root / "build/call-rank/final-producer/producer.json").read_text())
cli = producer["executable"]
assert hashlib.sha256(Path(cli).read_bytes()).hexdigest() == producer["sha256"]
base = Path("samples/cli/facts/nested_machine_invocation/joint_rank")
places = ["local", "field", "parameter", "call_result", "match_join", "const_name", "other_state", "other_machine", "target_body"]
rows = []
for place in places:
    source = base / place / "computed_measure/call"
    rows += [(source / "main.omg", None), (source / "invalid/main.omg", "a preserving cycle has no strict measure DECREASE")]
rows += [(base / "parameter/computed_measure/call/probe/main.omg", None), (base / "parameter/computed_measure/call/probe/composition/main.omg", None)]
for name, reason in [("precondition", "cannot prove rank range"), ("self_establishment", "cannot prove rank range"), ("crash_ceiling", "selected closure publishes a crash route"), ("nonterminating", "selected machine is not unconditionally terminating")]:
    rows.append((base / "controls" / ("declared_call_" + name) / "main.omg", reason))
results = []
for source, reason in rows:
    command = [cli, *(["--check"] if reason else ["run", "--both"]), str(source)]
    started = time.monotonic()
    observed = subprocess.run(command, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=120, env=dict(os.environ, OMEGA_LIBRARY_ROOT=str(root / "source/library")))
    result = dict(source=str(source), source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(), command=command, exit=observed.returncode, stdout=observed.stdout, stderr=observed.stderr, seconds=time.monotonic()-started, expected_rejection=reason)
    results.append(result)
    print(json.dumps(result), flush=True)
    if reason:
        assert observed.returncode == 1 and reason in observed.stderr, result
    else:
        assert observed.returncode == 0 and "native exit 0" in observed.stderr and "interp exit: 0" in observed.stderr, result
assert len(results) == 24
(root / "build/call-rank/final-row.json").write_text(json.dumps(dict(producer=producer, results=results), indent=2) + "\n")
print("11 positive programs: native0 and Terminal0; 13 fact-sensitive controls rejected")
