"""Compare saved correspondence drivers; run under one mbx CPU reservation.

psutil is optional. With it, peak_rss_bytes is a sampled process working set,
including parsing/setup, and must not be read as the validator's heap size.
"""
import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import statistics
import subprocess
import tempfile
import time

try:
    import psutil
except ImportError:
    psutil = None


def run(binary, extra):
    with tempfile.TemporaryFile(mode="w+t", encoding="utf-8") as output, tempfile.TemporaryFile(
        mode="w+t", encoding="utf-8"
    ) as errors:
        process = subprocess.Popen(
            [str(binary), "--samples", "1", *extra],
            stdin=subprocess.DEVNULL, stdout=output, stderr=errors,
        )
        peak = None
        if psutil is not None:
            observed = psutil.Process(process.pid)
            while process.poll() is None:
                try:
                    peak = max(peak or 0, observed.memory_info().rss)
                except psutil.NoSuchProcess:
                    break
                time.sleep(0.005)
        status = process.wait()
        output.seek(0)
        errors.seek(0)
        if status:
            raise RuntimeError(f"{binary} exited {status}: {errors.read()[-8000:]}")
        rows = list(csv.DictReader(io.StringIO(output.read())))
        if not rows:
            raise RuntimeError(f"{binary} emitted no measurements")
        return {"rows": rows, "peak_rss_bytes": peak}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--before", required=True, type=Path)
    parser.add_argument("--after", required=True, type=Path)
    parser.add_argument("--allocations-before", required=True, type=Path)
    parser.add_argument("--allocations-after", required=True, type=Path)
    parser.add_argument("--pairs", default=9, type=int)
    parser.add_argument("--output", required=True, type=Path)
    arguments = parser.parse_args()
    if arguments.pairs < 1:
        parser.error("--pairs must be positive")
    binaries = {
        name: getattr(arguments, name).resolve()
        for name in ("before", "after", "allocations_before", "allocations_after")
    }
    runs = []
    for pair in range(arguments.pairs):
        # Alternate which revision goes first; keep each pair adjacent.
        for revision in (("before", "after") if pair % 2 == 0 else ("after", "before")):
            runs.append({"pair": pair, "revision": revision, **run(binaries[revision], [])})
    allocations = {
        revision: run(binaries[f"allocations_{revision}"], ["--iterations", "1"])
        for revision in ("before", "after")
    }
    labels = [(row["workload"], row["methods"]) for row in runs[0]["rows"]]
    for observed in [*runs, *allocations.values()]:
        if [(row["workload"], row["methods"]) for row in observed["rows"]] != labels:
            raise RuntimeError("the revisions did not measure the same declaration workloads")
    summary = []
    for label, methods in labels:
        timings = {
            revision: [
                float(row["ns_per_validation"])
                for observed in runs if observed["revision"] == revision
                for row in observed["rows"] if row["workload"] == label
            ]
            for revision in ("before", "after")
        }
        row = {"workload": label, "methods": int(methods)}
        for revision in ("before", "after"):
            row[f"{revision}_median_ns"] = statistics.median(timings[revision])
            counters = next(item for item in allocations[revision]["rows"] if item["workload"] == label)
            row[f"{revision}_allocations"] = int(float(counters["allocations_per_validation"]))
            row[f"{revision}_requested_bytes"] = int(float(counters["requested_bytes_per_validation"]))
            row[f"{revision}_peak_extra_live_bytes"] = int(counters["peak_extra_live_bytes"])
        row["median_paired_before_over_after"] = statistics.median(
            before / after for before, after in zip(timings["before"], timings["after"])
        )
        summary.append(row)
    def sha256(path):
        with path.open("rb") as binary:
            return hashlib.file_digest(binary, "sha256").hexdigest()

    receipt = {
        "binaries": {
            revision: {"path": str(path), "sha256": sha256(path)}
            for revision, path in binaries.items()
        },
        "pairs": arguments.pairs, "rss_sampled": psutil is not None,
        "runs": runs, "allocations": allocations, "summary": summary,
    }
    arguments.output.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
