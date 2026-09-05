"""Authored-order, exact-name, and capacity controls for function lookup."""


def fixtures():
    names = [b"a", b"aa", b"aaa", b"aab", b"ab", b"b", b"ba", b"bb",
             b"z", b"a0", b"a00", b"a01", b"a1", b"a_", b"a__", b"_a"]
    declarations = [
        b"(def " + name + b" () Int " + str(index + 1).encode("ascii") + b")\n"
        for index, name in enumerate(names)
    ]
    # Calls have a fixed authored order independent of declaration placement.
    # Each distinct result exposes a wrong row even when its arity matches.
    body = b"0"
    for index in reversed(range(len(names))):
        body = (b"(let result" + str(index).encode("ascii") + b" Int (write ("
                + names[index] + b")) " + body + b")")
    main = b"(def main () Int " + body + b")\n"
    orders = (
        list(range(len(names))),
        list(reversed(range(len(names)))),
        list(range(0, len(names), 2)) + list(range(1, len(names), 2)),
    )
    cases = []
    for position, order in enumerate(orders):
        ordered = [declarations[index] for index in order]
        ordered.insert((0, len(ordered), len(ordered) // 2)[position], main)
        cases.append((f"mixed function order {position}", b"".join(ordered),
                      (0, bytes(range(1, len(names) + 1)) + b"\x00")))

    for missing in (b"_", b"aaaa", b"aac", b"a000", b"a02", b"bc", b"zz"):
        source = (b"".join(declarations)
                  + b"(def unused () Int (" + missing + b"))\n"
                  + b"(def main () Int 0)\n")
        cases.append((f"absent exact function {missing!r}", source, (1, b"")))

    marker = b"(def $application () Int 1)\n"
    helper = b"(def z () Int (/ 1 0))\n"
    call = b"(def main () Int (z))\n"
    cases.extend((
        ("first authored marker maps a helper trap", marker + helper + call,
         (249, b"")),
        ("later marker does not map a helper trap", helper + marker + call,
         (2, b"")),
        ("later marker does not authorize a pair result",
         b"(def z () Int 0)\n" + marker + b"(def main () Int (pair 0 1))\n",
         (2, b"")),
    ))

    # Reverse insertion exercises both index ends at the actual 4096-row cap.
    # The last authored duplicate must reject before requesting a fresh row.
    full = b"".join(
        f"(def f{index:04d} () Int {index % 251})\n".encode("ascii")
        for index in reversed(range(4095))
    ) + b"(def main () Int (let result Int (write (f4094)) (f0000)))\n"
    cases.extend((
        ("reverse full function census", full, (0, bytes((4094 % 251, 0)))),
        ("duplicate before full function provision",
         full + b"(def f0000 () Int 0)\n", (1, b"")),
        ("fresh function beyond reverse full census",
         full + b"(def f4095 () Int 0)\n", (3, b"")),
    ))
    return cases
