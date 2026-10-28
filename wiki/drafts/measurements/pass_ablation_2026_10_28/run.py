"""Replay this bounded Windows experiment from its frozen repository root.

python wiki/drafts/measurements/pass_ablation_2026_10_28/run.py <omega.exe> <new-output-directory>
The existing benchmark module owns profile verification and receipt parsing.
"""

import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import time

REPOSITORY = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPOSITORY / "tools" / "benchmark"))
import benchmark

SUBJECTS = {
    "length": "length/local",
    "dispatch": "dyn_dispatch_selector_payload_bound/local",
    "stack32": "thirtytwo_arg_stack_calling_plan/other_machine",
    "control": "length/local",
}
ENABLE = "    Optimizations::enable(&mut builder.optimizations, Optimization::ControlFlowCleanup);\n"


def require_success(code, output, artifact):
    if code != 0 or artifact is None or not artifact.is_file():
        raise RuntimeError(f"invalid compile observation (exit {code}): {output[-4000:]}")


def capture(arguments, directory, profile=False):
    environment = dict(os.environ)
    environment.pop("OMEGA_PHASE_PROFILE", None)
    if profile:
        environment["OMEGA_PHASE_PROFILE"] = "1"
    started = time.perf_counter()
    completed = subprocess.run(
        arguments, cwd=REPOSITORY, env=environment, stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=120,
    )
    elapsed = (time.perf_counter() - started) * 1000
    output = completed.stdout.decode("utf-8", "replace")
    (directory / "output.txt").write_text(output, encoding="utf-8")
    return completed.returncode, elapsed, output


def main():
    omega = Path(sys.argv[1]).resolve()
    output = Path(sys.argv[2]).resolve()
    output.mkdir(parents=True, exist_ok=False)
    profile = benchmark.require_optimized(omega)
    producer_hash = benchmark.sha256_file(omega)
    metadata = {
        "source_revision": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=REPOSITORY, text=True).strip(),
        "producer": str(omega), "producer_sha256": producer_hash,
        "profile": profile, "platform": platform.platform(),
        "processor": benchmark.cpu_name(), "rustc": benchmark.rustc_version(),
        "target": "windows_x86_64", "sources": {},
    }
    compiler_patch = subprocess.check_output(["git", "diff", "--", "omega-rust"], cwd=REPOSITORY)
    metadata["checkout_compiler_patch_sha256"] = hashlib.sha256(compiler_patch).hexdigest()
    (output / "compiler.patch").write_bytes(compiler_patch)
    records = []
    for name, relative in SUBJECTS.items():
        root = REPOSITORY / "samples/cli/facts" / relative / "main.omg"
        inputs = output / "inputs" / name
        inputs.mkdir(parents=True)
        shutil.copyfile(root, inputs / "main.omg")
        build = root.with_name("build.omg").read_text(encoding="utf-8")
        build = build.replace(ENABLE, "")
        assert "Optimization::" not in build
        if name != "control":
            prefix, suffix = build.rsplit("}", 1)
            build = prefix + ENABLE + "}" + suffix
        (inputs / "build.omg").write_text(build, encoding="utf-8", newline="\n")
        metadata["sources"][name] = {
            "root": str(root.relative_to(REPOSITORY)),
            "main_sha256": benchmark.sha256_file(root),
            "build_sha256": benchmark.sha256_file(root.with_name("build.omg")),
            "experiment_build_sha256": benchmark.sha256_file(inputs / "build.omg"),
        }
    (output / "metadata.json").write_text(json.dumps(metadata, indent=2), encoding="utf-8")

    for name, relative in SUBJECTS.items():
        root = output / "inputs" / name / "main.omg"
        if not root.with_name("omega.lock").is_file():
            benchmark.settle_package_review(omega, root.parent, "windows_x86_64")
        # Two warmups, four measured observations per variant, separate profiles.
        for index, variant in enumerate("ABABBAABBAAB"):
            kind = "warmup" if index < 2 else "profile" if index >= 10 else "measured"
            directory = output / f"{name}-{index:02}-{variant}-{kind}"
            directory.mkdir()
            arguments = [str(omega), "--timings", "--build-dir", str(directory / "build"),
                         "--target", "windows_x86_64", "--disable-optimization", "CopyPropagation"]
            if variant == "B":
                arguments += ["--disable-optimization", "ControlFlowCleanup"]
            arguments.append(str(root))
            code, wall, transcript = capture(arguments, directory, kind == "profile")
            stages, published = benchmark.parse_timings(transcript)
            artifact = Path(published) if published else None
            row = dict(subject=name, index=index, variant=variant, kind=kind,
                       command=arguments, exit=code, wall_ms=wall, stages=stages,
                       receipt=benchmark.parse_rollback_receipt(transcript))
            records.append(row)
            (output / "runs.json").write_text(json.dumps(records, indent=2), encoding="utf-8")
            require_success(code, transcript, artifact)
            selected = [] if name == "control" else ["ControlFlowCleanup"]
            expected_applied = selected if variant == "B" else []
            expected_effective = [] if variant == "B" else selected
            receipt = row["receipt"]
            assert receipt is not None, "missing observed selection"
            assert receipt["applied"] == expected_applied, receipt
            assert receipt["effective"] == expected_effective, receipt
            row["artifact_sha256"] = benchmark.sha256_file(artifact)
            row["artifact_bytes"] = artifact.stat().st_size
            execution = subprocess.run([str(artifact)], cwd=REPOSITORY, stdin=subprocess.DEVNULL,
                                       capture_output=True, timeout=30)
            row["native_exit"] = execution.returncode
            row["native_stdout_hex"] = execution.stdout.hex()
            row["native_stderr_hex"] = execution.stderr.hex()
            (output / "runs.json").write_text(json.dumps(records, indent=2), encoding="utf-8")
            assert execution.returncode == 0, row
            print(f"{name} {kind} {variant}: {wall:.1f} ms, native exit 0", flush=True)
    assert benchmark.sha256_file(omega) == producer_hash, "producer changed during experiment"


if __name__ == "__main__":
    # Guard the experiment's false-success boundary before measuring anything.
    try:
        require_success(1, "deliberate failed compiler", None)
    except RuntimeError:
        pass
    else:
        raise AssertionError("failed compile was accepted")
    try:
        require_success(0, "deliberately missing output", None)
    except RuntimeError:
        pass
    else:
        raise AssertionError("missing artifact was accepted")
    main()
