"""Compare the Alpha and Linux hosts for each Gamma backend on the real Delta compiler."""
import argparse
import hashlib
from pathlib import Path

from compiled import (ROOT, add_invocation_budget, delta_request, executable, execute, gamma_backend_source,
                      gamma_program, gamma_request, pack, pack_delta_compiler, produce, shell)
from conformance.vectors import epsilon_observations, epsilon_request, epsilon_syntax_controls


def check_epsilon(row, compiler, delta_program, target, request, closure, work, seconds):
    receipt = produce(delta_program, request, work / f"{row}-epsilon.gamma", seconds)
    artifact = work / f"{row}-epsilon.bin"
    image = produce(compiler, receipt, artifact, seconds)
    evaluator = executable(artifact, target)
    for label, payload, expected in (*epsilon_observations(closure), *epsilon_syntax_controls(closure)):
        if execute(evaluator, payload, seconds) != expected:
            raise RuntimeError(f"{row}: Epsilon observation failed: {label}")
    print(f"{row}: passed five Epsilon observations and both syntax controls", flush=True)
    return receipt, image


def check_product_word(work, closure, seconds):
    compiler = pack("bootstrap/omega/compiler/omega_compiler.epsilon.sources", work / "omega.epsilon")
    fixtures = ROOT / "tests/bootstrap/conformance/omega"
    guest = compiler + (fixtures / "main.epsilon").read_bytes()
    word = (ROOT / "source/foundation/wire/codec.omg").read_bytes()
    subject = word + b"\n" + (fixtures / "wire_word.omg").read_bytes()
    request = epsilon_request(closure, guest, subject)
    (work / "wire.eereq").write_bytes(request)
    print(f"Wire decoder: {len(request)} request bytes, sha256={hashlib.sha256(request).hexdigest()}", flush=True)
    observations = []
    # The matrix has already checked both constructions of each target evaluator.
    evaluators = {"alpha": "alpha-to-alpha-epsilon.vm", "linux": "linux-to-linux-epsilon.bin"}
    for target, evaluator in evaluators.items():
        print(f"Running the wire decoder compiler on {target}", flush=True)
        output = produce(work / evaluator, request, work / f"{target}-wire.observation", seconds)
        if output[:5] != bytes(5) or len(output) == 5:
            raise RuntimeError(f"{target}: no successful wire compiler output, prefix {output[:40].hex()}")
        observations.append(output)
    if observations[0] != observations[1]:
        raise RuntimeError("Epsilon evaluators produced different wire decoder tapes")
    tape = work / "wire.tape"
    tape.write_bytes(observations[0][5:])
    if execute(executable(tape, "alpha"), timeout=seconds) != (42, b""):
        raise RuntimeError("wire decoder failed its little-endian word checks")
    print(f"Product wire decoder: {tape.stat().st_size} identical Alpha bytes, exit 42.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("alpha", type=Path, help="output of tests/bootstrap/linux_x86_64/bootstrap --alpha")
    parser.add_argument("linux", type=Path, help="output of tests/bootstrap/linux_x86_64/bootstrap")
    parser.add_argument("output", type=Path, help="new directory for all four rows")
    parser.add_argument("--epsilon", action="store_true", help="also construct and exercise the full Epsilon evaluator")
    parser.add_argument("--wire", action="store_true", help="also run the Omega product wire decoder on both evaluators")
    add_invocation_budget(parser)
    args = parser.parse_args()
    work = args.output.resolve()
    work.mkdir(parents=True, exist_ok=False)
    hosts = {"alpha": args.alpha.resolve(), "linux": args.linux.resolve()}
    sources = {name: gamma_backend_source(name) for name in hosts}
    for name, directory in hosts.items():
        if (directory / "compiler.gamma").read_bytes() != sources[name]:
            raise ValueError(f"{directory}: compiler source differs from checkout")

    delta = pack_delta_compiler(work)
    support = pack("bootstrap/delta/compiler/support/support.gamma.sources", work / "support.bin")
    customer = b"(def main ((source Bytes)) Bytes (bytes_concat source source))\n"
    request = delta_request(customer, support)
    inputs = (b"", b"A\x00B", bytes(range(256)))
    include_epsilon = args.epsilon or args.wire
    if include_epsilon:
        closure = pack("bootstrap/epsilon/interpreter/epsilon_compiler.delta.sources", work / "epsilon.delta")
        entry = (ROOT / "bootstrap/epsilon/interpreter.delta").read_bytes()
        evaluator_request = delta_request(closure + entry, support)
    evaluator_products = {}
    receipts = {}
    delta_images = {}
    result_images = {}
    for target in hosts:
        for host, directory in hosts.items():
            # Compile the selected backend to this host before compiling Delta.
            row = f"{host}-to-{target}"
            backend_artifact = work / f"{row}-compiler.bin"
            compiler, _ = gamma_program(host, directory / "gamma-compiler", sources[target],
                                        backend_artifact, args.seconds)
            rejected = execute(compiler, b"(def main () Int unknown)", args.seconds)
            if rejected != (249, b""):
                raise RuntimeError(f"{row}: invalid Gamma returned {rejected!r}")

            delta_artifact = work / f"{row}-delta.bin"
            delta_images[row] = produce(compiler, delta, delta_artifact, args.seconds)
            delta_program = executable(delta_artifact, target)
            receipts[row] = produce(delta_program, request, work / f"{row}-receipt.gamma", args.seconds)
            image = work / f"{row}-customer.bin"
            result_images[row] = produce(compiler, receipts[row], image, args.seconds)
            program = executable(image, target)
            for payload in inputs:
                if execute(program, payload, args.seconds) != (0, payload * 2):
                    raise RuntimeError(f"{row}: wrong Delta customer observation ({len(payload)} bytes)")
            print(f"{row}: refused invalid Gamma and matched all three Delta executions", flush=True)
            if include_epsilon:
                evaluator_products[row] = check_epsilon(row, compiler, delta_program, target,
                                                         evaluator_request, closure, work, args.seconds)
        if include_epsilon and evaluator_products[f"alpha-to-{target}"] != evaluator_products[f"linux-to-{target}"]:
            raise RuntimeError(f"{target}: Epsilon receipt or executable differs between hosts")
        for name, images in (("Delta compiler", delta_images), ("compiled customer", result_images)):
            if images[f"alpha-to-{target}"] != images[f"linux-to-{target}"]:
                raise RuntimeError(f"{target}: {name} differs between hosts")
    if len(set(receipts.values())) != 1:
        raise RuntimeError("Delta compilers emitted different Gamma source for the same request")
    receipt = receipts["alpha-to-alpha"]
    interpreted = gamma_request(receipt)
    for payload in inputs:
        if execute(hosts["alpha"] / "gamma-evaluator", interpreted + payload, args.seconds) != (0, payload * 2):
            raise RuntimeError(f"Gamma interpreter: wrong Delta customer observation ({len(payload)} bytes)")
    print("Delta customer: interpreter, Alpha tape and Linux executable matched on all three inputs.", flush=True)
    if include_epsilon:
        if len({receipt for receipt, _ in evaluator_products.values()}) != 1:
            raise RuntimeError("Delta compilers emitted different Epsilon evaluator receipts")
        shell('. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"; '
              '. "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/evaluator_env.sh"; '
              'require_epsilon_evaluator_entry_receipt_identity "$1"', work / "linux-to-linux-epsilon.gamma")
    print("Both target images match across hosts; all four Delta receipts match.", flush=True)
    if args.wire:
        check_product_word(work, closure, args.seconds)


if __name__ == "__main__":
    main()
