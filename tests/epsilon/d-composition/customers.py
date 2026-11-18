"""Pinned D customers shared by literal observations and model refinement.

Each original customer executes once through the canonical Epsilon edge.
Literal expected bytes remain independent of the model; member mutations
exercise the model and edge on changed source without changing the base pins.
"""

import hashlib
import os
from pathlib import Path

from corpus import swap


# Expected Alpha echo/count tape. Offsets follow Alpha SEMANTICS.md.
ALPHA_PROGRAM = bytes.fromhex(
    "01 ff ffffffffffffffff"  # 0: imm EOF sentinel
    "01 fe 0100000000000000"  # 10: imm counter increment
    "01 fd 0000000000000000"  # 20: imm counter
    "11 00"                  # 30: read byte
    "10 00 ff 4a00000000000000"  # 32: jeq EOF -> halt at 74
    "13 4c00000000000000"     # 43: call writer at 76
    "03 fd fe"               # 52: increment counter
    "0e 00 1e00000000000000"  # 55: jnz -> read at 30
    "0c 1e00000000000000"     # 65: zero-byte back edge
    "00 fd"                  # 74: halt with count
    "12 00"                  # 76: write byte
    "14"                     # 78: ret
)


def byte_at(offset, value):
    """Replace one byte at a packed source offset."""
    def apply(source):
        return source[:offset] + bytes([value]) + source[offset + 1:]
    apply.__name__ = f"byte {offset} -> 0x{value:02x}"
    return apply


def require_identity(label, data, size, digest):
    actual = hashlib.sha256(data).hexdigest()
    if (len(data), actual) != (size, digest):
        raise SystemExit(
            f"D composition {label}: identity changed to "
            f"{len(data)} bytes, {actual}")
    return data


# The bound D member identities from omega_compiler.epsilon.sources, in
# manifest order — the packed closure is their plain concatenation.
MEMBER_RECORDS = [
    ("representations.epsilon", 38124,
     "05c94d2bc38d00b886d284494bbf2622d2c41d24d1dc7f14147c197071fa6816"),
    ("request_and_utf8.epsilon", 23678,
     "fe55376ff4c64ca61a045fee84c7856d4eefdda502832b967f680517150577e5"),
    ("lexical_classification.epsilon", 2520,
     "12a3775f19ac6030bcca609acbf530ee64a09111cc24d7e941292e0d05fd996f"),
    ("lexer.epsilon", 44432,
     "2a9c30ce9e66959bb33070937aba53e955461686bb221b128ec54801813f4c89"),
    ("parser.epsilon", 407867,
     "7e50e70d6f250fbd8279211233f512a845d69cbd2571ec235019836a5f92a514"),
    ("alpha_tape.epsilon", 30832,
     "26e943b2386e1f27761951af92d163cdaa54f32bdd990aa4694b91f10cb095a3"),
    ("scalar_compilation.epsilon", 246137,
     "f38200b3fb428c3a7d109f5d2563e43dbce318c2727b8ac79e89e098b344ace9"),
    ("outcome.epsilon", 13989,
     "a36fabd20ccecd3c15ba8dc10044567a11c0f60e92661277c11728121be65a4e"),
    ("source_set.epsilon", 6151,
     "b948dc79d0ea31ddb7a3e66e6ed5d1ef194659dd5ec8177bd5911b3a3f05ff9b"),
    ("declaration_resolution.epsilon", 25849,
     "d5327eeb7bb744f40653f17b37ef25505c92b782148070745d554ccb3a6f4329"),
]

PACKED_RECORD = (839579,
                 "d07d617005694d37c29e31b74a035ce29aef969e9b161295ae840763564d4f3c")

CUSTOMER_RECORDS = {
    "omega_lexical": (1486,
                      "6ce07453269102f7f468241d1a066a21cbe08c2a0652bb460c9c34d2f6ef11b2"),
    "omega_alpha_tape": (7274,
                         "a186166b32d38cfafdc02fc47f1ad46f2af7d18ed15b5bb0f18b0caa525bb35d"),
    "omega_request": (9510,
                      "0f838c478d6497cb4ca92d35f2aeed167b61eca4ab245847fa04da0fc2f7b0b7"),
    "omega_request_invocation": (5930,
                                 "b02e8f6f973a55ead1131a28cb939af0d6c31501d38bd7b15cba2cff4e6710bd"),
    "omega_numeric_base": (1479,
                           "abf50c23d589624d59b7b3603918d5ab76e6a6192594c50534fbee6cdf334386"),
    "omega_lexer": (6771,
                    "e4a262f1b011402970f958afbc6c950882bb75906fc7244b3ea19c8d489a0e06"),
}

