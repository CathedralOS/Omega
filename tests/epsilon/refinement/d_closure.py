"""Exact D closure member customers for the refinement gate.

The subjects below are the same programs tests/epsilon/d-composition/ checks
and executes through the canonical section-11 edge: whole, unchanged D
members pinned by omega_compiler.epsilon.sources, concatenated with the
pinned customer mains under
tests/epsilon/interpreted-omega-experiment/customers/, plus the two
whole-closure customers built on the sibling gate's own entry sources. The
sizes and digests restate those bound records; a changed member or customer
refuses before any evaluator invocation rather than refining different
bytes.

Each customer carries a bounded mutation set: one spelling change per D
member across the set (every member is mutated at least once), mixing
contract-derived results — an InvalidArrayLength formation reject, two
execution-level trap discriminations, and lexical rejects placed inside each
member at its packed offset. The model re-derives every mutated program's
observation; the evaluator must publish it byte-for-byte. Lexical rejects
are placed at low offsets inside the first packed member on the
whole-closure customers so a refusal stays cheap (the lexer stops at the
defect); the deep member mutations price a single lex of the packed prefix.
"""

import hashlib
import os
from pathlib import Path

from corpus import swap


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
            f"Epsilon refinement D member {label}: identity changed to "
            f"{len(data)} bytes, {actual}")
    return data


# The bound D member identities from omega_compiler.epsilon.sources, in
# manifest order — the packed closure is their plain concatenation.
MEMBER_RECORDS = [
    ("representations.epsilon", 31993,
     "9b29b3e4c5f410ca65b158f98bd099734e3c57e177ac2f8c89c92a88ccc715ec"),
    ("request_and_utf8.epsilon", 23678,
     "fe55376ff4c64ca61a045fee84c7856d4eefdda502832b967f680517150577e5"),
    ("lexical_classification.epsilon", 2520,
     "12a3775f19ac6030bcca609acbf530ee64a09111cc24d7e941292e0d05fd996f"),
    ("lexer.epsilon", 44649,
     "e16e7a42ee0848ff56b06dff4e9900569ae57724a281c0d3bada847717412ba6"),
    ("parser.epsilon", 355292,
     "43ad5d56162ae42aaae9eba707af2d51f2098a3323b56da0e96c94913a79109b"),
    ("alpha_tape.epsilon", 30832,
     "26e943b2386e1f27761951af92d163cdaa54f32bdd990aa4694b91f10cb095a3"),
    ("scalar_compilation.epsilon", 25118,
     "e7cd1f2eed74bfaa3b380acff2f297fb2a8587cb06a4f93b6a25c77177d8ca42"),
    ("outcome.epsilon", 11252,
     "77c3848a97ee9f37d6f161b158a74f64d2eaded2d0cd2b6e8ef8d6d62264a545"),
]

PACKED_RECORD = (525334,
                 "b507fb785ea450409f3cd1c34f3c451656a124d23ecd912f9e8f8e6e45f41c64")

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

    def __init__(self, name, member_names, source, stdin, mutations):
        self.name = name
        self.member_names = member_names
        self.source = source
        self.stdin = stdin
        self.mutations = mutations


def repo_root():
    if "OMEGA_REPO_ROOT" in os.environ:
        return Path(os.environ["OMEGA_REPO_ROOT"])
    # tests/epsilon/refinement/d_closure.py -> parents[3] is the repo root.
    return Path(__file__).resolve().parents[3]


def members_dir():
    if "OMEGA_PATH_OMEGA_COMPILER" in os.environ:
        return Path(os.environ["OMEGA_PATH_OMEGA_COMPILER"])
    return repo_root() / "bootstrap/5_omega"


def load_member(directory, name):
    record = dict((n, (s, d)) for n, s, d in MEMBER_RECORDS)[name]
    return require_identity(
        name, (directory / name).read_bytes(), record[0], record[1])


def load_customer(directory, name):
    size, digest = CUSTOMER_RECORDS[name]
    return require_identity(
        f"customer {name}", (directory / name / "main.epsilon").read_bytes(),
        size, digest)


def customers():
    """The eight canonical-edge D customers, in d-composition order."""
    members = members_dir()
    customers_dir = (repo_root()
                     / "tests/epsilon/interpreted-omega-experiment/customers")
    dcomp = Path(__file__).resolve().parent.parent / "d-composition"

    representations = load_member(members, "representations.epsilon")
    request_utf8 = load_member(members, "request_and_utf8.epsilon")
    lexical = load_member(members, "lexical_classification.epsilon")
    lexer = load_member(members, "lexer.epsilon")
    alpha_tape = load_member(members, "alpha_tape.epsilon")

    cases = []

    # Packed offsets of the mutated member inside each customer source are
    # named in the mutation labels; byte_at coordinates are packed offsets.
    cases.append(Customer(
        "Omega D lexical helpers",
        ("representations.epsilon", "lexical_classification.epsilon"),
        representations + lexical + load_customer(customers_dir, "omega_lexical"),
        b"",
        [("lexical_classification: whitespace arm 9 returns 0",
          swap(b"9 -> return 1", b"9 -> return 0")),
         ("representations: zero-length bytes array",
          swap(b"bytes: [u8; 16777212]", b"bytes: [u8; 0]"))]))
    cases.append(Customer(
        "Omega D Alpha tape buffers",
        ("representations.epsilon", "alpha_tape.epsilon"),
        representations + alpha_tape
        + load_customer(customers_dir, "omega_alpha_tape"),
        b"",
        [("alpha_tape: invalid byte at packed 32023", byte_at(32023, 0x01))]))
    cases.append(Customer(
        "Omega D request and UTF-8",
        ("representations.epsilon", "request_and_utf8.epsilon"),
        representations + request_utf8
        + load_customer(customers_dir, "omega_request"),
        b"",
        [("request_and_utf8: invalid byte at packed 32143",
          byte_at(32143, 0x01))]))
    cases.append(Customer(
        "Omega D request invocation fields",
        ("representations.epsilon", "request_and_utf8.epsilon"),
        representations + request_utf8
        + load_customer(customers_dir, "omega_request_invocation"),
        b"",
        [("request_and_utf8: invalid byte at packed 32293",
          byte_at(32293, 0x01))]))
    cases.append(Customer(
        "Omega D numeric-base sums",
        ("representations.epsilon", "lexical_classification.epsilon"),
        representations + lexical
        + load_customer(customers_dir, "omega_numeric_base"),
        b"",
        [("lexical_classification: binary digit bound 49 -> 48",
          swap(b"byte <= 49", b"byte <= 48"))]))
    cases.append(Customer(
        "Omega D lexer",
        ("representations.epsilon", "request_and_utf8.epsilon",
         "lexical_classification.epsilon", "lexer.epsilon"),
        representations + request_utf8 + lexical + lexer
        + load_customer(customers_dir, "omega_lexer"),
        b"",
        [("lexer: invalid byte at packed 58391", byte_at(58391, 0x01))]))

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
        [("scalar_compilation: invalid byte at packed 489164",
          byte_at(489164, 0x01)),
         ("representations: invalid byte at packed 800",
          byte_at(800, 0x01))]))
    cases.append(Customer(
        "Omega D complete closure check only",
        all_members,
        packed + check_only_main,
        b"",
        [("parser: invalid byte at packed 103991", byte_at(103991, 0x01)),
         ("outcome: invalid byte at packed 515347", byte_at(515347, 0x01))]))
    return cases
