"""Check the unchanged stack-call fact row and focused arithmetic controls."""

import json
import os
from pathlib import Path
import subprocess
import sys

from compare import REPOSITORY, benchmark, save

PLACES = ["local", "field", "parameter", "call_result", "match_join", "const_name",
          "other_state", "other_machine", "target_body"]
CONTROLS = [
    "pass/arithmetic/checked_flag_proof_wrapping_cast_fits",
    "pass/arithmetic/checked_flag_proof_saturating_cast_fits",
    "pass/arithmetic/runtime_short_circuit_cast_decisions_exit",
    "pass/machines/private_trapping_proven_safe_exit",
    "pass/machines/private_division_wrapping_pair_exit",
    "pass/machines/private_division_saturating_pair_exit",
    "fail/arithmetic/checked_flag_proof_wrapping_cast_overflow",
    "fail/arithmetic/checked_flag_proof_saturating_cast_overflow",
    "fail/arithmetic/short_circuit_cast_wrong_path",
    "fail/arithmetic/short_circuit_cast_mutated_subject",
    "fail/arithmetic/wrapping_guard_does_not_bound_original",
    "fail/arithmetic/implicit_integer_policy_erasure_rejected",
]


def main():
    producers = dict(zip("AB", (Path(value).resolve() for value in sys.argv[1:3])))
    output = Path(sys.argv[3]).resolve()
    resume = "--resume" in sys.argv[4:]
    output.mkdir(parents=True, exist_ok=resume)
    identities = {key: {"sha256": benchmark.sha256_file(path),
         "profile": benchmark.require_optimized(path)} for key, path in producers.items()}
    if resume:
        assert json.loads((output / "producers.json").read_text(encoding="utf-8")) == identities
    else:
        save(output / "producers.json", identities)
    cases = []
    fact = REPOSITORY / "samples/cli/facts/thirtytwo_arg_stack_calling_plan"
    for place in PLACES:
        cases.extend([(place, fact / place / "main.omg", False, True),
                      (place + "-invalid", fact / place / "invalid/main.omg", True, False)])
    for control in CONTROLS:
        cases.append((control.replace("/", "-"), REPOSITORY / "tests/omega" / control / "main.omg",
                      control.startswith("fail/"), False))
    records = json.loads((output / "results.json").read_text(encoding="utf-8")) if resume else []
    for name, source, invalid, runtime in cases:
        case_rows = []
        for variant, producer in producers.items():
            previous = next((row for row in records if row["case"] == name and row["variant"] == variant), None)
            if previous is not None:
                assert previous["passed"] and previous["source_sha256"] == benchmark.sha256_file(source)
                if previous["build_sha256"] is not None:
                    assert previous["build_sha256"] == benchmark.sha256_file(source.with_name("build.omg"))
                case_rows.append(previous)
                continue
            cache = output / "cache" / variant
            cache.mkdir(parents=True, exist_ok=True)
            os.environ["LOCALAPPDATA"] = str(cache)
            # Invalid controls must reach their authored checking refusal;
            # package review cannot succeed for an intentionally invalid root.
            if not invalid and source.with_name("build.omg").is_file():
                benchmark.settle_package_review(producer, source.parent, "windows_x86_64")
            arguments = [str(producer)] + (["run", "--both"] if runtime else ["--check", "--target", "windows_x86_64"])
            arguments.append(str(source))
            result = subprocess.run(arguments, cwd=REPOSITORY, stdin=subprocess.DEVNULL,
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=180)
            transcript = result.stdout.decode("utf-8", "replace")
            errors = [line.strip() for line in transcript.splitlines() if "error:" in line]
            passed = (result.returncode != 0 and bool(errors)) if invalid else result.returncode == 0
            if runtime:
                passed = passed and "native exit 0" in transcript and "interp exit: 0" in transcript
            if invalid and name.endswith("-invalid"):
                passed = passed and "exact arithmetic" in transcript and "may overflow `u64`" in transcript
            records.append(dict(case=name, variant=variant, command=arguments, exit=result.returncode,
                                passed=passed, errors=errors, output=transcript,
                                source_sha256=benchmark.sha256_file(source),
                                build_sha256=(benchmark.sha256_file(source.with_name("build.omg"))
                                              if source.with_name("build.omg").is_file() else None)))
            case_rows.append(records[-1])
            save(output / "results.json", records)
            print(f"{name} {variant}: {'PASS' if passed else 'FAIL'}", flush=True)
        assert case_rows[0]["exit"] == case_rows[1]["exit"], name
        assert case_rows[0]["errors"] == case_rows[1]["errors"], name
    assert all(row["passed"] for row in records), "see results.json"


if __name__ == "__main__":
    main()
