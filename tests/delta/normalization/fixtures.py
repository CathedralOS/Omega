"""Authored normalization programs and exact application observations."""

PAYLOAD = b"A\x00\x80\xff"


def fixtures():
    # name, source, application status/output, helpers required, authored count,
    # exact maximum height when fixed, pre-normalization receipt SHA256.
    cases = []
    identity = b"(def main ((source Bytes)) Bytes source)\n"
    boundary = (b"(def deep () Int " + b"(if 1 " * 255 + b"7"
                + b" 0)" * 255 + b")\n" + identity)
    cases.append(("height 255 preserves receipt", boundary, 0, PAYLOAD,
                  False, 2, 255,
                  "262c548f1a69a8880f0853ffcde625391304ddf7b7c87f06365e0ce0195625b7"))
    source = (b"(def deep () Int " + b"(if 1 " * 256 + b"7"
              + b" 0)" * 256 + b")\n"
              b"(def main ((source Bytes)) Bytes (if (eq (deep) 7) source (bytes_empty)))\n")
    cases.append(("height 256 selected computation", source, 0, PAYLOAD,
                  True, 2, None, None))
    source = (b"(def unused () Int " + b"(if 1 " * 300 + b"(/ 1 0)"
              + b" 0)" * 300 + b")\n" + identity)
    cases.append(("unused deep trapping body", source, 0, PAYLOAD,
                  True, 2, None, None))

    source = (b"(def main ((source Bytes)) Bytes (let value0 Bytes source "
              + b"".join(f"(let value{index} Bytes (bytes_empty) ".encode()
                         for index in range(1, 300))
              + b"(bytes_concat value0 value299)" + b")" * 300 + b")\n")
    cases.append(("300 lets capture earlier and later values", source, 0, PAYLOAD,
                  True, 1, None, None))
    source = (b"(def score ((value Int)) Int " + b"(+ 1 " * 130
              + b"value" + b")" * 130 + b")\n"
              b"(def main ((source Bytes)) Bytes (if (eq (score 7) 137) source (bytes_empty)))\n")
    cases.append(("nested checked arithmetic captures", source, 0, PAYLOAD,
                  True, 2, None, None))
    source = (b"(def score ((value Int)) Int " + b"(if 1 " * 250
              + b"(+ value 0)" + b" 0)" * 250 + b")\n"
              b"(def main ((source Bytes)) Bytes (if (eq (score 7) 7) source (bytes_empty)))\n")
    cases.append(("checked guard below deep branches", source, 0, PAYLOAD,
                  True, 2, None, None))

    for width, name in ((128, "wide payload bindings"), (300, "wide constructor and payload")):
        types = b" ".join([b"Int"] * (width - 1) + [b"Bytes"])
        binders = b" ".join(f"field{index}".encode() for index in range(width))
        values = b" ".join([b"0"] * (width - 1) + [b"source"])
        source = (b"(data Wide (Empty) (Wide " + types + b"))\n"
                  b"(def extract ((value Wide)) Bytes (match value (Empty (bytes_empty)) "
                  b"((Wide " + binders + b") field" + str(width - 1).encode() + b")))\n"
                  b"(def main ((source Bytes)) Bytes (extract (Wide " + values + b")))\n")
        cases.append((name, source, 0, PAYLOAD, True, 2, None, None))

    branch = b"(if 1 " * 260 + b"shared" + b" (bytes_empty))" * 260
    source = (b"(def main ((source Bytes)) Bytes (bytes_concat "
              b"(let shared Bytes source " + branch + b") "
              b"(let shared Bytes (bytes_empty) " + branch + b")))\n")
    cases.append(("disjoint same-spelling captures", source, 0, PAYLOAD,
                  True, 1, None, None))

    trap = b"(if 1 " * 300 + b"(bytes_single (/ 1 0))" + b" (bytes_empty))" * 300
    source = b"(def main ((source Bytes)) Bytes (if 0 " + trap + b" source))\n"
    cases.append(("unselected trapping branch stays lazy", source, 0, PAYLOAD,
                  True, 1, None, None))
    source = b"(def main ((source Bytes)) Bytes " + trap + b")\n"
    cases.append(("selected deep trap stays authored trap", source, 249, b"",
                  True, 1, None, None))

    source = (b"(data Choice (Zero) (One))\n(def main ((source Bytes)) Bytes "
              + b"(match One (Zero (bytes_single (/ 1 0))) (One " * 140
              + b"source" + b"))" * 140 + b")\n")
    cases.append(("generated match selectors retain captures", source, 0, PAYLOAD,
                  True, 1, None, None))

    source = (b"(def walk ((remaining Int) (value Bytes)) Bytes "
              b"(if (eq remaining 0) value "
              + b"".join(f"(let local{index} Int 0 ".encode() for index in range(300))
              + b"(walk (- remaining 1) value)" + b")" * 300 + b"))\n"
              b"(def main ((source Bytes)) Bytes (walk 1000 source))\n")
    cases.append(("tail recursion through extracted let bodies", source, 0, PAYLOAD,
                  True, 2, None, None))
    source = (b"(data One (Only))\n(data Step (Continue Int Bytes) (Done Bytes))\n"
              b"(def walk ((remaining Int) (value Bytes)) Bytes "
              b"(match (Continue remaining value) "
              b"((Continue count bytes) (if (eq count 0) bytes "
              + b"(match Only (Only " * 260
              + b"(walk (- count 1) bytes)" + b"))" * 260
              + b")) ((Done bytes) bytes)))\n"
              b"(def main ((source Bytes)) Bytes (walk 1000 source))\n")
    cases.append(("tail recursion through extracted match bodies", source, 0, PAYLOAD,
                  True, 2, None, None))
    cases = [case + (None,) for case in cases]
    source = (b"(def twice ((value Bytes)) Bytes " + b"(if 1 " * 600
              + b"(bytes_concat value value)" + b" value)" * 600 + b")\n"
              b"(def main ((source Bytes)) Bytes (twice source))\n")
    cases.append(("deduplicated captures across repeated extraction", source,
                  0, PAYLOAD + PAYLOAD, True, 2, None, None, 1))
    source = (b"(def main ((source Bytes)) Bytes (let value Bytes "
              b"(let value Bytes source " + b"(if 1 " * 600
              + b"value" + b" (bytes_empty))" * 600 + b") value))\n")
    cases.append(("same spelling inside extracted initializer", source,
                  0, PAYLOAD, True, 1, None, None, 1))
    return cases
