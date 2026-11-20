"""Run Console output invariants on a packed Epsilon evaluator with a direct Delta interpreter."""
from argparse import ArgumentParser
from pathlib import Path
import struct
import subprocess


def program(closure):
    helpers = Path(__file__).with_name("runtime-invariants")
    source = closure + (helpers / "support.delta").read_bytes() + (helpers / "observations.delta").read_bytes()
    source += b"""
(def console_test_line ((backing EpsilonRuntimeViewBacking) (low Int) (high Int)) EpsilonEvaluatorOutcome
  (epsilon_evaluator_console_value 4
    (EpsilonRuntimeViewValue (EpsilonRuntimeViewAt backing (EpsilonU8Type 0 0) low high))
    (invariant_store) (invariant_output)))
(def console_test_element ((value EpsilonRuntimeValue)) EpsilonEvaluatorOutcome
  (console_test_line (EpsilonRuntimeSnapshotBacking
    (EpsilonRuntimeAggregate (EpsilonArrayType 0 0 (EpsilonU8Type 0 0) (EpsilonIntegerSpan 0 0 1))
      (EpsilonRuntimeIndexedChildren 1 (LeafEpsilonRuntimeArrayTree 0 value)))) 0 1))
"""
    # The observation encoder uses tag 5 for resultless, 1 for trap, and 3 for internal.
    # 'A' is output produced before the Console effect.
    internal = b"\x03" + bytes(4)
    literal = '(EpsilonRuntimeLiteralBacking (bytes_concat (bytes_single 0) (bytes_single 255)))'
    dead_place = '(EpsilonRuntimePlaceAt 7 NoEpsilonRuntimePath (EpsilonU8Type 0 0))'
    sum_case = '(EpsilonResolvedSumCaseSyntax 0 0 (EpsilonNameSpan 0 0) NoEpsilonParameters)'
    cases = [
        (f"(console_test_line {literal} 0 2)", b"\x05A\x00\xff\n"),
        (f"(console_test_line {literal} 1 2)", b"\x05A\xff\n"),
        (f"(console_test_line {literal} 2 2)", b"\x05A\n"),
        (f"(console_test_line {literal} 0 3)", internal),
        (f"(console_test_line {literal} -1 1)", internal),
        (f'(console_test_line (EpsilonRuntimePlaceBacking {dead_place}) 0 1)', internal),
        ('(console_test_element (invariant_array))', internal),
        ('(console_test_element (invariant_view))', internal),
        (f'(console_test_element (EpsilonRuntimeSumValue {sum_case} NoEpsilonRuntimePayloads))', internal),
    ]
    for value in (-1, 0, 255, 256):
        scalar = f"(EpsilonRuntimeInteger {value})"
        valid = 0 <= value <= 255
        byte = bytes([value]) if valid else b""
        cases.append((f"(console_test_element {scalar})", b"\x05A" + byte + b"\n" if valid else internal))
        cases.append((f"(epsilon_evaluator_console_value 2 {scalar} (invariant_store) (invariant_output))",
                      b"\x05A" + byte if valid else b"\x01\x05A"))
    result = '""'
    for expression, _ in reversed(cases):
        result = f"(bytes_concat (invariant_observe {expression}) {result})"
    return source + f"(def main ((input Bytes)) Bytes {result})\n".encode(), b"".join(out for _, out in cases)


if __name__ == "__main__":
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("evaluator_source", type=Path, help="packed Delta evaluator, without its entry")
    parser.add_argument("interpreter", type=Path, help="direct Delta interpreter executable")
    args = parser.parse_args()
    source, expected = program(args.evaluator_source.read_bytes())
    observation = subprocess.run([args.interpreter], input=struct.pack("<I", len(source)) + source,
                                 capture_output=True)
    if observation.returncode or observation.stderr or observation.stdout != expected:
        raise SystemExit(f"Console returned {observation.returncode}, stdout {observation.stdout.hex()}, "
                         f"stderr {observation.stderr!r}, expected {expected.hex()}")
    print("Console output observations passed")
