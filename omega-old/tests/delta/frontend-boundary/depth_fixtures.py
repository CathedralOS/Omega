"""Authored Delta byte fixtures for expression-level parse_depth accounting.

These builders repeat explicit language forms, not parsed source. Coordinates
are lengths of authored prefixes; the compiler alone judges the resulting input.
"""

import struct


def fixtures(rejection):
    def refusal(coordinate):
        return 2, struct.pack(
            "<8sBBHIQQQ", b"\xffDCOUT\x01\x00", 2, 1, 0, 8,
            coordinate, 1024, 1025,
        )

    missing_entry = rejection(19, 0, space=0)
    header = b"(def helper ((value Int)) Int value)\n(def probe () Int "

    def calls(count, leaf=b"0"):
        return b"(helper " * count + leaf + b")" * count

    cases = []
    exact_prefix = header + b"(helper " * 1023
    cases.append(("depth exact 1024 atom", exact_prefix + b"0" + b")" * 1024,
                  missing_entry))
    cases.append(("depth exact 1024 application",
                  b"(def retain ((value Bytes)) Bytes value)\n(def probe () Bytes "
                  + b"(retain " * 1023 + b"(bytes_empty)" + b")" * 1024,
                  missing_entry))
    cases.append(("depth exact 1024 unknown atom",
                  exact_prefix + b"missing" + b")" * 1024,
                  rejection(14, len(exact_prefix))))
    over_prefix = header + b"(helper " * 1024
    for name, leaf in (("atom", b"0"), ("application", b"(helper 0)"),
                       ("malformed atom", b"-"), ("unknown atom", b"missing")):
        cases.append((f"depth adjacent 1025 {name}",
                      over_prefix + leaf + b")" * 1025,
                      refusal(len(over_prefix))))

    # Branch depth resets after the condition and after the entire first branch.
    branch = calls(1022)
    cases.append(("depth exact siblings reset",
                  header + b"(if 1 " + branch + b" " + branch + b"))",
                  missing_entry))

    # Let binding and annotation atoms do not add expression levels. Both
    # initializer and body start one level below the enclosing let expression.
    for role, before, after in (
        ("initializer", b"(let value Int ", b" value)"),
        ("body", b"(let value Int 0 ", b")"),
    ):
        cases.append((f"depth exact let {role}",
                      header + before + calls(1022) + after + b")",
                      missing_entry))
        prefix = header + before + b"(helper " * 1023
        cases.append((f"depth adjacent let {role}",
                      prefix + b"0" + b")" * 1023 + after + b")",
                      refusal(len(prefix))))

    match_header = b"(data Single (Only))\n(def probe () Int "
    match_open = b"(match Only (Only "
    cases.append(("depth exact nested match arms",
                  match_header + match_open * 1023 + b"0"
                  + b"))" * 1023 + b")", missing_entry))
    # A match's subject precedes its arms: the first refused node is Only,
    # not the deeper arm body. Arm wrappers and patterns add no levels.
    prefix = match_header + match_open * 1023 + b"(match "
    cases.append(("depth adjacent nested match subject",
                  prefix + b"Only (Only 0))" + b"))" * 1023 + b")",
                  refusal(len(prefix))))

    subject_header = (
        b"(data Single (Only))\n"
        b"(def retain ((value Single)) Single value)\n(def probe () Int (match "
    )
    cases.append(("depth exact match scrutinee",
                  subject_header + b"(retain " * 1022 + b"Only"
                  + b")" * 1022 + b" (Only 0)))", missing_entry))
    prefix = subject_header + b"(retain " * 1023
    cases.append(("depth adjacent match scrutinee",
                  prefix + b"Only" + b")" * 1023 + b" (Only 0)))",
                  refusal(len(prefix))))

    # All prior phases run before depth accounting. Within grammar work,
    # an earlier malformed expression still precedes a later refused node.
    over_source = over_prefix + b"0" + b")" * 1025
    cases.extend((
        ("depth before global duplicate",
         b"(def helper () Int 0)\n" + over_source,
         refusal(len(b"(def helper () Int 0)\n") + len(over_prefix))),
        ("later source byte before depth", over_source + b"\x00",
         rejection(3, len(over_source))),
        ("later lexical defect before depth", over_source + b" @",
         rejection(4, len(over_source) + 1)),
        ("later unmatched close before depth", over_source + b")",
         rejection(4, len(over_source))),
        ("later missing close before depth", over_source + b"(",
         rejection(4, len(over_source) + 1)),
        ("earlier grammar defect before depth",
         header + b"(if - " + calls(1023) + b" 0))",
         rejection(4, len(header) + len(b"(if "))),
    ))
    return cases
