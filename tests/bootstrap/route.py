#!/usr/bin/env python3
"""Run one Epsilon program with independently selected Gamma, Delta and Epsilon implementations."""
import argparse
from contextlib import nullcontext, redirect_stdout
from dataclasses import dataclass
from functools import cached_property
from hashlib import sha256
from itertools import product
import json
import os
from pathlib import Path
import subprocess
import sys
import time

from compiled import (ROOT, add_invocation_budget, delta_request, executable, gamma_backend_source, gamma_request,
                      invoke, pack, pack_delta_compiler, reconstruct)
from conformance.vectors import epsilon_program_cases, epsilon_request
from independent.epsilon.composition import CompileRefusal, compiler_source, linked_program
from independent.epsilon.enum_test import byte_class_case, byte_class_wrong_base_case
from independent.epsilon.lexer_scan_test import public_scan_case, public_scan_wrong_offset_case, punctuation_case
from independent.product_wire import check_tape
from independent.whole_request import small_constructor_case
from interpreted.delta.interpreter import materialize

MODES = ("interpret", "alpha", "native")
COMPILER_PROGRAMS = {
    "omega-constructor": small_constructor_case,
    "omega-byte-classes": byte_class_case,
    "omega-lexer": public_scan_case,
    "omega-punctuation": punctuation_case,
}
COMPILER_REFUSALS = {
    "omega-wrong-base": byte_class_wrong_base_case,
    "omega-wrong-offset": public_scan_wrong_offset_case,
}


@dataclass(frozen=True)
class Program:
    image: Path
    # Each interpreter consumes one framed source and passes the remaining bytes to its guest.
    sources: tuple[Path, ...] = ()

    def bind(self, source):
        return Program(self.image, (*self.sources, source))

    def input(self, payload):
        return b"".join(gamma_request(source.read_bytes()) for source in self.sources) + payload


def observation_kind(data):
    # Epsilon v1 defines closed trap and rejection codes. FF frames are transport refusals.
    if data[:1] == b"\x00" and len(data) >= 5:
        return "exit"
    if data[:1] == b"\x01" and len(data) >= 2 and 1 <= data[1] <= 9:
        return "trap"
    if data[:1] == b"\x02" and len(data) == 6 and 1 <= data[1] <= 26:
        return "reject"
    if len(data) == 40 and data.startswith(b"\xffEEOUT\x01\x00"):
        raise ValueError(f"EEOUT refusal: {data.hex()}")
    raise ValueError(f"invalid Epsilon observation ({len(data)} bytes): {data[:40].hex()}")


