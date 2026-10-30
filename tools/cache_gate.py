#!/usr/bin/env python3
"""Compare real warm/fresh compiler results after source and cache mutations.

Only disposable projects and child-process cache directories are changed. Every
accepted observation emits a fresh native image and executes its authored oracle.
The compiler-switch check requires a second, independently built producer.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import tempfile
import time

from benchmark.benchmark import REVIEW_LINE, accept_pending_decisions
from compiler_profile import require_optimized

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tests/omega/pass/build/executed_project_identity_exit"


def host_target():
    architecture = platform.machine().lower()
    if os.name == "nt" and architecture in {"amd64", "x86_64"}:
        return "windows_x86_64"
    if platform.system() == "Darwin" and architecture in {"arm64", "aarch64"}:
        return "macos_arm64"
    if platform.system() == "Linux" and architecture in {"amd64", "x86_64", "arm64", "aarch64"}:
        return "linux_arm64" if architecture in {"arm64", "aarch64"} else "linux_x86_64"
    raise ValueError("cache gate needs a supported native host")


def cache_environment(directory):
    environment = dict(os.environ, OMEGA_PHASE_PROFILE="1")
    if os.name == "nt":
        environment["LOCALAPPDATA"] = str(directory)
    elif platform.system() == "Darwin":
        environment["HOME"] = str(directory)
    else:
        environment["XDG_CACHE_HOME"] = str(directory)
    return environment


def counter(text, name):
    return sum(int(value) for value in re.findall(
        rf"^\s*(\d+) n\s+{re.escape(name)}\s*$", text, re.MULTILINE))


def graph(directory):
    records = list(directory.rglob("source-graphs/*.rec"))
    if len(records) != 1:
        raise AssertionError(f"expected one retained source graph, found {records}")
    return records[0]


class Gate:
    def __init__(self, compiler, output, target):
        self.compiler = compiler
        self.output = output
        self.target = target
        self.observations = []
        self.comparisons = []
        self.commands = 0

    def command(self, arguments, project, cache, label, compiler=None):
        self.commands += 1
        command = [str(compiler or self.compiler), *map(str, arguments)]
        started = time.monotonic()
        result = subprocess.run(command, cwd=project, env=cache_environment(cache),
                                stdin=subprocess.DEVNULL, capture_output=True,
                                text=True, encoding="utf-8", errors="replace", timeout=120)
        text = result.stdout + result.stderr
        log = self.output / f"{self.commands:03}-{label}.log"
        log.write_text(text, encoding="utf-8")
        self.observations.append(dict(command=command, exit_code=result.returncode,
                                      seconds=time.monotonic() - started, log=log.name))
        return result.returncode, text

    def prepare(self, project, cache, label, compiler=None):
        code, text = self.command(
            ["update", "--offline", "--project", project, "--target", self.target],
            project, cache, label + "-update", compiler)
        # Only this invocation's emitted review can supply decisions. An old
        # omega.lock or old review cannot turn a failed update into acceptance.
        reported = [Path(value.strip()) for value in REVIEW_LINE.findall(text)]
        if reported:
            review = next((path for path in reported if path.name == f"review-{self.target}.txt"), None)
            if review is None or not review.is_file():
                raise AssertionError("update named a missing review document")
            settled, count = accept_pending_decisions(review.read_text(encoding="utf-8"))
            if count:
                review.write_text(settled, encoding="utf-8")
                code, resumed = self.command(["update", "--resume", "--offline", "--project", project],
                                             project, cache, label + "-resume", compiler)
                text += resumed
        return code, text

    def observe(self, project, cache, label, *, expected=0, rejection=None,
                prepare=True, compiler=None):
        destination = self.output / label
        destination.mkdir()
        source = {str(path.relative_to(project)): path.read_text(encoding="utf-8")
                  for path in project.rglob("*.omg") if "build" not in path.relative_to(project).parts[:-1]}
        (destination / "source.json").write_text(json.dumps(source, indent=2), encoding="utf-8")
        code, text = self.prepare(project, cache, label, compiler) if prepare else (0, "")
        if code == 0:
            code, compiled = self.command(
                ["--offline", "--timings", "--target", self.target, "--build-dir", destination,
                 project / "main.omg"], project, cache, label + "-compile", compiler)
            text += compiled
        image = destination / ("omega-program.exe" if os.name == "nt" else "omega-program")
        if rejection is not None:
            assert code != 0 and rejection in text, f"{label}: missing intended rejection {rejection!r}\n{text}"
            assert not image.exists(), f"{label}: rejected source published an image"
            normalized = text.replace(str(cache).replace("\\", "\\\\"), "<cache>").replace(str(cache), "<cache>")
            errors = tuple(line.strip() for line in normalized.splitlines() if line.strip().startswith("error:"))
            assert errors, f"{label}: rejection had no diagnostic"
            return ("rejected", errors), text
        assert code == 0 and image.is_file(), f"{label}: no successful fresh image\n{text}"
        executed = subprocess.run([str(image)], stdin=subprocess.DEVNULL, capture_output=True, timeout=10)
        exit_code = executed.returncode
        if os.name == "nt" and exit_code >= 2**31:
            exit_code -= 2**32
        allowed = ({-1073741795} if os.name == "nt" else {-signal.SIGILL, -signal.SIGTRAP}) if expected == "trap" else {expected}
        assert exit_code in allowed, f"{label}: native exit {exit_code}, expected {expected}"
        result = ("accepted", hashlib.sha256(image.read_bytes()).hexdigest(), exit_code,
                  executed.stdout.hex(), executed.stderr.hex())
        (destination / "native.json").write_text(json.dumps(result), encoding="utf-8")
        return result, text

    def pair(self, project, warm, cold, label, **options):
        retained, text = self.observe(project, warm, label + "-warm", **options)
        fresh, _ = self.observe(project, cold, label + "-fresh", **options)
        assert retained == fresh, f"{label}: warm/fresh disagreement: {retained} != {fresh}"
        self.comparisons.append(dict(label=label, result=retained,
                                     retained_hits=counter(text, "05.b.retained_match.hit"),
                                     loaded_graphs=counter(text, "source.graph.loaded")))
        return text


def project_at(directory, target):
    project = directory / "project"
    project.mkdir()
    for name in ("main.omg", "result.omg"):
        (project / name).write_bytes((FIXTURE / name).read_bytes())
    (project / "build.omg").write_text(
        'machine build(builder: &mut Build) { Build::application(builder, "cache_gate"); '
        f'Build::bind(builder, {target}::ProgramEntry, main); }}\n', encoding="utf-8")
    return project


def source_mutations(gate, directory):
    for name in ("body", "contract", "package-contract", "missing-module", "build-entry", "transitive"):
        with tempfile.TemporaryDirectory(prefix=name + "-", dir=directory) as temporary:
            root = Path(temporary)
            project = project_at(root, gate.target)
            warm = root / "warm"
            dependency = project / "result.omg"
            if name in {"contract", "package-contract"}:
                dependency.write_text('module result; public machine answer(value: u64) -> i32 requires value < 8u64 { 0 }\n')
                main = project / "main.omg"
                main.write_text(main.read_text().replace("answer()", "answer(5u64)"))
                if name == "package-contract":
                    package = project / "dependency"
                    package.mkdir()
                    (package / "build.omg").write_text('machine build(builder: &mut Build) { Build::package(builder, "dependency"); }\n')
                    (package / "main.omg").write_text('use result;\n')
                    dependency.rename(package / "result.omg")
                    dependency = package / "result.omg"
                    main.write_text(main.read_text().replace("use result;", "use dependency::result;"))
                    build = project / "build.omg"
                    build.write_text(build.read_text().replace("Build::application", 'Build::depend_as(builder, "dependency", Source::Path { location: "dependency" }); Build::application'))
            if name == "transitive":
                (project / "limit.omg").write_text('module limit; public machine value() -> i32 { 0 }\n')
                dependency.write_text('module result; use limit; public machine answer() -> i32 { limit::value() }\n')
            gate.observe(project, warm, name + "-seed")
            text = gate.pair(project, warm, root / "fresh-initial", name + "-unchanged", prepare=False)
            assert counter(text, "source.graph.loaded") and counter(text, "05.b.retained_match.hit"), f"{name}: did not exercise a retained judgment"
            rejection, expected = None, 0
            changed = dependency
            original = changed.read_bytes()
            if name == "body":
                changed.write_bytes(original.replace(b"{ 0 }", b"{ 1 }"))
                expected = "trap"
            elif name in {"contract", "package-contract"}:
                metadata = changed.stat()
                changed.write_bytes(original.replace(b"8u64", b"4u64"))
                os.utime(changed, ns=(metadata.st_atime_ns, metadata.st_mtime_ns))
                assert changed.stat().st_size == metadata.st_size
                assert changed.stat().st_mtime_ns == metadata.st_mtime_ns
                rejection = "violates required fact"
            elif name == "missing-module":
                changed.unlink()
                rejection = "captured entry is absent: result.omg"
            elif name == "build-entry":
                changed = project / "build.omg"
                original = changed.read_bytes()
                changed.write_bytes(original.replace(b"ProgramEntry, main", b"ProgramEntry, bad"))
                expected = "trap"
            else:
                changed = project / "limit.omg"
                original = changed.read_bytes()
                changed.write_bytes(original.replace(b"{ 0 }", b"{ 1 }"))
                expected = "trap"
            gate.pair(project, warm, root / "fresh-locked", name + "-stale-lock",
                      prepare=False, rejection="omega update")
            gate.pair(project, warm, root / "fresh-mutated", name + "-changed", expected=expected, rejection=rejection)
            changed.write_bytes(original)
            gate.pair(project, warm, root / "fresh-restored", name + "-restored")
            print(f"PASS {name}: unchanged, mutation, recovery", flush=True)


def cache_damage(gate, directory):
    with tempfile.TemporaryDirectory(prefix="records-", dir=directory) as temporary:
        root = Path(temporary)
        project = project_at(root, gate.target)
        warm = root / "warm"
        gate.observe(project, warm, "record-seed")
        record = graph(warm)
        original = record.read_bytes()
        for name in ("truncated", "payload", "compiler", "schema", "orphan-write", "absent"):
            damaged = bytearray(original)
            if name == "truncated":
                damaged = damaged[:79]
            elif name == "payload":
                damaged[-1] ^= 1
            elif name == "compiler":
                damaged[8] ^= 1
            elif name == "schema":
                damaged[0] ^= 1
            record.write_bytes(damaged)
            if name == "orphan-write":
                record.with_name(".123-0.tmp").write_bytes(original[:79])
            elif name == "absent":
                record.unlink()
            text = gate.pair(project, warm, root / ("fresh-" + name), "record-" + name, prepare=False)
            if name in {"truncated", "payload", "compiler", "schema"}:
                assert counter(text, "source.graph.invalid"), f"{name}: corruption was not observed"
            print(f"PASS record-{name}: native result agrees with fresh build", flush=True)
        # Kill only the child we launched, after observing its unpublished record.
        record.write_bytes(original)
        dependency = project / "result.omg"
        dependency.write_bytes(dependency.read_bytes().replace(b"{ 0 }", b"{ 1 }"))
        command = [str(gate.compiler), "update", "--offline", "--project", str(project), "--target", gate.target]
        log = gate.output / "interrupted-write.log"
        started = time.monotonic()
        interrupted = None
        with log.open("wb") as output:
            process = subprocess.Popen(command, cwd=project, env=cache_environment(warm),
                                       stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.STDOUT)
            try:
                while process.poll() is None and time.monotonic() - started < 120:
                    for temporary_record in record.parent.glob(f".{process.pid}-*.tmp"):
                        try:
                            size = temporary_record.stat().st_size
                        except FileNotFoundError:
                            continue
                        process.kill()
                        process.wait(timeout=10)
                        interrupted = dict(temporary=temporary_record.name, observed_bytes=size)
                        break
                    if interrupted is not None:
                        break
                    time.sleep(0.002)
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=10)
        gate.observations.append(dict(command=command, exit_code=process.returncode,
                                      seconds=time.monotonic() - started, log=log.name,
                                      interruption=interrupted))
        assert interrupted is not None, "did not observe and interrupt a source-graph write"
        assert record.read_bytes() == original, "interruption did not preserve the previously published record"
        gate.pair(project, warm, root / "fresh-interrupted", "record-interrupted", expected="trap")
        print("PASS record-interrupted: killed writer, then rebuilt changed source correctly", flush=True)


def compiler_switch(gate, directory, other):
    with tempfile.TemporaryDirectory(prefix="compiler-", dir=directory) as temporary:
        root = Path(temporary)
        project = project_at(root, gate.target)
        warm = root / "warm"
        gate.observe(project, warm, "compiler-first")
        first = graph(warm)
        first_bytes = first.read_bytes()
        gate.pair(project, warm, root / "fresh-other-locked", "compiler-stale-lock",
                  prepare=False, rejection="omega update", compiler=other)
        gate.pair(project, warm, root / "fresh-other", "compiler-other", compiler=other)
        records = list(warm.rglob("source-graphs/*.rec"))
        assert len(records) == 2 and first.read_bytes() == first_bytes, "different compiler must retain a distinct source namespace"
        gate.pair(project, warm, root / "fresh-return", "compiler-return")
        print("PASS compiler-switch: separate namespaces and matching native results", flush=True)


def main():
    if not __debug__:
        raise RuntimeError("run this assertion-based gate without Python -O")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--other-cli", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    first = require_optimized(arguments.cli)
    other = require_optimized(arguments.other_cli)
    if first["sha256"] == other["sha256"]:
        parser.error("compiler-switch coverage requires different producer binaries")
    output = arguments.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    gate = Gate(arguments.cli.resolve(), output, host_target())
    result = dict(compilers=[first, other], target=gate.target, status="failed",
                  gate_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  commands=gate.observations, comparisons=gate.comparisons)
    try:
        with tempfile.TemporaryDirectory(prefix="cache-gate-") as temporary:
            directory = Path(temporary).resolve()
            source_mutations(gate, directory)
            cache_damage(gate, directory)
            compiler_switch(gate, directory, arguments.other_cli.resolve())
        result["status"] = "passed"
    finally:
        (output / "result.json").write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(f"PASS cache independence; evidence: {output}")


if __name__ == "__main__":
    main()
