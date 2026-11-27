"""Shared bootstrap inputs, wire framing, and expected process observations."""
import hashlib
import struct


GAMMA_CASES = [
    ("constant", "(def main () Int 42)", b"", (0, b"*")),
    ("wrap", "(def main () Int (+ 18446744073709551615 43))", b"", (0, b"*")),
    ("signed", "(def main () Int (+ (/ -85 2) 84))", b"", (0, b"*")),
    ("modulo", "(def main () Int (+ (% -85 2) 43))", b"", (0, b"*")),
    ("character", "(def main () Int ' ')", b"", (0, b" ")),
    ("character-delimiters",
     "(def 4id ((a'b Int)) Int a'b) (def main () Int (let ((x Int (write '(')) (y Int (write ')')) "
     "(z Int (write ';')) (s Int (write '\\s'))) (4id '\\n')))", b"", (0, b"(); \n")),
    ("packed-eight", "(def main () Int (eq 'ABCDEFGH' {}))".format(int.from_bytes(b"ABCDEFGH", "little")),
     b"", (0, b"\x01")),
    ("packed-delimiters", "(def main () Int (eq ');\\n\\s' {}))".format(int.from_bytes(b");\n ", "little")),
     b"", (0, b"\x01")),
    ("near-application-marker", "(def $applicatioN () Int 42) (def main () Int ($applicatioN))",
     b"", (0, b"*")),
    ("grouped", "(def main () Int (let ((first Int 40) (second Int (+ first 2))) second))", b"", (0, b"*")),
    ("binder-let", "(def main () Int (let ((let Int 42)) let))", b"", (0, b"*")),
    ("minimum-word", "(def main () Int (+ 41 (lt -9223372036854775808 0)))", b"", (0, b"*")),
    ("cr-comment", "(def main () Int (f)); comment\r(def f () Int 42)", b"", (0, b"*")),
    ("intrinsic-prefix-calls",
     "(def secondX () Int (second (pair 0 40))) (def secondXY () Int (+ 1 (secondX))) "
     "(def secondXYZ () Int (+ 1 (secondXY))) (def main () Int (secondXYZ))",
     b"", (0, b"*")),
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
    b"(def main () Int '')", b"(def main () Int 'abcdefghi')",
    b"(def main () Int 'abcdefgh\\s')", b"(def main () Int 'a\\q')",
    b"(def 'if' () Int 0) (def main () Int 42)",
    b"(def main () Int 'a\tb')", b"(def main () Int 'unfinished)",
    b"(def main () Int 42);comment\r(def unused () Wrong 0)",
    b"(def main () Int '\t')", b"(def main () Int '\n')", b"(def main () Int '\r')",
    b"(def '\\q' () Int 0) (def main () Int 42)",
    b"(def main () Int (let '\\q' Int 6 '\\q'))",
    b"(def 'x' () Int 0) (def main () Int 42)",
    b"(def unused (('x' Int)) Int 0) (def main () Int 42)",
    b"(def main () Int (let (('x' Int 1)) 42))",
    b"(def 42 () Int 0) (def main () Int 42)",
    b"(def main () Int (let -42 Int 1 42))",
    b"(def main () Int ''')", b"(def main () Int '\\')",
]


def _epsilon_console(view, receiver):
    return b"""boundary trait Console {
  machine exit_process(return_code: i32) -> never;
  machine write_byte(value: i32);
  machine read_byte() -> i32;
  machine write_line(text: %s);
}
data Main { console: Console; }
machine Main::main(%s) {
""" % (view, receiver)


def epsilon_observations(closure):
    return _epsilon_observations(closure, _epsilon_console(b"&[u8]", b"&mut self"))


def epsilon_historical_at_observations(closure):
    # The inputs and observations originally published in 3507de61f2b8.
    return _epsilon_observations(closure, _epsilon_console(b"@[u8]", b"@mut self"))


def epsilon_input_view_cases():
    console = _epsilon_console(b"&[u8]", b"&mut self")
    source = console + b"""let original: &[u8] = self.console.remaining_input;
let first: i32 = self.console.read_byte();
let tail: &[u8] = self.console.remaining_input;
transition original.len { 0 -> empty(first, tail) _ -> report(original, first, tail) }
state empty(first: i32, tail: &[u8]) {
    transition (first == -1) && (tail.len == 0) {
        true -> self.console.exit_process(42) _ -> self.console.exit_process(9)
    }
}
state report(original: &[u8], first: i32, tail: &[u8]) {
    self.console.write_byte(original[0]);
    self.console.write_byte(first);
    self.console.write_line(tail);
    self.console.exit_process(42);
} }"""
    exited = struct.pack("<Bi", 0, 42)
    yield "empty input", source, b"", exited
    yield "view at EOF", source, b"\xff", exited + b"\xff\xff\n"
    yield "retained and advanced views", source, b"\0Z\xff", exited + b"\0\0Z\xff\n"

    source = console + b"self.console.exit_process(self.console.remaining_input[0]); }"
    yield "index empty view", source, b"", bytes((1, 6))
    source = console + b"self.console.remaining_input[0] = 42; }"
    yield "write to input view", source, b"Z", struct.pack("<BBI", 2, 20, source.index(b"self.console"))
    source = console.replace(b"console: Console;", b"console: Console; remaining_input: i32;")
    source += b"self.remaining_input = 42; self.console.exit_process(self.remaining_input); }"
    yield "authored field with same name", source, b"", exited


