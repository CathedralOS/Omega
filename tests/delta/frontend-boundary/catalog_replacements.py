"""Resolved rows preserve raw-catalog terminals, children, and authored order."""


def fixtures(rejection):
    identity = b"(def main ((source Bytes)) Bytes source)\n"
    mixed = b"(data Owner (BoxLong Bytes) (A) (Box) (Zoo Int) (BoxLonger Bytes) (Z))\n"
    rejected = []
    for name, code, prefix, suffix in (
        ("payload annotation after skipped nullary rows precedes body checking", 11,
         b"(data Owner (BoxLong Bytes) (A) (Box ",
         b"Missing) (Zoo Int))\n(def main () Int missing)\n"),
        ("first payload annotation failure precedes later replacement", 11,
         b"(data First (BoxLong ",
         b"Missing) (A))\n(data Second (Box Bytes) (Z) (Zoo LaterMissing))\n"
         + identity),
        ("later signature annotation precedes an earlier function body", 11,
         mixed + b"(def take_long () Int missing)\n(def take ((value ",
         b"Missing)) Int 0)\n" + identity),
        ("replaced payload row supplies its resolved field type", 15,
         mixed + b"(def probe () Owner (BoxLong ", b"0))\n" + identity),
    ):
        rejected.append((name, prefix + suffix, rejection(code, len(prefix))))

    accepted = (
        ("payload replacements preserve skipped nullary prefix terminals",
         mixed
         + b"(def extract ((item Owner)) Bytes (match item "
         b"((BoxLong value) value) (A (bytes_single 1)) (Box (bytes_single 2)) "
         b"((Zoo number) (bytes_single number)) ((BoxLonger value) value) "
         b"(Z (bytes_single 4))))\n"
         b"(def main ((source Bytes)) Bytes "
         b"(if (eq (bytes_get (extract A) 0) 1) "
         b"(if (eq (bytes_get (extract Box) 0) 2) "
         b"(if (eq (bytes_get (extract (Zoo 17)) 0) 17) "
         b"(if (eq (bytes_get (extract Z) 0) 4) "
         b"(bytes_concat (extract (BoxLong source)) "
         b"(extract (BoxLonger (bytes_empty)))) (bytes_empty)) "
         b"(bytes_empty)) (bytes_empty)) (bytes_empty)))\n"),
        ("replacement cursors preserve mixed owners and function prefix payloads",
         b"(data First (ItemLong Bytes) (Item))\n"
         b"(data Other (Away) (ItemLonger Int Bytes))\n"
         b"(data Last (Z) (Zoo Int))\n"
         b"(def take_long ((value Bytes)) Bytes value)\n"
         b"(def a ((value Int)) Int (+ value 1))\n"
         b"(def take ((value First)) Bytes "
         b"(match value ((ItemLong bytes) bytes) (Item (bytes_empty))))\n"
         b"(def zoo ((value Last)) Int (match value (Z 0) ((Zoo number) number)))\n"
         b"(def take_longer ((value Bytes)) First (ItemLong value))\n"
         b"(def other ((value Other)) Bytes (match value (Away (bytes_empty)) "
         b"((ItemLonger number bytes) (if (eq number 29) bytes (bytes_single 77)))))\n"
         b"(def main ((source Bytes)) Bytes (if (eq (a (zoo (Zoo 16))) 17) "
         b"(bytes_concat (take (take_longer (take_long source))) "
         b"(other (ItemLonger 29 (bytes_empty)))) (bytes_empty)))\n"),
    )
    return rejected, accepted
