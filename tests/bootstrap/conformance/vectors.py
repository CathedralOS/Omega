"""Shared bootstrap inputs, wire framing, and expected process observations."""
import hashlib
import struct


GAMMA_CASES = [
    ("constant", "(def main () Int 42)", b"", (0, b"*")),
    ("wrap", "(def main () Int (+ 18446744073709551615 43))", b"", (0, b"*")),
    ("signed", "(def main () Int (+ (/ -85 2) 84))", b"", (0, b"*")),
    ("modulo", "(def main () Int (+ (% -85 2) 43))", b"", (0, b"*")),
    ("character", "(def main () Int ' ')", b"", (0, b" ")),
    ("grouped", "(def main () Int (let ((first Int 40) (second Int (+ first 2))) second))", b"", (0, b"*")),
    ("binder-let", "(def main () Int (let ((let Int 42)) let))", b"", (0, b"*")),
    ("minimum-word", "(def main () Int (+ 41 (lt -9223372036854775808 0)))", b"", (0, b"*")),
    ("cr-comment", "(def main () Int (f)); comment\r(def f () Int 42)", b"", (0, b"*")),
    ("nested-pair", "(def main () Int (first (second (pair 0 (pair 42 9)))))", b"", (0, b"*")),
    ("order", "(def f ((a Int) (b Int)) Int (+ a b)) (def main () Int (f (write 20) (write 22)))", b"", (0, bytes([20,22,42]))),
    ("sealed", "(def main () Int (+ (input) (read 1)))", b"AB", (0, b"D")),
    ("recursive", "(def sum ((n Int)) Int (if (eq n 0) 0 (+ n (sum (- n 1))))) (def main () Int (sum 7))", b"", (0, bytes([28]))),
    ("tail-swap", "(def loop ((n Int) (a Int) (b Int)) Int (if (eq n 0) a (loop (- n 1) b a))) (def main () Int (loop 100001 41 42))", b"", (0, b"*")),
    ("tail-arity", "(def one ((n Int)) Int (if (eq n 0) 42 (two (- n 1) 9))) (def two ((n Int) (x Int)) Int (one n)) (def main () Int (one 10000))", b"", (0, b"*")),
    ("tail-nested", "(def f ((n Int)) Int (if (eq n 0) 40 (f (- n 1)))) (def main () Int (let x Int 2 (+ (f 100) x)))", b"", (0, b"*")),
    ("tail-pair", "(def f ((p Int) (n Int)) Int (if (eq n 0) p (g n p 7))) (def g ((n Int) (p Int) (x Int)) Int (let next Int (- n 1) (f p next))) (def main () Int (first (f (pair 42 43) 100)))", b"", (0, b"*")),
    ("bad-projection", "(def main () Int (first 42))", b"", (2, b"")),
    ("tagged-condition", "(def main () Int (if (pair 0 0) 42 41))", b"", (2, b"")),
    ("tagged-arithmetic", "(def main () Int (+ (pair 0 0) 42))", b"", (2, b"")),
    ("tagged-io", "(def main () Int (write (pair 0 0)))", b"", (2, b"")),
    ("pair-main", "(def main () Int (pair 0 1))", b"", (2, b"")),
    ("atomic", "(def main () Int (let x Int (write 42) (/ x 0)))", b"", (2, b"")),
    ("divide-overflow", "(def main () Int (/ -9223372036854775808 -1))", b"", (2, b"")),
    ("mod-overflow", "(def main () Int (% -9223372036854775808 -1))", b"", (2, b"")),
    ("read-negative", "(def main () Int (read -1))", b"A", (2, b"")),
    ("read-end", "(def main () Int (read 1))", b"A", (2, b"")),
    ("write-range", "(def main () Int (write 256))", b"", (2, b"")),
    ("app-publish", "(def $application () Int 1) (def main () Int (let x Int (write 42) (pair 7 1)))", b"", (7, b"*")),
    ("app-discard", "(def $application () Int 1) (def main () Int (let x Int (write 42) (pair 7 0)))", b"", (7, b"")),
    ("app-trap", "(def $application () Int 1) (def main () Int (let x Int (write 42) (pair 0 0)))", b"", (249, b"")),
    ("app-empty-error", "(def $application () Int 1) (def main () Int (pair 7 1))", b"", (249, b"")),
]

