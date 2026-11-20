"""Direct parse_file comparisons for GRAMMAR.md's named structural callable rule.

Pin the inputs by digest rather than keeping another runnable copy of the Rust
corpus. Positions below are byte offsets of the *inner* `machine`. Compare the
actual input, location, kind and expected-token payload, not just an emitted ID.
"""
from hashlib import sha256

TARGET = "tests/callable_aliases.rs"
TEST = "each_inline_signature_points_at_the_type_to_extract"
PREFIX = "inline-callable/"
EXPECTED = "a named callable type (declare its signature with `data Name = machine(...);`)"
PLACEMENTS = {
    "parameter": (20, "69f5bcaf232383a6001497a830e1ec9b08af978b4ac2066fd774f4808957afd8"),
    "const-bound": (26, "530cf90a6663d624dd7e609e8eb0fc605f5cdf6e34e290bf29713f2254782dd7"),
    "return": (16, "bd14f3f9c445c3d32aa82652c8b818bd9e7204c82b5c67b787988845e945dbdc"),
    "field": (19, "1dfac02ae46b61920c0cdcb977581310b4da984ca091b0983cf9767fc734f17a"),
    "case-field": (30, "45e86dbdd257a7e6e9fc0b571f123de8b26dae0b682ecfdaaabc6978ee2db4df"),
    "local": (26, "97d51b50a015ce0e4f788bd84cb49ea29513179f207a448a10a473e8dcec8397"),
    "array-element": (13, "2e0412bea01911eeca8291ac1fe479a0994dc1b6e833133e087f2b8616e3844b"),
    "type-argument": (22, "5b5d547246ed4ac64758df06ffdbf433ad8fbeadf6085f8a085315f9f4bc24fe"),
    "nested-return": (26, "0b8a753c35784a4340b862d08f69b9e7eea4b57653c674ca1311749d8131da71"),
    "nested-const-bound": (34, "e81ae00a9c6ab942b5f98cc711f9e8d0d36ee3e61c6b638df6554f0f09caade1"),
    "nested-parameter": (25, "ca38e1418d33291850ae6d3a95b289f3127d6f1f0166cf8c6f6cb620d3991034"),
}


def record(report, observations, suite, target, test, fields):
    key = fields[1].removeprefix(PREFIX)
    receipts = observations.setdefault("inline_callables", {})
    if key in receipts or key not in PLACEMENTS or (suite, target, test) != ("L1", TARGET, TEST) or len(fields) != 7:
        report["measurement_errors"].append(f"unknown, duplicate or misplaced callable comparison: {fields}")
        receipts[key] = None
        return
    receipts[key] = dict(zip(("source", "kind", "start", "end", "expected"), fields[2:]))


def matches(placement, receipt):
    if not isinstance(receipt, dict) or not isinstance(receipt.get("source"), str):
        return False
    start, digest = PLACEMENTS[placement]
    source = receipt["source"].encode("utf-8")
    return (sha256(source).hexdigest() == digest and source[start:start + 7] == b"machine"
            and receipt.get("kind") == "UnexpectedToken" and receipt.get("expected") == EXPECTED
            and receipt.get("start") == str(start) and receipt.get("end") == str(start + 7))


def requirements(rows, evidence, results, errors):
    entries = [row for row in rows if row["rule"] == "types"]
    if len(entries) != 1:
        errors.append("L1: expected one types entry for the inline callable comparisons")
        return
    row, = entries
    passed = results.get(("L1", TARGET, TEST)) == "ok"
    for placement in PLACEMENTS:
        name = PREFIX + placement
        verified = passed and matches(placement, evidence.get("inline_callables", {}).get(placement))
        row.setdefault("named_requirements", []).append({"kind": "parse_file comparison", "name": name,
                                                         "side": "refusals", "verified": verified})
        if verified:
            row.setdefault("refusals", []).append(name)