class Route:
    def __init__(self, args, work, shared=None, *, expect_rejection=False):
        self.args = args
        self.work = work
        self.steps = []
        self.backends = {}
        self.artifacts = {}
        self.shared = shared
        self.expect_rejection = expect_rejection
        self.epsilon_interpreter = getattr(args, "epsilon_interpreter", "direct")

    def stage(self, description, action):
        self.steps.append((description, action))

    def publish(self, path, data):
        path.write_bytes(data)
        record = {"bytes": len(data), "sha256": sha256(data).hexdigest()}
        self.artifacts[path.name] = record
        return record

    def file(self, name, description, produce):
        path = self.work / name
        self.stage(f"{description} -> {name}", lambda: self.publish(path, produce()))
        return path

    def run(self, name, description, program, payload, *, terminal=False):
        chain = " -> ".join(path.name for path in (program.image, *program.sources))
        path = self.work / name

        def run():
            request = program.input(payload())
            key = sha256(program.image.read_bytes()).hexdigest(), sha256(request).hexdigest()
            cache = None if terminal else self.shared
            previous = cache.get(key) if cache is not None else None
            if previous is None:
                data = invoke([program.image], request, self.args.seconds)
            else:
                original, digest = previous
                data = original.read_bytes()
                if sha256(data).hexdigest() != digest:
                    raise ValueError(f"shared producer output changed: {original}")
            record = self.publish(path, data)
            record.update(producer_sha256=key[0], request_sha256=key[1])
            if previous is not None:
                record["reused_from"] = os.path.relpath(original, self.work.parent)
            elif cache is not None:
                cache[key] = path, record["sha256"]

        self.stage(f"{description} [{chain}] -> {name}", run)
        return path

    @cached_property
    def gamma_interpreter(self):
        self.stage("rebuild Beta and the Gamma interpreter on Alpha",
                   lambda: reconstruct(self.work, seconds=self.args.seconds))
        return Program(self.work / "reference")

    def prepare_gamma(self):
        host = self.args.gamma
        if host == "interpret":
            self.host = self.gamma_interpreter
            return
        build = self.args.gamma_build or self.work / "cold"
        if self.args.gamma_build:
            def check_build():
                if (build / "compiler.gamma").read_bytes() != gamma_backend_source(self.target(host)):
                    raise ValueError(f"{build}: Gamma compiler source differs from the selected host and checkout")
                compiler = build / "gamma-compiler"
                if not os.access(compiler, os.X_OK):
                    raise ValueError(f"{build}: missing executable gamma-compiler")
                if compiler.read_bytes() != (build / "gamma1").read_bytes():
                    raise ValueError(f"{build}: Gamma compiler differs from its fixed-point rebuild")
            self.stage(f"reuse {host} cold build at {build}", check_build)
        else:
            launcher = ROOT / "tests/bootstrap/linux_x86_64/bootstrap"
            command = [launcher, *(["--alpha"] if host == "alpha" else []), ROOT, build]
            self.stage(f"cold-build the {host} Gamma compiler from Alpha and Beta",
                       lambda: invoke(command, b"", self.args.seconds))
        self.host = Program(build / "gamma-compiler")

    @staticmethod
    def target(mode):
        return {"alpha": "alpha", "native": "linux"}[mode]

    def image(self, name, target, compiler, source):
        artifact = self.run(name + (".tape" if target == "alpha" else ".elf"),
                            f"compile {source.name} for {target}", compiler, source.read_bytes)
        self.stage(f"prepare {artifact.name} for {target}", lambda: executable(artifact, self.target(target)))
        return Program(artifact.with_suffix(".vm") if target == "alpha" else artifact)

    def on_gamma(self, name, source):
        if self.args.gamma == "interpret":
            return self.host.bind(source)
        return self.image(name, self.args.gamma, self.host, source)

    def backend(self, target):
        if target == self.args.gamma:
            return self.host
        if target not in self.backends:
            source = self.file(f"gamma-to-{target}.gamma", f"select the {target} Gamma backend",
                               lambda: gamma_backend_source(self.target(target)))
            self.backends[target] = self.on_gamma(f"gamma-to-{target}", source)
        return self.backends[target]

    @cached_property
    def delta_compiler(self):
        source = self.work / "delta.gamma"
        self.stage("assemble the Gamma-written Delta compiler", lambda: pack_delta_compiler(self.work))
        return self.on_gamma("delta-compiler", source)

    @cached_property
    def support(self):
        path = self.work / "support.bin"
        self.stage("assemble Delta support", lambda: pack(
            "bootstrap/delta/compiler/support/support.gamma.sources", path))
        return path

    @cached_property
    def direct_delta_interpreter(self):
        source = self.work / "interpreter.gamma"
        self.stage("assemble the direct Delta interpreter", lambda: materialize(self.work, self.args.profile))
        return self.on_gamma("delta-interpreter", source)

    def on_delta(self, name, source, mode):
        if mode == "interpret" and self.args.delta_interpreter == "direct":
            return self.direct_delta_interpreter.bind(source)
        compiler = self.delta_compiler
        support = self.support
        profile = 1 if self.args.profile == "conformance" else 2
        gamma = self.run(name + ".gamma", f"translate {source.name} to Gamma", compiler,
                         lambda: delta_request(source.read_bytes(), support.read_bytes(), profile))
        if mode == "interpret":
            return self.gamma_interpreter.bind(gamma)
        return self.image(name, mode, self.backend(mode), gamma)

    @property
    def evaluates_epsilon_source(self):
        return self.args.epsilon == "interpret" and self.epsilon_interpreter == "direct"

    def select(self):
        self.prepare_gamma()
        closure = self.work / "epsilon.delta"
        self.stage("assemble Epsilon evaluator sources", lambda: pack(
            "bootstrap/epsilon/interpreter/epsilon_compiler.delta.sources", closure))
        guest = self.work / "subject.epsilon"
        stdin = self.work / "stdin.bin"
        if self.evaluates_epsilon_source:
            entry = ROOT / "bootstrap/epsilon/interpreter.delta"
            source = self.file("evaluator.delta", "select the Epsilon interpreter",
                               lambda: closure.read_bytes() + entry.read_bytes())
            evaluator = self.on_delta("evaluator", source, self.args.delta)
            self.run("guest.eeout", "interpret subject.epsilon with stdin.bin", evaluator,
                     lambda: epsilon_request(closure.read_bytes(), guest.read_bytes(), stdin.read_bytes()),
                     terminal=True)
        else:
            source = self.file("epsilon-compiler.delta", "select the Epsilon compiler",
                               lambda: compiler_source(closure.read_bytes()))
            compiler = self.on_delta("epsilon-compiler", source, self.args.delta)
            if self.expect_rejection:
                self.run("compiler.response", "check subject.epsilon for the expected rejection", compiler,
                         guest.read_bytes, terminal=True)
                return
            module = self.run("module.out", "compile subject.epsilon to Delta", compiler, guest.read_bytes)
            delta = self.file("subject.delta", "read compiler result and attach the runtime",
                              lambda: linked_program(module.read_bytes()))
            program = self.on_delta("subject", delta, self.args.epsilon)
            self.run("guest.eeout", "run the compiled Epsilon program with stdin.bin", program,
                     stdin.read_bytes, terminal=True)

        if getattr(self.args, "case", None) == "omega-constructor":
            self.stage("run the constructed Omega program on the Alpha seed and require exit 42",
                       self.check_omega_program)

    def check_omega_program(self):
        output = (self.work / "guest.eeout").read_bytes()
        if output[:5] != bytes(5):
            raise ValueError(f"Omega constructor did not return exit 0: {output[:9].hex()}")
        tape = output[5:]
        check_tape(self.work, tape, self.args.seconds, name="omega-program", expected=42)
        self.artifacts["omega-program.tape"] = {"bytes": len(tape), "sha256": sha256(tape).hexdigest(),
                                               "guest_exit": 42}


