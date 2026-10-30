"""Interleave the retained and private-slot diagnostic producers on one witness."""

import json
from pathlib import Path
import subprocess
import sys

producers = dict(zip("AB", (Path(value).resolve() for value in sys.argv[1:3])))
output = Path(sys.argv[3]).resolve()
output.mkdir(parents=True, exist_ok=False)
runner = Path(__file__).with_name("profile.py")
records = []
for index, variant in enumerate("ABBAAB"):
    directory = output / f"{index:02}-{variant}"
    with (output / f"{index:02}-{variant}.log").open("w", encoding="utf-8") as log:
        process = subprocess.run(
            [sys.executable, str(runner), str(producers[variant]), str(directory)],
            stdout=log, stderr=subprocess.STDOUT, timeout=180,
        )
    assert process.returncode == 0, f"failed observation: {directory}"
    record = json.loads((directory / "result.json").read_text(encoding="utf-8"))
    record.update(index=index, variant=variant)
    records.append(record)
    (output / "runs.json").write_text(json.dumps(records, indent=2), encoding="utf-8")
    print(f"{index} {variant}: {record['wall_ms']:.1f} ms, native exit {record['native_exit']}", flush=True)
for variant in "AB":
    rows = [record for record in records if record["variant"] == variant]
    assert len({row["artifact_sha256"] for row in rows}) == 1
    assert len({row["producer_sha256"] for row in rows}) == 1
assert len({row["source_sha256"] for row in records}) == 1
assert len({row["build_sha256"] for row in records}) == 1