INVALID_GAMMA = [
    b"(def main () Int 42", b"(def main () Int 42)\x00",
    b"(def main () Int 42) (def main () Int 43)",
    b"(def main ((x Int)) Int x)", b"(def main () Bytes 42)",
    b"(def main () Int (if 1 42 unknown))",
    b"(def main () Int 42) (def unused () Int (missing 1))",
    b"(def main () Int (let x Int x x))",
    b"(def main () Int (let ((x Int 1) (x Int 2)) x))",
    b"(def f ((x Int) (x Int)) Int x) (def main () Int 42)",
    b"(def f ((x Int)) Int x) (def main () Int (f))",
    b"(def main () Int (+ 1))",
    b"(def main () Int 42);comment\r(def unused () Wrong 0)",
    b"(def main () Int ''')", b"(def main () Int '\\')",
]


def epsilon_observations(closure):
    console = b"""boundary trait Console {
  machine exit_process(return_code: i32) -> never;
  machine write_byte(value: i32);
  machine read_byte() -> i32;
  machine write_line(text: @[u8]);
}
data Main { console: Console; }
machine Main::main(@mut self) {
"""
    cases = [
        ("empty source", b"", b"", b"\x02\x08\x00\x00\x00\x00"),
        ("exit", console + b"self.console.write_byte(65); self.console.exit_process(42); }", b"", b"\x00" + struct.pack("<i", 42) + b"A"),
        ("stdin", console + b"let x: i32 = self.console.read_byte(); self.console.write_byte(x); self.console.exit_process(x); }", b"Z", b"\x00" + struct.pack("<i", 90) + b"Z"),
        ("trap", console + b"self.console.write_byte(66); let x: i32 = 1 / 0; }", b"", b"\x01\x02B"),
    ]
    for name, source, stdin, expected in cases:
        yield name, epsilon_request(closure, source, stdin), (0, expected)
    malformed = struct.pack("<8sBBHIQQQ", b"\xffEEOUT\x01\x00", 1, 1, 0, 1, 0, 0, 0)
    yield "empty EEREQ", b"", (0, malformed)


def epsilon_request(closure, source, stdin):
    return (b"EEREQ\x01\x00\x00" + struct.pack("<III", 1, len(source), len(stdin))
            + hashlib.sha256(closure).digest() + source + stdin)


def ocreq_request(source, path):
    """Frame the canonical OCREQ V1 request for this gate's single-source
    program: one application package whose snapshot is exactly `path` as a
    non-executable regular file, an empty edge table, and an invocation
    selecting the alpha_bootstrap_tape product on the alpha_bootstrap
    target profile with the bound SHA-256 subject commitment."""
    def u32(value):
        return struct.pack("<I", value)

    def field(payload):
        return u32(len(payload)) + payload

    snapshot_row = u32(2) + field(path) + u32(0) + field(source)
    package_row = (
        field(b"program")        # name
        + u32(1)                 # lineage_kind: external_local
        + field(b"")             # lineage
        + field(b"")             # revision
        + field(b"")             # tree
        + field(b"")             # content
        + field(b"")             # member projection
        + u32(2)                 # role: application
        + u32(1) + snapshot_row  # snapshot table
    )
    subject = u32(1) + package_row + u32(0) + u32(0) + u32(2)
    commitment = hashlib.sha256(
        b"omega.ocreq.subject.sha256.v1\x00" + subject).digest()
    invocation = u32(4) + field(b"alpha_bootstrap") + u32(0) + commitment
    return (b"OCREQ\x01\x00\x00" + u32(len(subject)) + u32(len(invocation))
            + subject + invocation)
