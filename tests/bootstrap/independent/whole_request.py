#!/usr/bin/env python3
"""Prepare the interpreted and compiled routes to the Omega construction application."""
import argparse
from hashlib import sha256
import json
from pathlib import Path
import runpy
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tests/bootstrap"))
from compiled import delta_request, pack, shell
from conformance.vectors import epsilon_request
from independent.epsilon.composition import compiler_source, decode_module, program_source


def write(work, name, data):
    (work / name).write_bytes(data)
    print(f"{name}: {len(data)} bytes sha256={sha256(data).hexdigest()}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="directory for construction requests")
    parser.add_argument("--receipt", type=Path,
                        help="also write a private 8 MiB input version of the canonical evaluator Gamma receipt")
    parser.add_argument("--entry-control", action="store_true",
                        help="also frame the original package graph with just its entry source unit")
    parser.add_argument("--frame-result", type=Path, metavar="OBSERVATION",
                        help="use the prepared directory and frame the Epsilon compiler output for Delta")
    args = parser.parse_args()
    work = args.output.resolve()
    if args.frame_result:
        if args.receipt or args.entry_control:
            parser.error("--frame-result uses an already prepared directory")
        try:
            module, runtime = decode_module(args.frame_result.read_bytes())
        except ValueError as error:
            parser.exit(1, str(error) + "\n")
        program = program_source(module, runtime=runtime)
        support = (work / "support.bin").read_bytes()
        write(work, "omega.dcreq", delta_request(program, support, profile=2))
        return
    work.mkdir(parents=True)
    product = work / "product.bin"
    constructor = ROOT / "source/application/construction/construct.py"
    subprocess.run([sys.executable, constructor, "--input", product], check=True)
    compiler = pack("bootstrap/omega/compiler/omega_compiler.epsilon.sources",
                    work / "omega.epsilon")
    evaluator = pack("bootstrap/epsilon/interpreter/epsilon_compiler.delta.sources",
                     work / "evaluator.delta")
    guest = compiler + (ROOT / "bootstrap/omega/construct_omega.epsilon").read_bytes()
    support = pack("bootstrap/delta/compiler/support/support.gamma.sources", work / "support.bin")
    interpreter = evaluator + (ROOT / "bootstrap/epsilon/interpreter.delta").read_bytes()
    write(work, "compiler.dcreq", delta_request(compiler_source(evaluator), support, profile=2))
    write(work, "evaluator.dcreq", delta_request(interpreter, support, profile=2))
    write(work, "whole.epsilon", guest)
    write(work, "whole.eereq", epsilon_request(evaluator, guest, product.read_bytes()))

    if args.entry_control:
        manifest = json.loads(product.with_suffix(".json").read_text())
        entry = manifest["units"][manifest["entry_unit"]]
        source = (ROOT / entry["path"]).read_bytes()
        if sha256(source).hexdigest() != entry["sha256"]:
            raise SystemExit("entry source changed after the constructor captured it")
        builder = runpy.run_path(str(constructor))
        packages = [(name, role) for name, _, role in builder["PACKAGES"]]
        stdin, _, _, _ = builder["encode"](packages, builder["EDGES"], [(entry["package"], source)],
                                            0, builder["ENTRY_NAME"])
        write(work, "entry-unit.eereq", epsilon_request(evaluator, guest, stdin))

    if args.receipt:
        receipt = args.receipt.resolve(strict=True)
        shell('. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"; '
              '. "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/evaluator_env.sh"; '
              'require_epsilon_evaluator_entry_receipt_identity "$1"', receipt)
        original = receipt.read_bytes()
        adapter = (ROOT / "bootstrap/delta/compiler/support/adapter.gamma").read_bytes()
        old = b"(lt 4194304 (input))"
        if adapter.count(old) != 1 or original.count(adapter) != 1:
            raise SystemExit("expected one bound input limit in the generated Delta adapter")
        wide = adapter.replace(old, b"(lt 8388608 (input))")
        write(work, "epsilon-8m.gamma", original.replace(adapter, wide))


if __name__ == "__main__":
    main()
