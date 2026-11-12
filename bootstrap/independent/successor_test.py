#!/usr/bin/env python3
"""Construct the real Delta/Epsilon successors with standalone native Gamma.

Default execution repeats cold native-compiler construction first. The explicit
--reuse-constructed option checks and reuses that retained construction; it
adds no alternative semantic producer. Epsilon remains an evaluator here:
this milestone does not claim standalone compilation of the Omega bootstrap
compiler's Epsilon source.
"""
import argparse
import importlib.util
import json
import os
from datetime import datetime, timezone
from pathlib import Path
import platform
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
# Each peer owns a source_test.py; distinct module identity keeps their imports separate.
spec = importlib.util.spec_from_file_location(
    "native_construction", Path(__file__).with_name("source_test.py"))
construction = importlib.util.module_from_spec(spec)
spec.loader.exec_module(construction)
vm = construction.vm


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reuse-constructed", action="store_true")
    parser.add_argument("--seconds", type=int, default=1800)
    parser.add_argument("--omega", action="store_true")
    parser.add_argument("--omega-control", action="store_true",
                        help="construct the real lexer classifiers and execute nominal/control checks")
    parser.add_argument("--omega-utf8", action="store_true",
                        help="construct the real decoder and execute arithmetic/refusal controls")
    parser.add_argument("--literals", action="store_true",
                        help="validate Delta byte literals and the actual Epsilon source encoder")
    parser.add_argument("--compile-epsilon", action="store_true",
                        help="compile production lexical/request modules and exercise receiver storage/views")
    args = parser.parse_args()
    if args.seconds <= 0:
        parser.error("--seconds must be positive")
    if (platform.system(), platform.machine()) != ("Linux", "x86_64"):
        parser.error("native successors currently require Linux x86-64")
    import resource
    work = ROOT / "build/independent-successors"
    work.mkdir(parents=True, exist_ok=True)
    result = {"state": "running", "stage": "preflight", "passed_stages": [],
              "pid": os.getpid(), "started_at": datetime.now(timezone.utc).isoformat(),
              "reused_constructed_predecessor": args.reuse_constructed,
              "omega_fixture": args.omega, "omega_utf8": args.omega_utf8,
              "omega_control": args.omega_control,
              "byte_literals": args.literals,
              "compile_epsilon": args.compile_epsilon}

    def checkpoint(stage, subjects):
        result["stage"] = stage
        for label, name in subjects:
            result[label] = construction.identity((work / name).read_bytes())
        result["children_maxrss_kib"] = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
        pending = work / "report.json.tmp"
        pending.write_text(json.dumps(result, indent=2) + "\n")
        pending.replace(work / "report.json")

    checkpoint("preflight", ())
    if not args.reuse_constructed:
        subprocess.run([sys.executable, str(Path(__file__).with_name("source_test.py")),
                        "--seconds", str(args.seconds)], cwd=ROOT, check=True)
    predecessor = ROOT / "build/independent-native-reference"
    report = json.loads((predecessor / "report.json").read_text())
    assert report["mode"] == "reference", "successors require the source-constructed compiler"
    assert report["compiler_source"] == construction.identity(construction.source(True))
    compiler = predecessor / "compiler2"
    assert report["compiler_elf"] == construction.identity(compiler.read_bytes())
    # The retained differential evaluator is also a semantic executable. Bind
    # it to the audited seed plus the Gamma tape reconstructed in the cold run.
    vm.shell('''
      . "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
      . "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"
      materialize_gamma_evaluator "$1/reference-expected"
    ''', work)
    reference = predecessor / "reference"
    assert reference.read_bytes() == (work / "reference-expected").read_bytes(), "retained Gamma evaluator identity"
    result.update(native_compiler_source=report["compiler_source"],
                  native_compiler_elf=report["compiler_elf"],
                  gamma_evaluator_image=construction.identity(reference.read_bytes()))

    class NativeCompiler:
        def __init__(self, subject):
            self.subject = subject

        def compile(self):
            status, result = vm.execute(compiler, self.subject, args.seconds)
            if status:
                assert result == b"" and status == 249, ("Gamma compilation", status)
                raise vm.Invalid("Gamma source compiler rejected malformed source")
            return result

    checkpoint("delta", ())
    vm.customer(work, reference, producer=NativeCompiler,
                realize=construction.realize)
    result["passed_stages"].append("delta")
    checkpoint("epsilon", (("delta_source", "delta.gamma"),
                           ("delta_elf", "delta.elf"),
                           ("delta_receipt", "receipt.gamma")))
    if args.literals:
        literal_spec = importlib.util.spec_from_file_location(
            "literal_controls", Path(__file__).parent / "epsilon/literal_controls.py")
        literal_controls = importlib.util.module_from_spec(literal_spec)
        literal_spec.loader.exec_module(literal_controls)
        result["literal_controls"] = literal_controls.run(
            work, vm, NativeCompiler, construction.realize, args.seconds)
        result["passed_stages"].append("literal_emitter")
        checkpoint("epsilon", ())
    vm.epsilon_customer(work, producer=NativeCompiler, realize=construction.realize)
    result["passed_stages"].append("epsilon")
    next_stage = "omega" if args.omega or args.omega_utf8 or args.omega_control else "complete"
    checkpoint("compiled_epsilon" if args.compile_epsilon else next_stage,
               (("epsilon_request", "epsilon-request.bin"),
                ("epsilon_gamma_receipt", "epsilon.gamma"),
                ("epsilon_elf", "epsilon.elf")))
    if args.compile_epsilon:
        compilation_spec = importlib.util.spec_from_file_location(
            "epsilon_compilation_controls", Path(__file__).parent / "epsilon/compile_test.py")
        compilation_controls = importlib.util.module_from_spec(compilation_spec)
        compilation_spec.loader.exec_module(compilation_controls)
        result["compiled_epsilon"] = compilation_controls.run(
            work, vm, NativeCompiler, construction.realize, args.seconds)
        result["passed_stages"].append("compiled_epsilon_lexical_customer")
        checkpoint(next_stage, ())
    if args.omega:
        vm.omega_customer(work, args.seconds)
        result["passed_stages"].append("omega_fixture")
        checkpoint("complete", (("omega_request", "omega-request.bin"),
                                ("omega_observation", "omega-observation.bin"),
                                ("omega_tape", "omega.tape")))
    if args.omega_control:
        checkpoint("omega_control", ())
        vm.omega_control_customer(work, args.seconds)
        result["passed_stages"].append("omega_control")
        subjects = [("omega_bootstrap_compiler", "bootstrap-compiler.epsilon"),
                    ("omega_control_source", "control-source.epsilon"),
                    ("omega_control_program", "control-program.omg"),
                    ("omega_control_request", "control-request.bin"),
                    ("omega_control_observation", "control-observation.bin")]
        subjects.extend((f"omega_control_tape_{index}", f"control-{index}.tape")
                        for index in range(17))
        checkpoint("complete", subjects)
    if args.omega_utf8:
        checkpoint("omega_utf8", ())
        vm.omega_utf8_customer(work, args.seconds)
        result["passed_stages"].append("omega_utf8")
        subjects = [("omega_bootstrap_compiler", "bootstrap-compiler.epsilon"),
                    ("omega_utf8_source", "utf8-source.epsilon"),
                    ("omega_utf8_program", "utf8-program.omg")]
        for batch in range(4):
            subjects.extend(((f"omega_utf8_request_{batch}", f"utf8-{batch}-request.bin"),
                             (f"omega_utf8_observation_{batch}", f"utf8-{batch}-observation.bin")))
        checkpoint("complete", subjects)
    result["state"] = "passed"
    checkpoint("complete", ())
    print(json.dumps(result, sort_keys=True), flush=True)
    print("native Delta and Epsilon successor observations passed", flush=True)


if __name__ == "__main__":
    main()