# The sibling gate's own pinned entry sources and sealed stdin.
COMPOSITION_MAIN = (1759,
                    "4fb023e60c166d5700fddc343a8ee8f3242d3c2915e7a7556ec36bef19aded9b")
CHECK_ONLY_MAIN = (245,
                   "6ba150eb76208d88a764e4a90a6bdaf97d63a170ad26fd56d5016affadda0bbb")
OMEGA_SOURCE = (33,
                "7142aacc6080f4e3c41078e712ed5f4a9947c005c5d29d19b6dedcca9aa106d6")


class Customer:
    """One D-source program: source bytes, sealed stdin, covering members,
    and (label, mutation) pairs applied to the packed source."""

    def __init__(self, name, member_names, source, stdin, expected, mutations):
        self.name = name
        self.member_names = member_names
        self.source = source
        self.stdin = stdin
        self.expected = expected
        self.mutations = mutations


def repo_root():
    if "OMEGA_REPO_ROOT" in os.environ:
        return Path(os.environ["OMEGA_REPO_ROOT"])
    # tests/epsilon/d-composition/customers.py -> parents[3] is the repo root.
    return Path(__file__).resolve().parents[3]


def members_dir():
    if "OMEGA_PATH_OMEGA_COMPILER" in os.environ:
        return Path(os.environ["OMEGA_PATH_OMEGA_COMPILER"])
    return repo_root() / "bootstrap/interpreted/5_omega"


def load_member(directory, name):
    record = dict((n, (s, d)) for n, s, d in MEMBER_RECORDS)[name]
    return require_identity(
        name, (directory / name).read_bytes(), record[0], record[1])


def load_customer(directory, name):
    size, digest = CUSTOMER_RECORDS[name]
    return require_identity(
        f"customer {name}", (directory / name / "main.epsilon").read_bytes(),
        size, digest)


