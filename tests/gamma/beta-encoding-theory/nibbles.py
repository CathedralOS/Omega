"""Fixed finite nibble equations and explicit three-row join derivations."""

from lexical import certificate, checked, envelope, proposition, record, rejected


def cases(definitions):
    terms = [record(1, byte + 1, 0) for byte in range(256)]
    terms.extend(record(1, 259 + nibble, 0) for nibble in range(16))
    proofs = []
    roots = []
    for high in range(16):
        for low in range(16):
            terms.append(record(2, 21, 2, 257 + high, 257 + low))
            public = len(terms)
            terms.append(record(2, 5 + high, 1, 257 + low))
            helper = len(terms)
            byte = 16 * high + low + 1
            first = len(proofs) + 1
            proofs.extend((record(5, public, helper, high + 1),
                           record(5, helper, byte, low + 1),
                           record(3, public, byte, first, first + 1)))
            roots.append((public, helper, byte))
    owner = proposition(terms, roots[-1][0], roots[-1][2])
    start = 24 + len(definitions) + len(owner) + 12
    yield "all_256_nibble_joins", envelope((definitions, owner, certificate(proofs=proofs))), checked(768, 10757)
    # Each group is two 20-byte Unfold rows and one 24-byte Trans row.
    for name, index, row, replacement, field, code in (
        ("join_public_wrong_clause", 255, 0, record(5, *roots[255][:2], 15), 16, 10),
        ("join_helper_wrong_clause", 255, 1, record(5, roots[255][1], 256, 15), 16, 10),
        ("join_wrong_byte", 255, 1, record(5, roots[255][1], 255, 16), 12, 12),
        ("join_swapped_nibbles", 18, 1, record(5, roots[18][1], 34, 3), 12, 12),
        ("join_trans_wrong_premise", 255, 2, record(3, roots[255][0], 256, 766, 765), 20, 12),
        ("join_cannot_skip_helper", 255, 0, record(5, roots[255][0], 256, 16), 12, 12),
    ):
        changed = list(proofs)
        changed[3 * index + row] = replacement
        yield name, envelope((definitions, owner, certificate(proofs=changed))), rejected(start + 64 * index + 20 * row + field, code)

    terms = terms[:272]
    proofs = []
    roots = []
    for function in (22, 23):
        for byte in range(256):
            terms.append(record(2, function, 1, byte + 1))
            right = 257 + (byte // 16 if function == 22 else byte % 16)
            roots.append((len(terms), right))
            proofs.append(record(5, len(terms), right, byte + 1))
    owner = proposition(terms, *roots[-1])
    start = 24 + len(definitions) + len(owner) + 12
    yield "all_512_byte_nibble_splits", envelope((definitions, owner, certificate(proofs=proofs))), checked(512, 68869)
    for name, index, right, clause, field, code in (
        ("split_high_wrong_nibble", 254, 270, 255, 12, 12),
        ("split_low_wrong_nibble", 511, 271, 256, 12, 12),
        ("split_high_uses_low", 18, 259, 19, 12, 12),
        ("split_low_uses_high", 274, 258, 19, 12, 12),
        ("split_zero_clause", 0, 257, 0, 16, 10),
        ("split_late_wrong_clause", 511, 272, 255, 16, 10),
    ):
        changed = list(proofs)
        changed[index] = record(5, roots[index][0], right, clause)
        yield name, envelope((definitions, owner, certificate(proofs=changed))), rejected(start + 20 * index + field, code)
