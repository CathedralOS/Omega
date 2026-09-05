"""Full authored active-environment boundaries and immutable scope restoration."""

import struct


def outcome(tag, code, coordinate, limit=0, requested=0):
    return tag, struct.pack(
        "<8sBBHIQQQ", b"\xffDCOUT\x01\x00", tag, 1, 0, code,
        coordinate, limit, requested,
    )


def fixtures():
    parameters = tuple(
        f"(value{index:05d} Int)".encode("ascii") for index in range(65536)
    )
    full = b"(def main (" + b" ".join(parameters)
    room = b"(def main (" + b" ".join(parameters[:-1])
    full_body = full + b") Int "
    room_body = room + b") Int "
    item = b"(data Item (Item Int))\n"
    pair = b"(data Pair (Pair Int Int))\n"
    choice = b"(data Choice (First Int) (Second Int))\n"

    # Each expected coordinate is a literal authored byte offset, not a lookup
    # into compiler diagnostics. Main's deliberate schema mismatch stops the
    # two complete scope checks before lowering or generated-profile admission.
    cases = (
        ("65536 parameters reach result annotation", full + b") Missing 0)\n",
         1114135, "9830a829f0d1f95a032511ccfd8d6d949526bd207de3bded342997ac106c79d2",
         outcome(1, 11, 1114124)),
        ("65537th fresh parameter refuses its environment row",
         full + b" (value65536 Int)) Int 0)\n",
         1114148, "70fe5a6fb2307a4fc1f2407f949d67fd3702ebe2f5bd422504162abc86696691",
         outcome(2, 5, 1114124, 65536, 65537)),
        ("full parameter environment diagnoses duplicate before annotation",
         full + b" (value00000 Missing)) Int 0)\n",
         1114152, "c3ea8dd36a4dd7da248416e226fb1916c2de6c6cd77c2387966a9de04cdb99b2",
         outcome(1, 9, 1114124)),
        ("fresh parameter annotation resolves before row provision",
         full + b" (value65536 Missing)) Int 0)\n",
         1114152, "3f54da589afe53c8a647398dc45ade5f1455ff859dda2e8b62300cbe129d6a43",
         outcome(1, 11, 1114135)),
        ("full environment refuses let before its outer initializer",
         full_body + b"(let temporary Int missing temporary))\n",
         1114167, "0786de2907d942809a10a5bac90f752f49e59f7d5dce89dc85461e1f949c7db8",
         outcome(2, 5, 1114133, 65536, 65537)),
        ("let annotation precedes full-environment conflict and provision",
         full_body + b"(let value00000 Missing missing value00000))\n",
         1114173, "27a6dc0c373b7e56d6c3f56afec087e01da44325af8499b93df85c0457b6bb5f",
         outcome(1, 11, 1114144)),
        ("full environment refuses pattern binder before arm body",
         item + full_body + b"(match (Item 0) ((Item temporary) missing)))\n",
         1114196, "6d2907da6841b7aff4de6f83e726024c56435dc41a55233fbb405d1119690da7",
         outcome(2, 5, 1114174, 65536, 65537)),
        ("full pattern environment retains outer conflict diagnosis",
         item + full_body + b"(match (Item 0) ((Item value00000) missing)))\n",
         1114197, "fe711803a057489e9c5f648d5b451086acff5d5a0ec6ceeff376c0b4dcc42138",
         outcome(1, 9, 1114174)),
        ("pattern repetition precedes another row at capacity",
         pair + room_body
         + b"(match (Pair 0 0) ((Pair temporary temporary) missing)))\n",
         1114195, "8e1adb50b9dea98b7c903f5d3e928fa1a34885cb06fb9a4dfc1c9202a4fb4fcc",
         outcome(1, 10, 1114173)),
        ("initializer and sibling lets restore active row counts",
         room_body
         + b"(+ (let temporary Int (let initializer Int 0 initializer) temporary) "
         + b"(let temporary Int 0 temporary)))\n",
         1114214, "a6e43812a23f2e5228a682bcfe43951ba5241a72b6125fe1297b432a1f6c9249",
         outcome(1, 20, 5)),
        ("disjoint match arms restore active row counts",
         choice + room_body
         + b"(match (First 0) ((First temporary) temporary) "
         + b"((Second temporary) temporary)))\n",
         1114230, "bd98d394c4a6c6b889a35542d777380dad54e6f57e95fdd0908f115ea50008e1",
         outcome(1, 20, 44)),
    )
    anchors = (
        (1114124, b"Missing"), (1114124, b"value65536"),
        (1114124, b"value00000"), (1114135, b"Missing"),
        (1114133, b"temporary"), (1114144, b"Missing"),
        (1114174, b"temporary"), (1114174, b"value00000"),
        (1114173, b"temporary"), (5, b"main"), (44, b"main"),
    )
    for (_, source, _, _, _), (coordinate, spelling) in zip(cases, anchors):
        assert source[coordinate:coordinate + len(spelling)] == spelling
    return cases
