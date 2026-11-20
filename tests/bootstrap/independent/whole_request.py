#!/usr/bin/env python3
"""Prepare the interpreted and compiled routes to the Omega construction application."""
import argparse
from hashlib import sha256
import json
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tests/bootstrap"))
from compiled import delta_request, pack, shell
from conformance.vectors import epsilon_request
from independent.epsilon.composition import compiler_source, linked_program


def write(work, name, data):
    (work / name).write_bytes(data)
    print(f"{name}: {len(data)} bytes sha256={sha256(data).hexdigest()}")


def constructor_source(work):
    library = pack("bootstrap/omega/compiler/omega_compiler.epsilon.sources", work / "omega.epsilon")
    return library + (ROOT / "bootstrap/omega/construct_omega.epsilon").read_bytes()


def small_constructor_case(root):
    builder = runpy.run_path(str(root / "source/application/construction/construct.py"))
    source = b"machine main() -> u8 { 42 }\n"
    stdin, _, _, _ = builder["encode"]([("app", 0)], [], [(0, source)], 0, b"main", paths=["main.omg"])
    with tempfile.TemporaryDirectory() as temp:
        guest = constructor_source(Path(temp))
    return guest, stdin, None  # The emitted program must execute on Alpha.


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
            program = linked_program(args.frame_result.read_bytes())
        except ValueError as error:
            parser.exit(1, str(error) + "\n")
        support = (work / "support.bin").read_bytes()
        write(work, "omega.delta", program)
        write(work, "omega.dcreq", delta_request(program, support, profile=2))
        return
    work.mkdir(parents=True)
    product = work / "product.bin"
    constructor = ROOT / "source/application/construction/construct.py"
    subprocess.run([sys.executable, constructor, "--input", product], check=True)
    guest = constructor_source(work)
    evaluator = pack("bootstrap/epsilon/interpreter/epsilon_compiler.delta.sources",
                     work / "evaluator.delta")
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
        package_root = builder["PACKAGES"][entry["package"]][1]
        logical_path = Path(entry["path"]).relative_to(package_root).as_posix()
        stdin, _, _, _ = builder["encode"](packages, builder["EDGES"], [(entry["package"], source)],
                                            0, builder["ENTRY_NAME"], paths=[logical_path])
        write(work, "entry-unit.bin", stdin)
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
