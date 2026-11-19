"""Authored-order catalog movement, prefix terminals, and exact prior payloads."""


def nominal_roundtrip(members):
    declarations = b"".join(
        b"(data " + type_name + b" (" + constructor + b" Bytes))\n"
        for type_name, constructor, helper in members
    )
    definitions = b"".join(
        b"(def " + helper + b" ((value " + type_name + b")) Bytes "
        b"(match value ((" + constructor + b" contents) contents)))\n"
        for type_name, constructor, helper in members
    )
    body = b"source"
    for type_name, constructor, helper in members:
        body = b"(" + helper + b" (" + constructor + b" " + body + b"))"
    return declarations + definitions + b"(def main ((source Bytes)) Bytes " + body + b")\n"


def fixtures(rejection):
    identity = b"(def main ((source Bytes)) Bytes source)\n"
    rejected = []
    for name, code, prefix, suffix in (
        ("type duplicate after leaving and revisiting its prefix", 6,
         b"(data TowerLong (First))\n(data A (Second))\n"
         b"(data Tower (Third))\n(data Zoo (Fourth))\n(data ",
         b"TowerLong (Fifth))\n" + identity),
        ("constructor duplicate after leaving and revisiting its prefix", 7,
         b"(data Owner (BoxLong) (A) (Box) (Zoo) (", b"BoxLong))\n" + identity),
        ("function duplicate after leaving and revisiting its prefix", 8,
         b"(def take_long () Int 1)\n(def a () Int 2)\n"
         b"(def take () Int 3)\n(def zoo () Int 4)\n(def ",
         b"take_long () Int 5)\n" + identity),
        ("type prefix remains absent after sibling insertions", 11,
         b"(data TowerLong (Box))\n(data A (Other))\n(def probe ((value ",
         b"Tower)) Int 0)\n" + identity),
        ("constructor prefix remains absent after sibling insertions", 12,
         b"(data Owner (BoxLong) (A))\n(def probe () Owner ",
         b"Box)\n" + identity),
        ("function prefix remains absent after sibling insertions", 13,
         b"(def take_long () Int 1)\n(def a () Int 2)\n(def probe () Int (",
         b"take))\n" + identity),
    ):
        rejected.append((name, prefix + suffix, rejection(code, len(prefix))))

    members = (
        (b"TowerLong", b"BoxLong", b"take_long"),
        (b"A", b"Zed", b"take_a"),
        (b"Tower", b"Box", b"take"),
        (b"Zoo", b"A", b"take_zoo"),
        (b"TowerLonger", b"BoxLonger", b"take_longer"),
    )
    long_type = b"T" + b"a" * 1023
    long_constructor = b"C" + b"a" * 1023
    long_members = (
        (long_type + b"left", long_constructor + b"left", b"left"),
        (b"A", b"A", b"other"),
        (long_type, long_constructor, b"middle"),
        (b"Z", b"Z", b"last_other"),
        (long_type + b"right", long_constructor + b"right", b"right"),
    )
    names = (b"take_long", b"a", b"take", b"zoo", b"take_longer", b"a_long")
    functions = b"".join(
        b"(def " + name + b" () Int " + str(index).encode("ascii") + b")\n"
        for index, name in enumerate(names, 1)
    )
    body = b"source"
    for index, name in enumerate(names, 1):
        body = b"(if (eq (" + name + b") " + str(index).encode("ascii") + b") " + body + b" (bytes_empty))"
    accepted = (
        ("catalogs preserve independent owners across arbitrary prefix movement",
         nominal_roundtrip(members)),
        ("catalogs revisit long prefixes without losing terminal payloads",
         nominal_roundtrip(long_members)),
        ("function catalog preserves every payload after prefix movement",
         functions + b"(def main ((source Bytes)) Bytes " + body + b")\n"),
    )
    return rejected, accepted
