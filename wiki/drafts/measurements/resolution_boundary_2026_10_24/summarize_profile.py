import json
import statistics
from pathlib import Path


def median_stage(runs, name):
    samples = []
    for run in runs:
        samples.append(sum(stage["ms"] for stage in run["stages"] if stage["name"] == name))
    return statistics.median(samples) if samples else None


def probe_median(runs, label, kind):
    totals = []
    counts = []
    rows = []
    for run in runs:
        matched = [
            probe
            for probe in run["probes"]
            if probe["label"] == label and probe["kind"] == kind
        ]
        totals.append(sum(probe["ns"] for probe in matched) / 1e6)
        counts.append(len(matched))
        if matched and "rows" in matched[0]:
            rows.append([probe["rows"] for probe in matched])
    return {
        "median_ms": statistics.median(totals),
        "count": statistics.median(counts),
        "rows": rows[0] if rows else [],
    }


def phase_median(runs, name):
    samples = []
    for run in runs:
        samples.append(sum(row["ms"] for row in run["phase_rows"] if row["name"] == name))
    return statistics.median(samples) if samples else None


def report(title, runs):
    print(title, "n", len(runs))
    walls = [run["elapsed_ns"] / 1e6 for run in runs]
    print(" wall", round(statistics.median(walls), 1), "min", round(min(walls), 1), "max", round(max(walls), 1))
    elapsed = median_stage(runs, "total elapsed")
    print(" elapsed", None if elapsed is None else round(elapsed, 1))
    for name in (
        "Stage 03: SyntaxTrees -> SymbolResolvedTrees",
        "Stage 04: SymbolResolvedTrees -> TypedTrees",
        "Stage 05: TypedTrees -> CheckedTrees",
        "Step: build: AssembledSyntax -> CheckedTrees (contains Stage 01-05)",
        "Source ingress: SourceFiles -> SyntaxTrees",
        "Step: assemble: PreparedSources -> AssembledSyntax",
        "Step: settle: CheckedTrees -> SelectedExecution",
        "compile: sources -> requested product",
    ):
        value = median_stage(runs, name)
        if value:
            print(f"  {value:8.1f}  {name}")
    for label, kind in (
        ("syntax_forest", "clone"),
        ("symbol_resolved", "clone"),
        ("prepare", "scope"),
        ("pre_resolution", "scope"),
        ("retain_applied_results", "scope"),
        ("call_result_selection", "scope"),
        ("complete_bodies", "scope"),
        ("seed_body_lowerer", "scope"),
    ):
        row = probe_median(runs, label, kind)
        print(
            f"  probe {label:24} {row['median_ms']:8.3f} ms  n={row['count']} rows={row['rows']}"
        )
    for name in (
        "05",
        "05.d.lower_typed_trees",
        "05.d.3.build_check_facts",
        "05.d.3.b.flow_fixed_point",
        "05.d.3.b.flow_fixed_point.build_flow_facts",
        "05.d.2.a.2.machine.std_or_core",
    ):
        value = phase_median(runs, name)
        if value:
            print(f"  phase {value:8.1f}  {name}")


inplace = json.loads(Path("observations-inplace.json").read_text(encoding="utf-8"))
for workload in (
    "dyn_dispatch_selector_payload_bound",
    "nested_nine_level_record_field_bound",
):
    runs = [
        run
        for run in inplace["runs"]
        if run["role"] == "measured" and run["workload"] == workload and run["returncode"] == 0
    ]
    report(workload, runs)
    print(" native", [run["native_exit"] for run in runs])

length = json.loads(Path("length-observations.json").read_text(encoding="utf-8"))
runs = [run for run in length["runs"] if run["role"] == "measured" and run["returncode"] == 0]
report("length", runs)
print(" native", [run["native_exit"] for run in runs])
print("host", inplace["host"])
print("sha", inplace["omega_sha256"])
print("length sha", length["omega_sha256"])