def check_observation(route, expected, report):
    compiler_response = route.expect_rejection and not route.evaluates_epsilon_source
    name = "compiler.response" if compiler_response else "guest.eeout"
    output = (route.work / name).read_bytes()
    if route.expect_rejection:
        code, offset = expected[1], int.from_bytes(expected[2:], "little")
    if compiler_response:
        expected = f"rejected {code} {offset}".encode("ascii")
    else:
        report["observation"] = observation_kind(output)
        if report["observation"] == "exit":
            report["guest_exit"] = int.from_bytes(output[1:5], "little", signed=True)
    report["observation_sha256"] = sha256(output).hexdigest()
    if expected is not None and output != expected:
        first = next((i for i, (a, b) in enumerate(zip(expected, output)) if a != b),
                     min(len(expected), len(output)))
        raise ValueError(f"wrong observation at byte {first}: "
                         f"expected {expected[first:first+1]!r} ({len(expected)} bytes), "
                         f"observed {output[first:first+1]!r} ({len(output)} bytes)")
    if route.expect_rejection:
        report.update(observation="reject", rejection_code=code, source_offset=offset,
                      observation_artifact=name, guest_executed=False)
    if compiler_response:
        report["result"] = "expected-rejection"
    else:
        matched = expected is not None or "omega-program.tape" in route.artifacts
        report["result"] = "matched" if matched else "observed"


def prior_invocations(directories):
    shared = {}
    for directory in directories:
        row = directory / "result.json"
        reports = [row] if row.is_file() else sorted(directory.glob("*/result.json"))
        if not reports:
            raise ValueError(f"{directory}: no route receipts")
        for path in reports:
            for name, record in json.loads(path.read_text())["artifacts"].items():
                if name in ("guest.eeout", "compiler.response") or "producer_sha256" not in record:
                    continue
                if not name or Path(name).name != name or name in (".", ".."):
                    raise ValueError(f"{path}: invalid artifact name {name!r}")
                key = record["producer_sha256"], record["request_sha256"]
                digest = record["sha256"]
                original, previous_digest = shared.setdefault(key, (path.parent / name, digest))
                if previous_digest != digest:
                    raise ValueError(f"identical invocation has different outputs at {original} and {path.parent / name}")
    return shared


def save_report(work, report):
    pending = work / "result.json.tmp"
    pending.write_text(json.dumps(report, indent=2) + "\n")
    pending.replace(work / "result.json")


