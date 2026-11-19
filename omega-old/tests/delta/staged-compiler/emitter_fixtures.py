"""Authored emitter-context regressions; no source parsing or host lowering.

Each case is a fixed byte construction. The small all-Int receipts are authored
independently; nominal cases require repeated identical compiler output and an
exact generated-program observation instead of reproducing nominal lowering.
"""


def fixtures():
    cases = []
    source = (b"(def keep ((value Int)) Int value)\n(def main () Int "
              + b"(keep " * 248 + b"7" + b")" * 248 + b")\n")
    receipt = (b"(def __d_keep ((value Int)) Int value)\n(def main () Int "
               + b"(__d_keep " * 248 + b"7" + b")" * 248 + b")\n\n")
    cases.append(("nested ordinary calls", source, receipt, b"\x07"))

    source = b"(def main () Int " + b"(if 1 " * 250 + b"7" + b" 0)" * 250 + b")\n"
    cases.append(("nested if branches", source, source + b"\n", b"\x07"))
    source = (b"(def main () Int "
              + b"".join(f"(let value{index} Int 0 ".encode() for index in range(250))
              + b"7" + b")" * 250 + b")\n")
    cases.append(("nested distinct let bodies", source, source + b"\n", b"\x07"))

    # A single checked addition below 246 branches exercises arithmetic's
    # retained child continuation without requiring 246 expanded additions.
    prefix = b"(def main () Int " + b"(if 1 " * 246
    source = prefix + b"(+ 1 6)" + b" 0)" * 246 + b")\n"
    receipt = prefix + (
        b"(let $l1493 Int 1 (let $r1493 Int 6 (let $z1493 Int (+ $l1493 $r1493) "
        b"(if (lt 0 $r1493) (if (lt $z1493 $l1493) (/ 1 0) $z1493) "
        b"(if (lt $r1493 0) (if (lt $l1493 $z1493) (/ 1 0) $z1493) $z1493)))))"
    ) + b" 0)" * 246 + b")\n\n"
    assert len(prefix) == 1493
    cases.append(("checked arithmetic below branches", source, receipt, b"\x07"))

    fields = b" ".join([b"Int"] * 248)
    values = b" ".join([b"0"] * 247 + [b"7"])
    source = (b"(data Wide (Wide " + fields + b"))\n"
              b"(def consume ((value Wide)) Int 7)\n"
              b"(def main () Int (consume (Wide " + values + b")))\n")
    cases.append(("wide constructor fields", source, None, b"\x07"))

    source = (b"(data One (Only))\n(def main () Int "
              + b"(match Only (Only " * 128 + b"7" + b"))" * 128 + b")\n")
    cases.append(("nested nullary matches", source, None, b"\x07"))

    fields = b" ".join([b"Int"] * 64)
    binders = b" ".join(f"field{index}".encode() for index in range(64))
    values = b" ".join([b"0"] * 63 + [b"7"])
    source = (b"(data One (Only))\n(data Wide (Wide " + fields + b"))\n"
              b"(def consume ((value Wide)) Int "
              + b"(match Only (Only " * 96
              + b"(match value ((Wide " + binders + b") field63))"
              + b"))" * 96 + b")\n"
              b"(def main () Int (consume (Wide " + values + b")))\n")
    cases.append(("wide binders below matches", source, None, b"\x07"))

    source = (b"(data One (Only))\n(data Leaf (Leaf Int))\n"
              b"(data Packet (Packet Int Bytes Leaf))\n(def main () Int "
              + b"(match Only (Only " * 110
              + b"".join(
                  (f"(match (Packet 7 (bytes_single 8) (Leaf 9)) "
                   f"((Packet number{index} bytes{index} leaf{index}) ").encode()
                  for index in range(20))
              + b"(match leaf19 ((Leaf amount) (+ number19 (+ (bytes_get bytes19 0) amount))))"
              + b"))" * 20 + b"))" * 110 + b")\n")
    cases.append(("nested mixed payload matches", source, None, b"\x18"))
    return cases