def epsilon_input_view_observations(closure):
    for name, source, stdin, expected in epsilon_input_view_cases():
        yield name, epsilon_request(closure, source, stdin), (0, expected)


def epsilon_syntax_controls(closure):
    for name, view, receiver in [
        ("invalid @ in view type", b"@[u8]", b"&mut self"),
        ("invalid @ in receiver", b"&[u8]", b"@mut self"),
    ]:
        source = _epsilon_console(view, receiver) + b"self.console.exit_process(42); }"
        invalid_token = b"\x02\x02" + struct.pack("<I", source.index(b"@"))
        yield name, epsilon_request(closure, source, b""), (0, invalid_token)

    console = _epsilon_console(b"&[u8]", b"&mut self")
    body = br'''self.console.write_byte(('\x42' >> 1) << 1); // ! outside a token
    self.console.write_line("\x41");
    self.console.exit_process(('a' != 'b') && (2 <= 3)); }'''
    yield "longest operators and both literal escapes", epsilon_request(closure, console + body, b""), (
        0, b"\x00" + struct.pack("<i", 1) + b"BA\n")
    for label, old, new, reason, anchor in (
        ("lone exclamation", b"!=", b"!", 2, b"! 'b'"),
        ("bad character escape", br"'\x42'", br"'\x4G'", 5, br"\x4G"),
        ("bad string escape", br'"\x41"', br'"\xG1"', 5, br"\xG1"),
    ):
        source = console + body.replace(old, new)
        rejection = struct.pack("<BBI", 2, reason, source.index(anchor))
        yield label, epsilon_request(closure, source, b""), (0, rejection)

    body = b"""self.console.write_byte(18 / 3 / 2);
self.console.write_byte(1 | 2 ^ 3 & 4);
self.console.write_byte(1 << 1 + 2 * 2);
self.console.write_byte(0 == 1 < 2);
self.console.write_byte(1 || 0 && 0);
self.console.write_byte(11 % 6 - 2);
self.console.exit_process(42); }"""
    source = console + body
    yield "infix precedence and left associativity", epsilon_request(closure, source, b""), (
        0, b"\x00" + struct.pack("<i", 42) + bytes((3, 3, 32, 0, 1, 3)))
    malformed = source.replace(b"1 + 2 * 2", b"1 + ")
    close = malformed.index(b")", malformed.index(b"1 + )"))
    yield "missing right operand before close paren", epsilon_request(closure, malformed, b""), (
        0, struct.pack("<BBI", 2, 7, close))

    source = console + br"""transition self.console.read_byte() {
        0 -> return
        '/' -> self.console.exit_process(42)
        '\xFF' -> self.console.exit_process(17)
        _ -> self.console.exit_process(9)
    } }"""
    for label, value, status in (("slash", 47, 42), ("escaped high byte", 255, 17),
                                 ("wildcard", 65, 9)):
        yield f"quoted selector {label}", epsilon_request(closure, source, bytes([value])), (
            0, struct.pack("<Bi", 0, status))

    for first, second in ((b"'/'", b"47"), (b"47", br"'\x2f'"),
                          (b"true", br"'\x01'")):
        source = (console + b"transition 0 { " + first + b" -> return "
                  + second + b" -> return } }")
        offset = source.index(second + b" -> return }")
        label = f"duplicate selectors {first!r} and {second!r}"
        yield label, epsilon_request(closure, source, b""), (
            0, struct.pack("<BBI", 2, 25, offset))


def epsilon_envelope_observations(closure):
    """Exact EEREQ v1 identity bytes and first-error coordinates."""
    header = epsilon_request(closure, b"", b"")

    def changed(*offsets):
        request = bytearray(header)
        for offset in offsets:
            request[offset] ^= 0xff
        return bytes(request)

    def refusal(code, coordinate, limit=0, requested=0):
        return 0, struct.pack("<8sBBHIQQQ", b"\xffEEOUT\x01\0", 1, 1, 0, code,
                              coordinate, limit, requested)

    yield "exact envelope admits empty source", header, (0, b"\x02\x08\0\0\0\0")
    for offset in range(8):
        yield f"incorrect identity byte {offset}", changed(offset), refusal(2, offset)
    for index in range(32):
        offset = 20 + index
        yield f"incorrect digest byte {index}", changed(offset), refusal(5, offset)

    # Supply multiple faults in the same request to exercise validation order.
    yield "short header before identity", changed(0)[:51], refusal(1, 51)
    yield "first identity byte before profile", changed(1, 7, 8), refusal(2, 1)
    yield "profile before lengths", changed(8, 15, 19), refusal(3, 8)
    yield "source length before stdin length", changed(15, 19), refusal(4, 15)
    yield "stdin length before digest", changed(19, 20), refusal(4, 19)
    yield "first digest byte before extent", changed(12, 21, 51), refusal(5, 21)
    yield "source extent before stdin extent", changed(12, 16), refusal(6, 12, 0, 255)
    yield "unexpected trailing byte", header + b"x", refusal(7, 52)