def execute_route(route, guest, stdin, expected, verbose):
    work = route.work
    work.mkdir(parents=True, exist_ok=False)
    (work / "subject.epsilon").write_bytes(guest)
    (work / "stdin.bin").write_bytes(stdin)
    args = route.args
    uses_delta_interpreter = (args.delta == "interpret" or
                              (args.epsilon == "interpret" and route.epsilon_interpreter == "delta"))
    report = {"gamma": args.gamma, "delta": args.delta, "epsilon": args.epsilon, "profile": args.profile,
              "delta_interpreter": args.delta_interpreter if uses_delta_interpreter else None,
              "epsilon_interpreter": route.epsilon_interpreter if args.epsilon == "interpret" else None,
              "source_sha256": sha256(guest).hexdigest(), "stdin_sha256": sha256(stdin).hexdigest(),
              "seconds_per_invocation": args.seconds, "stages": [], "artifacts": route.artifacts, "result": "running"}
    if expected is not None:
        report["expected_observation_sha256"] = sha256(expected).hexdigest()
    status = 0
    try:
        log = nullcontext(sys.stdout) if verbose else (work / "stages.log").open("w")
        with log as stream, redirect_stdout(stream):
            for description, action in route.steps:
                print(description, flush=True)
                stage = {"stage": description}
                report["stages"].append(stage)
                start = time.monotonic()
                try:
                    action()
                    stage["completed"] = True
                finally:
                    stage["elapsed_seconds"] = round(time.monotonic() - start, 3)
                save_report(work, report)
        check_observation(route, expected, report)
    except CompileRefusal as error:
        status = 1
        report.update(result="compiler-refusal", detail=str(error))
    except subprocess.TimeoutExpired:
        status = 124
        report["result"] = "timeout"
    except (OSError, RuntimeError, ValueError, subprocess.CalledProcessError, SystemExit) as error:
        status = 1
        report.update(result="failed", detail=str(error))
    if report["stages"] and report["stages"][0].get("completed"):
        report["gamma_host_sha256"] = sha256(route.host.image.read_bytes()).hexdigest()
    save_report(work, report)
    return status, report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, nargs="?", help="new artifact directory (optional for --plan)")
    for rung in ("gamma", "delta", "epsilon"):
        parser.add_argument("--" + rung, choices=(*MODES, "all"), required=True)
    program = parser.add_mutually_exclusive_group()
    program.add_argument("--source", type=Path, help="complete Epsilon program with its entry")
    program.add_argument("--case", choices=("stdin", "exit", "trap", *COMPILER_PROGRAMS, *COMPILER_REFUSALS),
                         help="program with a known observation (default: stdin)")
    parser.add_argument("--stdin", type=Path, help="raw guest input for --source (default: empty)")
    parser.add_argument("--expect-observation", type=Path, metavar="FILE",
                        help="exact binary Epsilon exit, trap or source rejection for --source")
    parser.add_argument("--epsilon-interpreter", choices=("direct", "delta"), default="direct",
                        help="evaluate Epsilon source directly or compile it and interpret the resulting Delta")
    parser.add_argument("--delta-interpreter", choices=("direct", "gamma"), default="direct",
                        help="interpret checked Delta directly or translate it to Gamma and interpret that source")
    parser.add_argument("--gamma-build", type=Path, help="reuse the selected Alpha or native cold build")
    parser.add_argument("--reuse-stages", action="store_true",
                        help="share intermediate outputs of identical invocations within this matrix")
    parser.add_argument("--reuse-from", action="append", default=[], type=Path, metavar="DIR",
                        help="reuse intermediate invocations recorded in an earlier route or matrix directory")
    parser.add_argument("--profile", choices=("conformance", "construction"),
                        help="Delta byte limits (default: construction for omega-constructor, otherwise conformance)")
    add_invocation_budget(parser)
    parser.add_argument("--plan", action="store_true", help="print the stages without building or running them")
    args = parser.parse_args()
    if args.profile is None:
        args.profile = "construction" if args.case == "omega-constructor" else "conformance"
    if not args.output and not args.plan:
        parser.error("an output directory is required for execution")
    if args.stdin and not args.source:
        parser.error("--stdin requires --source")
    if args.epsilon_interpreter == "delta" and args.epsilon not in ("interpret", "all"):
        parser.error("--epsilon-interpreter delta requires --epsilon interpret or all")
    if (args.delta_interpreter == "gamma" and args.delta not in ("interpret", "all")
            and args.epsilon_interpreter != "delta"):
        parser.error("--delta-interpreter gamma requires a Delta interpreter for the producer or guest")
    if args.gamma_build and args.gamma not in ("alpha", "native"):
        parser.error("--gamma-build requires --gamma alpha or --gamma native")
    root = (args.output or Path(".")).resolve()
    choices = (args.gamma, args.delta, args.epsilon)
    matrix = "all" in choices
    if args.reuse_stages and not matrix:
        parser.error("--reuse-stages requires at least one selector set to all")
    expected = None
    expect_rejection = args.case in COMPILER_REFUSALS
    if args.expect_observation:
        if not args.source:
            parser.error("--expect-observation requires --source")
        try:
            expected = args.expect_observation.read_bytes()
            expect_rejection = observation_kind(expected) == "reject"
        except (OSError, ValueError) as error:
            parser.error(str(error))
    try:
        shared = prior_invocations(args.reuse_from) if args.reuse_from or args.reuse_stages else None
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        parser.error(f"invalid stage receipts: {error}")
    if not args.plan:
        if root.exists():
            parser.error(f"output already exists: {root}")
        try:
            if args.source:
                guest = args.source.read_bytes()
                stdin = args.stdin.read_bytes() if args.stdin else b""
            elif expect_rejection:
                guest, stdin, expected = COMPILER_REFUSALS[args.case](ROOT)
            elif args.case in COMPILER_PROGRAMS:
                guest, stdin, expected = COMPILER_PROGRAMS[args.case](ROOT)
            else:
                cases = {name: (source, stdin, result) for name, source, stdin, result in epsilon_program_cases()}
                guest, stdin, expected = cases[args.case or "stdin"]
        except OSError as error:
            parser.error(str(error))
        if matrix:
            root.mkdir(parents=True)
    builds = {args.gamma: args.gamma_build.resolve()} if args.gamma_build else {}
    observations = {}
    exit_status = 0
    axes = [MODES if mode == "all" else (mode,) for mode in choices]
    if matrix and not args.plan:
        headers = ["Epsilon " + mode for mode in axes[2]]
        print("Gamma      Delta      " + " ".join(f"{header:18}" for header in headers).rstrip(), flush=True)
    cells = []
    for gamma, delta, epsilon in product(*axes):
        label = f"{gamma}-{delta}-{epsilon}"
        row = argparse.Namespace(**vars(args))
        row.gamma, row.delta, row.epsilon = gamma, delta, epsilon
        row.gamma_build = builds.get(gamma)
        work = root / label if matrix else root
        route = Route(row, work, shared, expect_rejection=expect_rejection)
        route.select()
        if args.plan:
            print(label)
            for description, _ in route.steps:
                print("  " + description)
            if gamma != "interpret" and not row.gamma_build:
                builds[gamma] = work / "cold"
            continue
        status, report = execute_route(route, guest, stdin, expected, verbose=not matrix)
        if matrix:
            cells.append(report["result"])
            if epsilon == axes[2][-1]:
                print(f"{gamma:10} {delta:10} " + " ".join(f"{cell:18}" for cell in cells).rstrip(), flush=True)
                cells.clear()
        else:
            detail = report.get("detail", "")
            if status and report["stages"]:
                detail = f"{report['stages'][-1]['stage']} {detail}"
            elif "rejection_code" in report:
                detail = f"rejection {report['rejection_code']} at source byte {report['source_offset']}"
            elif "omega-program.tape" in report["artifacts"]:
                exit_code = report["artifacts"]["omega-program.tape"]["guest_exit"]
                detail = f"Omega compiler exit {report['guest_exit']}, emitted Omega program exit {exit_code}"
            elif "guest_exit" in report:
                detail = f"guest exit {report['guest_exit']}"
            print(f"{label}  {report['result']}  {detail}".rstrip(), flush=True)
        if status and exit_status != 1:
            exit_status = status
        if not status:
            key = ((report["rejection_code"], report["source_offset"]) if expect_rejection
                   else report["observation_sha256"])
            observations.setdefault(key, []).append(label)
        # The cold launcher publishes gamma-compiler after the fixed-point check.
        if report["stages"][0].get("completed") and (work / "cold/gamma-compiler").is_file():
            builds[gamma] = work / "cold"
    if len(observations) > 1:
        for digest, rows in observations.items():
            print(f"{digest}  {', '.join(rows)}")
        comparison = "rejection code and source offset" if expect_rejection else "guest.eeout"
        print(f"Completed routes disagree on {comparison}.", file=sys.stderr)
        return 1
    if matrix and observations:
        digest, rows = next(iter(observations.items()))
        if expect_rejection:
            code, offset = digest
            print(f"{len(rows)} matched rejection {code} at source byte {offset}.")
        else:
            print(f"{len(rows)} completed with observation {digest}.")
    return exit_status


if __name__ == "__main__":
    sys.exit(main())
