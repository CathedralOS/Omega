"""Seven literal proof rows per byte make split/join composition explicit."""

from lexical import certificate, checked, envelope, proposition, record, rejected


def cases(definitions):
    terms = [record(1, byte + 1, 0) for byte in range(256)]
    terms.extend(record(1, 259 + nibble, 0) for nibble in range(16))
    proofs = []
    roots = []
    for byte in range(256):
        high, low = byte // 16, byte % 16
        terms.append(record(2, 22, 1, byte + 1))
        high_call = len(terms)
        terms.append(record(2, 23, 1, byte + 1))
        low_call = len(terms)
        terms.append(record(2, 21, 2, high_call, low_call))
        composed = len(terms)
        terms.append(record(2, 21, 2, 257 + high, 257 + low))
        explicit = len(terms)
        terms.append(record(2, 5 + high, 1, 257 + low))
        helper = len(terms)
        first = len(proofs) + 1
        proofs.extend((
            record(5, high_call, 257 + high, byte + 1),
            record(5, low_call, 257 + low, byte + 1),
            record(5, explicit, helper, high + 1),
            record(5, helper, byte + 1, low + 1),
            record(3, explicit, byte + 1, first + 2, first + 3),
            record(4, composed, explicit, 2, first, first + 1),
            record(3, composed, byte + 1, first + 5, first + 4),
        ))
        roots.append((composed, explicit, helper))
    owner = proposition(terms, roots[-1][0], 256)
    start = 24 + len(definitions) + len(owner) + 12
    yield "all_256_split_join_roundtrips", envelope((definitions, owner, certificate(proofs=proofs))), checked(1792, 84741)
    changed = list(proofs)
    index = 18
    first = 7 * index + 1
    changed[7 * index + 5] = record(4, roots[index][0], roots[index][1], 2, first + 1, first)
    # Per byte: four Unfold20, Trans24, Cong28, Trans24 =156 bytes.
    yield "roundtrip_swapped_congruence_premises", envelope((definitions, owner, certificate(proofs=changed))), rejected(start + 156 * index + 104 + 20)
    changed = list(proofs)
    changed[7 * 255 + 2] = record(5, roots[255][0], roots[255][2], 16)
    yield "roundtrip_missing_normalization", envelope((definitions, owner, certificate(proofs=changed))), rejected(start + 156 * 255 + 40 + 16, 10)
