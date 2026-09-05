"""Fixed authored sources and manually derived expanded Gamma list heights."""


def fixtures():
    cases = [
        ("atom", b"(def main () Int 7)\n", (0,)),
        ("ordinary call", b"(def keep ((value Int)) Int value)\n"
         b"(def main () Int (keep 7))\n", (0, 1)),
        ("let", b"(def main () Int (let value Int 7 value))\n", (1,)),
        ("if child maximum", b"(def main () Int (if (eq 0 0) (if 1 7 0) 0))\n", (2,)),
        ("checked and raw arithmetic",
         b"(def addition () Int (+ 1 6))\n"
         b"(def subtraction () Int (- 8 1))\n"
         b"(def multiplication () Int (* 1 7))\n"
         b"(def quotient () Int (/ 7 1))\n"
         b"(def remainder () Int (% 7 8))\n"
         b"(def equal () Int (eq 7 7))\n"
         b"(def below () Int (lt 0 7))\n"
         b"(def main () Int 7)\n", (7, 7, 7, 1, 1, 1, 1, 0)),
        ("left nested checked additions",
         b"(def main () Int " + b"(+ " * 130 + b"0" + b" 0)" * 130 + b")\n",
         (136,)),
        ("right nested checked additions beyond 255",
         b"(def main () Int " + b"(+ 0 " * 130 + b"0" + b")" * 130 + b")\n",
         (265,)),
        ("calls at admitted Delta depth",
         b"(def keep ((value Int)) Int value)\n(def main () Int "
         + b"(keep " * 1023 + b"7" + b")" * 1023 + b")\n", (0, 1023)),
        ("let bodies beyond 255",
         b"(def main () Int "
         + b"".join(f"(let value{index} Int 0 ".encode() for index in range(300))
         + b"7" + b")" * 300 + b")\n", (300,)),
        ("nullary match", b"(data One (Only))\n"
         b"(def main () Int (match Only (Only 7)))\n", (1,)),
        ("constructor product spines",
         b"(data Product (Empty) (One Int) (Triple Int Int Int))\n"
         b"(def empty () Product Empty)\n"
         b"(def one () Product (One 7))\n"
         b"(def triple () Product (Triple 1 2 3))\n"
         b"(def main () Int 7)\n", (1, 1, 3, 0)),
    ]
    fields = b" ".join([b"Int"] * 128)
    values = b" ".join([b"0"] * 128)
    binders = b" ".join(f"field{index}".encode() for index in range(128))
    cases.append(("wide payload projections beyond 255",
                  b"(data Wide (Wide " + fields + b"))\n"
                  b"(def make () Wide (Wide " + values + b"))\n"
                  b"(def inspect ((value Wide)) Int (match value ((Wide "
                  + binders + b") field127)))\n(def main () Int 7)\n",
                  (128, 258, 0)))
    cases.append(("nested mixed payload and arithmetic",
                  b"(data Leaf (Leaf Int))\n(data Packet (Packet Int Bytes Leaf))\n"
                  b"(def main () Int (match (Packet 7 (bytes_single 8) (Leaf 9)) "
                  b"((Packet number bytes leaf) (match leaf "
                  b"((Leaf amount) (+ number amount))))))\n", (17,)))
    return cases
