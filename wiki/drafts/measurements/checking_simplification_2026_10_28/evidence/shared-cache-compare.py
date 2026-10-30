"""Compare two exact producers on unchanged source with the existing benchmark tools.

Usage: python <this file> <baseline omega> <candidate omega> <new output directory>
"""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

REPOSITORY = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPOSITORY / "tools/benchmark"))
import benchmark

SUBJECTS = {
    "stack32": "thirtytwo_arg_stack_calling_plan/other_machine",
    "length": "length/local",
    "dispatch": "dyn_dispatch_selector_payload_bound/match_join",
}


def save(path, value):
    path.write_text(json.dumps(value, indent=2), encoding="utf-8")


def main():
    producers = dict(zip("AB", (Path(value).resolve() for value in sys.argv[1:3])))
    output = Path(sys.argv[3]).resolve()
    output.mkdir(parents=True, exist_ok=False)
    os.environ.pop("OMEGA_PHASE_PROFILE", None)
    metadata = {
        "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPOSITORY, text=True).strip(),
        "producers": {key: {"path": str(path), "sha256": benchmark.sha256_file(path),
                            "profile": benchmark.require_optimized(path)} for key, path in producers.items()},
        "cpu": benchmark.cpu_name(), "rustc": benchmark.rustc_version(),
        "target": "windows_x86_64", "sources": {},
    }
    (output / "compiler.patch").write_bytes(subprocess.check_output(["git", "diff", "--", "omega-rust"], cwd=REPOSITORY))
    records = []
    for name, relative in SUBJECTS.items():
        source = REPOSITORY / "samples/cli/facts" / relative / "main.omg"
        metadata["sources"][name] = {"path": str(source), "sha256": benchmark.sha256_file(source),
                                     "build_sha256": benchmark.sha256_file(source.with_name("build.omg"))}
        save(output / "metadata.json", metadata)
        for index, variant in enumerate("ABABBAABBAAB"):
            kind = "warmup" if index < 2 else "profile" if index >= 10 else "measured"
            directory = output / f"{name}-{index:02}-{variant}-{kind}"
            directory.mkdir()
            project = directory / "input"
            project.mkdir()
            for filename in ["main.omg", "build.omg"]:
                shutil.copyfile(source.with_name(filename), project / filename)
            benchmark.settle_package_review(producers[variant], project, "windows_x86_64")
            if kind == "profile":
                os.environ["OMEGA_PHASE_PROFILE"] = "1"
            arguments = [str(producers[variant]), "--timings", "--target", "windows_x86_64",
                         "--build-dir", str(directory / "build"), str(project / "main.omg")]
            code, wall, memory, transcript = benchmark.capture_compile(arguments, REPOSITORY)
            (directory / "output.txt").write_text(transcript, encoding="utf-8")
            stages, published = benchmark.parse_timings(transcript)
            row = dict(subject=name, index=index, variant=variant, kind=kind, command=arguments,
                       exit=code, wall_ms=wall, peak_job_memory_bytes=memory, stages=stages)
            records.append(row)
            save(output / "runs.json", records)
            if code != 0 or not published or not Path(published).is_file():
                raise RuntimeError(f"invalid compile observation: {row}\n{transcript[-3000:]}")
            assert stages["Stage 05: TypedTrees -> CheckedTrees"] > 100, "checking appears reused; investigate before timing"
            artifact = Path(published)
            execution = subprocess.run([str(artifact)], cwd=REPOSITORY, capture_output=True, timeout=30)
            row.update(artifact_sha256=benchmark.sha256_file(artifact), artifact_bytes=artifact.stat().st_size,
                       native_exit=execution.returncode, native_stdout_hex=execution.stdout.hex(),
                       native_stderr_hex=execution.stderr.hex())
            save(output / "runs.json", records)
            assert execution.returncode == 0, row
            print(f"{name} {kind} {variant}: {wall:.1f} ms; native exit 0", flush=True)
        os.environ.pop("OMEGA_PHASE_PROFILE", None)
        assert len({row["artifact_sha256"] for row in records if row["subject"] == name}) == 1
        assert benchmark.sha256_file(source) == metadata["sources"][name]["sha256"]
        assert benchmark.sha256_file(source.with_name("build.omg")) == metadata["sources"][name]["build_sha256"]
    for variant, producer in producers.items():
        assert benchmark.sha256_file(producer) == metadata["producers"][variant]["sha256"]


if __name__ == "__main__":
    main()
