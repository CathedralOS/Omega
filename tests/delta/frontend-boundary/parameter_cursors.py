"""Parameter builders retain exact names and types across authored-order seeks."""


def roundtrip(names):
    longer, unrelated, prefix, other, extension = names
    parameters = b" ".join(
        b"(" + name + b" " + annotation + b")"
        for name, annotation in zip(names, (b"Int", b"Bytes", b"Box", b"Int", b"Bytes"))
    )
    body = (
        b"(if (eq " + longer + b" 17) (if (eq " + other + b" 29) "
        b"(bytes_concat " + unrelated + b" (bytes_concat "
        b"(match " + prefix + b" ((Box contents) contents)) " + extension + b")) "
        b"(bytes_empty)) (bytes_empty))"
    )
    return (
        b"(data Box (Box Bytes))\n(def transfer (" + parameters + b") Bytes " + body + b")\n"
        b"(def main ((source Bytes)) Bytes "
        b"(transfer 17 source (Box (bytes_empty)) 29 (bytes_empty)))\n"
    )


def fixtures(rejection):
    identity = b"(def main ((source Bytes)) Bytes source)\n"
    parameters = b"(towerLong Int) (a Bytes) (tower Int) (zoo Bytes)"
    rejected = []
    for name, code, prefix, suffix in (
        ("parameter duplicate after prefix movement precedes its annotation", 9,
         b"(def probe (" + parameters + b" (", b"towerLong Missing)) Int 0)\n"),
        ("parameter annotation precedes a later revisited duplicate", 11,
         b"(def probe (" + parameters + b" (fresh ",
         b"Missing) (towerLong Int)) Int 0)\n"),
        ("parameter prefix remains absent in the finished environment", 14,
         b"(def probe ((towerLong Int) (a Bytes) (zoo Int)) Int ", b"tower)\n"),
        ("parameter extension remains absent in the finished environment", 14,
         b"(def probe (" + parameters + b") Int ", b"towerLonger)\n"),
    ):
        rejected.append((name, prefix + suffix + identity, rejection(code, len(prefix))))

    long_prefix = b"p" + b"a" * 1023
    accepted = (
        ("parameter cursor preserves mixed types after arbitrary prefix movement",
         roundtrip((b"towerLong", b"a", b"tower", b"zoo", b"towerLonger"))),
        ("parameter cursor restores long prefixes and their typed payloads",
         roundtrip((long_prefix + b"left", b"a", long_prefix, b"zoo", long_prefix + b"right"))),
    )
    return rejected, accepted