def epsilon_program_cases(console=None):
    if console is None:
        console = _epsilon_console(b"&[u8]", b"&mut self")
    return [
        ("empty source", b"", b"", b"\x02\x08\x00\x00\x00\x00"),
        ("exit", console + b"self.console.write_byte(65); self.console.exit_process(42); }", b"", b"\x00" + struct.pack("<i", 42) + b"A"),
        ("stdin", console + b"let x: i32 = self.console.read_byte(); self.console.write_byte(x); self.console.exit_process(x); }", b"Z", b"\x00" + struct.pack("<i", 90) + b"Z"),
        ("trap", console + b"self.console.write_byte(66); let x: i32 = 1 / 0; }", b"", b"\x01\x02B"),
    ]


def _epsilon_observations(closure, console):
    for name, source, stdin, expected in epsilon_program_cases(console):
        yield name, epsilon_request(closure, source, stdin), (0, expected)
    malformed = struct.pack("<8sBBHIQQQ", b"\xffEEOUT\x01\x00", 1, 1, 0, 1, 0, 0, 0)
    yield "empty EEREQ", b"", (0, malformed)


def epsilon_request(closure, source, stdin):
    return (b"EEREQ\x01\x00\x00" + struct.pack("<III", 1, len(source), len(stdin))
            + hashlib.sha256(closure).digest() + source + stdin)


def ocreq_request(source, path, *, lineage_field=None):
    """Frame the OCREQ V1 structure fixture for this gate's single-source
    program: one application package whose snapshot is exactly `path` as a
    non-executable regular file, an empty edge table, and an invocation
    selecting the alpha_bootstrap_tape product on the alpha_bootstrap
    target profile with the bound SHA-256 subject commitment."""
    def u32(value):
        return struct.pack("<I", value)

    def field(payload):
        return u32(len(payload)) + payload

    if lineage_field is None:
        workspace = hashlib.sha256(b"OCREQ test workspace").digest()
        lineage_field = field(b"\x03" + workspace + field(b"program"))
    snapshot_row = u32(2) + field(path) + u32(0) + field(source)
    package_row = (
        field(b"program")        # name
        + lineage_field          # framed source lineage
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

def _ocreq_lineage_payloads():
    """Wire examples and first malformed offsets relative to the lineage payload."""
    def field(text):
        return struct.pack("<I", len(text)) + text

    digest = bytes(range(32))
    repo = field(b"org/pkg")
    host = field(b"git.example")
    git = b"\x02\x00\x00" + host + b"\x00" + repo
    ssh = b"\x02\x01\x01" + field(b"git") + host + b"\x01"
    yield "github", b"\x00" + field(b"example") + field(b"pkg"), None
    yield "gitlab", b"\x01" + repo, None
    yield "git-https", git, None
    yield "git-ssh", ssh + struct.pack("<H", 2222) + repo, None
    yield "git-scp", b"\x02\x02\x01" + field(b"git") + host + b"\x00" + repo, None
    yield "workspace", b"\x03" + digest + field(b"member"), None
    yield "external-local", b"\x04" + digest + field(b"/example/pkg"), None
    yield "bundled", b"\x05" + digest + field(b"system"), None
    yield "empty", b"", 0
    yield "unassigned-family", b"\x06", 0
    yield "unassigned-transport", b"\x02\x03", 1
    yield "unassigned-user-option", b"\x02\x01\x02", 2
    yield "unassigned-port-option", b"\x02\x01\x00" + host + b"\x02", 3 + len(host)
    yield "truncated-port", ssh + b"\x16", len(ssh) + 1
    yield "truncated-digest", b"\x05" + digest[:-1], 32
    yield "high-bit-path-length", b"\x01\x00\x00\x00\x80", 1
    yield "truncated-path", b"\x01" + repo[:-1], len(repo)
    yield "trailing", git + b"\x00", len(git)


def ocreq_lineage_cases():
    """Complete requests and absolute refusal coordinates for both compilers."""
    # The envelope precedes the package count, name and outer lineage frame.
    payload_at = 16 + 4 + 4 + len(b"program") + 4
    for label, payload, bad in _ocreq_lineage_payloads():
        framed = struct.pack("<I", len(payload)) + payload
        request = ocreq_request(b"", b"program.omg", lineage_field=framed)
        yield label, request, None if bad is None else payload_at + bad
    for kind, text in [(1, b""), (2, b"git")]:
        old = struct.pack("<II", kind, len(text)) + text
        request = ocreq_request(b"", b"program.omg", lineage_field=old)
        # Kind becomes a one- or two-byte frame. Its contents cannot encode
        # a complete GitHub or workspace lineage.
        yield f"retired-origin-{kind}", request, payload_at + kind
