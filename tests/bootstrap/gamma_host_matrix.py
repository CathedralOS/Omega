"""Compare the Alpha and Linux hosts for each Gamma backend on the real Delta compiler."""
import argparse
import hashlib
from pathlib import Path
import struct
import time

from compiled import ROOT, execute, pack, stamp
from conformance.vectors import epsilon_observations, epsilon_syntax_controls
from independent.source_test import source as gamma_source


def produce(compiler, source, artifact, seconds):
    start = time.monotonic()
    status, data = execute(compiler, source, seconds)
    if status != 0 or not data:
        raise RuntimeError(f"{artifact.name}: compiler exit {status}, {len(data)} output bytes")
    artifact.write_bytes(data)
    print(f"{artifact.name}: {len(data)} bytes, sha256={hashlib.sha256(data).hexdigest()}, "
          f"{time.monotonic() - start:.2f}s", flush=True)
    return data


def executable(artifact, target):
    if target == "linux":
        artifact.chmod(0o700)
        return artifact
    program = artifact.with_suffix(".vm")
    stamp(artifact, program)
    return program


def delta_request(subject, support):
    return b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject)) + subject + support


def check_epsilon(row, compiler, delta_program, target, request, closure, work, seconds):
    receipt = produce(delta_program, request, work / f"{row}-epsilon.gamma", seconds)
    artifact = work / f"{row}-epsilon.bin"
    image = produce(compiler, receipt, artifact, seconds)
    evaluator = executable(artifact, target)
    for label, payload, expected in (*epsilon_observations(closure), *epsilon_syntax_controls(closure)):
        if execute(evaluator, payload, seconds) != expected:
            raise RuntimeError(f"{row}: Epsilon observation failed: {label}")
    print(f"{row}: passed five Epsilon observations and both syntax controls", flush=True)
    return hashlib.sha256(receipt).digest(), hashlib.sha256(image).digest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("alpha", type=Path, help="output of linux_x86_64/bootstrap --alpha")
    parser.add_argument("linux", type=Path, help="output of linux_x86_64/bootstrap")
    parser.add_argument("output", type=Path, help="new directory for all four rows")
    parser.add_argument("--epsilon", action="store_true", help="also construct and exercise the full Epsilon evaluator")
    parser.add_argument("--seconds", type=int, default=1800, help="timeout per invocation")
    args = parser.parse_args()
    if args.seconds <= 0:
        parser.error("--seconds must be positive")
    work = args.output.resolve()
    work.mkdir(parents=True, exist_ok=False)
    hosts = {"alpha": args.alpha.resolve(), "linux": args.linux.resolve()}
    sources = {name: gamma_source(native=name == "linux") for name in hosts}
    for name, directory in hosts.items():
        if (directory / "compiler.gamma").read_bytes() != sources[name]:
            raise ValueError(f"{directory}: compiler source differs from checkout")

    delta = (ROOT / "bootstrap/linux_x86_64/delta.gamma").read_bytes()
    support = pack("bootstrap/interpreted/3_delta/support/support.gamma.sources", work / "support.bin")
    customer = b"(def main ((source Bytes)) Bytes (bytes_concat source source))\n"
    request = delta_request(customer, support)
    if args.epsilon:
        closure = pack("bootstrap/interpreted/4_epsilon/epsilon_compiler.delta.sources", work / "epsilon.delta")
        entry = (ROOT / "tests/bootstrap/conformance/epsilon/evaluator_entry.delta").read_bytes()
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
            produce(directory / "gamma-compiler", sources[target], backend_artifact, args.seconds)
            compiler = executable(backend_artifact, host)
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
            for payload in (b"", b"A\x00B", bytes(range(256))):
                if execute(program, payload, args.seconds) != (0, payload * 2):
                    raise RuntimeError(f"{row}: wrong Delta customer observation ({len(payload)} bytes)")
            print(f"{row}: refused invalid Gamma and matched all three Delta executions", flush=True)
            if args.epsilon:
                evaluator_products[row] = check_epsilon(row, compiler, delta_program, target,
                                                         evaluator_request, closure, work, args.seconds)
        if args.epsilon and evaluator_products[f"alpha-to-{target}"] != evaluator_products[f"linux-to-{target}"]:
            raise RuntimeError(f"{target}: Epsilon receipt or executable differs between hosts")
        for name, images in (("Delta compiler", delta_images), ("compiled customer", result_images)):
            if images[f"alpha-to-{target}"] != images[f"linux-to-{target}"]:
                raise RuntimeError(f"{target}: {name} differs between hosts")
    if len(set(receipts.values())) != 1:
        raise RuntimeError("Delta compilers emitted different Gamma source for the same request")
    if args.epsilon and len({receipt for receipt, image in evaluator_products.values()}) != 1:
        raise RuntimeError("Delta compilers emitted different Epsilon evaluator receipts")
    print("Both target images match across hosts; all four Delta receipts match.")


if __name__ == "__main__":
    main()