def member_customers():
    """The six D member programs shared by the diagnostic and canonical entries."""
    members = members_dir()
    customers_dir = (repo_root()
                     / "tests/epsilon/interpreted-omega-experiment/customers")

    representations = load_member(members, "representations.epsilon")
    request_utf8 = load_member(members, "request_and_utf8.epsilon")
    lexical = load_member(members, "lexical_classification.epsilon")
    lexer = load_member(members, "lexer.epsilon")
    alpha_tape = load_member(members, "alpha_tape.epsilon")

    cases = []

    # Packed offsets of the mutated member inside each customer source are
    # named in the mutation labels; byte_at coordinates are packed offsets.
    # Changed members rebind corruption to an actual declaration token rather
    # than preserving obsolete absolute offsets (which can now hit comments).
    # The request variants corrupt the first two bytes of the same machine
    # keyword; other probes corrupt their owner's first machine/data keyword.
    cases.append(Customer(
        "Omega D lexical helpers",
        ("representations.epsilon", "lexical_classification.epsilon"),
        representations + lexical + load_customer(customers_dir, "omega_lexical"),
        b"", b"\x00\x00\x00\x00\x00A",
        [("lexical_classification: whitespace arm 9 returns 0",
          swap(b"9 -> return 1", b"9 -> return 0")),
         ("representations: zero-length bytes array",
          swap(b"bytes: [u8; 16777212]", b"bytes: [u8; 0]"))]))
    cases.append(Customer(
        "Omega D Alpha tape buffers",
        ("representations.epsilon", "alpha_tape.epsilon"),
        representations + alpha_tape
        + load_customer(customers_dir, "omega_alpha_tape"),
        b"", b"\x00\x00\x00\x00\x00ABCDEFGH"
        b"\x0c\x09\x00\x00\x00\x00\x00\x00\x00\x14" + ALPHA_PROGRAM,
        [("alpha_tape: invalid byte at packed 38125", byte_at(38125, 0x01))]))
    cases.append(Customer(
        "Omega D request and UTF-8",
        ("representations.epsilon", "request_and_utf8.epsilon"),
        representations + request_utf8
        + load_customer(customers_dir, "omega_request"),
        b"", b"\x00\x00\x00\x00\x00A\n",
        [("request_and_utf8: invalid byte at packed 38269",
          byte_at(38269, 0x01))]))
    cases.append(Customer(
        "Omega D request invocation fields",
        ("representations.epsilon", "request_and_utf8.epsilon"),
        representations + request_utf8
        + load_customer(customers_dir, "omega_request_invocation"),
        b"", b"\x00\x00\x00\x00\x00A\n",
        [("request_and_utf8: invalid byte at packed 38270",
          byte_at(38270, 0x01))]))
    cases.append(Customer(
        "Omega D numeric-base sums",
        ("representations.epsilon", "lexical_classification.epsilon"),
        representations + lexical
        + load_customer(customers_dir, "omega_numeric_base"),
        b"", b"\x00\x00\x00\x00\x00A",
        [("lexical_classification: binary digit bound 49 -> 48",
          swap(b"byte <= 49", b"byte <= 48"))]))
    cases.append(Customer(
        "Omega D lexer",
        ("representations.epsilon", "request_and_utf8.epsilon",
         "lexical_classification.epsilon", "lexer.epsilon"),
        representations + request_utf8 + lexical + lexer
        + load_customer(customers_dir, "omega_lexer"),
        b"", b"\x00\x00\x00\x00\x00A",
        [("lexer: invalid byte at packed 64323", byte_at(64323, 0x01))]))

    # Preserve the six original composition subjects as well as their members.
    packed_records = (
        (42130, "4da97cfd36021165b6dfd609c0aca56e1242610b9444647d34a5ff0d641ff482"),
        (76230, "ebc36020f52d2154a57ad454b2331cc473cdb1aaab1a8093297ef7d25487f6b5"),
        (71312, "923d86809cc29848427abef17d5b5cd8be510c8df77bac9cdb922bbb1115b2cd"),
        (67732, "cdee3a464b0ed7b655e742b2c281cb5b0ea8e22745d3dc547a94aa36a0239ac2"),
        (42123, "12d728251bb3c0c4a8b791b63f70cb456f41ce900080095b6cceb09072b53e9b"),
        (115525, "bb2bf21fb16ddd92724598f49e515431a6e0edae1d4c61875dad20c1cbcd7dd8"),
    )
    if len(cases) != len(packed_records):
        raise SystemExit("D member customer inventory differs from its packed records")
    for case, record in zip(cases, packed_records):
        require_identity(case.name, case.source, *record)
    return cases


def customers():
    """The eight canonical-edge D customers, in d-composition order."""
    cases = member_customers()
    members = members_dir()
    dcomp = Path(__file__).resolve().parent
    # The whole bound closure in manifest order, re-pinned as the packed
    # record, plus the sibling gate's pinned entry sources.
    packed = b"".join(
        load_member(members, name) for name, _, _ in MEMBER_RECORDS)
    require_identity("packed Omega D closure", packed, *PACKED_RECORD)
    composition_main = require_identity(
        "composition main", (dcomp / "main.epsilon").read_bytes(),
        *COMPOSITION_MAIN)
    check_only_main = require_identity(
        "check-only main", (dcomp / "check_only_main.epsilon").read_bytes(),
        *CHECK_ONLY_MAIN)
    omega_source = require_identity(
        "Omega source", (dcomp / "program.omg").read_bytes(), *OMEGA_SOURCE)
    all_members = tuple(name for name, _, _ in MEMBER_RECORDS)

    cases.append(Customer(
        "Omega D complete closure",
        all_members,
        packed + composition_main,
        omega_source,
        bytes.fromhex((dcomp / "expected.hex").read_text(encoding="ascii")),
        [("scalar_compilation: invalid byte at packed 547659",
          byte_at(547659, 0x01)),
         ("representations: invalid byte at packed 754",
          byte_at(754, 0x01))]))
    cases.append(Customer(
        "Omega D complete closure check only",
        all_members,
        packed + check_only_main,
        b"", b"\x00\x00\x00\x00\x00",
        [("parser: invalid byte at packed 109979", byte_at(109979, 0x01)),
         ("outcome: invalid byte at packed 794987", byte_at(794987, 0x01))]))
    return cases
