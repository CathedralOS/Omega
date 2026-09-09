"""Authored normalization programs and exact application observations."""

PAYLOAD = b"A\x00\x80\xff"


def wide_pattern_source(width, digits):
    return (b"(data Wide (Wide" + b" Int" * width + b"))\n"
            b"(def select ((value Wide)) Int (match value ((Wide "
            + b" ".join(f"field{index:0{digits}}".encode() for index in range(width))
            + f") field{width - 1:0{digits}})))\n".encode()
            + b"(def main ((source Bytes)) Bytes source)\n")


def fixtures(full_width=False):
    # name, source, application status/output, extraction required, initial definitions,
    # exact maximum height when fixed, pre-normalization receipt SHA256.
    if full_width:
        # One parameter plus these pattern binders fills the active-local provision.
        return [("65535-field compiler completion", wide_pattern_source(65535, 5),
                 0, PAYLOAD, True, 3, None,
                 "15e856f8acd6429be8a1e25f516c68d3e6f06259bc368bf25e74695a99c3a668", None)]
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

    for width, name in ((128, "wide payload bindings"), (256, "last inline payload width"),
                        (257, "first shared projection width"),
                        (300, "wide constructor and payload")):
        types = b" ".join([b"Int"] * (width - 1) + [b"Bytes"])
        binders = b" ".join(f"field{index}".encode() for index in range(width))
        arguments = [b"0"] * (width - 1) + [b"source"]
        arguments[0] = b"7"
        arguments[width // 2] = b"8"
        values = b" ".join(arguments)
        source = (b"(data Wide (Empty) (Wide " + types + b"))\n"
                  b"(def extract ((value Wide)) Bytes (match value (Empty (bytes_empty)) "
                  b"((Wide " + binders + b") (if (eq field0 7) (if (eq field"
                  + str(width // 2).encode() + b" 8) field" + str(width - 1).encode()
                  + b" (bytes_empty)) (bytes_empty)))))\n"
                  b"(def main ((source Bytes)) Bytes (extract (Wide " + values + b")))\n")
        cases.append((name, source, 0, PAYLOAD, True,
                      2 + (width > 256), None, None))

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
    source = (b"(def select ((value Bytes)) Bytes " + b"(if 1 " * 1023
              + b"value" + b" value)" * 1023 + b")\n"
              b"(def main ((source Bytes)) Bytes (select source))\n")
    cases.append(("profile depth captures remain singular", source,
                  0, PAYLOAD, True, 2, None, None, 1))
    source = (b"(def score ((value Int)) Int " + b"(+ 1 " * 1023
              + b"value" + b")" * 1023 + b")\n"
              b"(def main ((source Bytes)) Bytes "
              b"(if (eq (score 1) 1024) source (bytes_empty)))\n")
    cases.append(("profile depth preserves checked arithmetic", source,
                  0, PAYLOAD, True, 2, None, None, None))
    source = wide_pattern_source(2048, 4)
    cases.append(("2048-field compiler completion", source, 0, PAYLOAD, True, 3,
                  None, "6c7956785ddd24ff99c344bb2f12ae011fe45a7d35c6c1dda8ca94af1eef6cfc", None))
    return cases
