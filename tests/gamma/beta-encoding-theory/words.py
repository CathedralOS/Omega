"""Literal eight-byte word cases; these do not parse or assemble Beta source."""

from lexical import certificate, checked, envelope, proposition, record, rejected


# Values are stated low byte first, independently of the emitted clause.
WORD_BYTES = (
    (0, 0, 0, 0, 0, 0, 0, 0),
    (255, 255, 255, 255, 255, 255, 255, 255),
    (0, 0, 0, 0, 0, 0, 0, 128),
    (239, 205, 171, 137, 103, 69, 35, 1),
    (128, 255, 1, 127, 0, 16, 254, 129),
    *((0,) * slot + (1,) + (0,) * (7 - slot) for slot in range(8)),
)


def cases(definitions):
    terms = [record(1, byte + 1, 0) for byte in range(256)]
    terms.append(record(1, 278, 0))

    def byte_list(values):
        tail = 257
        for byte in reversed(values):
            terms.append(record(1, 279, 2, byte + 1, tail))
            tail = len(terms)
        return tail

    roots = []
    for values in WORD_BYTES:
        terms.append(record(1, 277, 8, *(byte + 1 for byte in values)))
        terms.append(record(2, 24, 1, len(terms)))
        left = len(terms)
        roots.append((left, byte_list(values)))
    # Additional valid ground lists pin incorrect endian order, length and slots.
    mutations = [("word_reversed_endian", 3, byte_list(tuple(reversed(WORD_BYTES[3])))),
                 ("word_seven_bytes", 3, byte_list(WORD_BYTES[3][:-1])),
                 ("word_nine_bytes", 3, byte_list((*WORD_BYTES[3], 0))),
                 ("word_high_bit_lost", 2, byte_list((0,) * 8)),
                 ("word_max_truncated", 1, byte_list((255,) * 7 + (0,)))]
    for slot in range(8):
        wrong = list(WORD_BYTES[3])
        wrong[slot] ^= 1
        mutations.append((f"word_wrong_slot_{slot}", 3, byte_list(wrong)))
    proofs = [record(5, left, right, 1) for left, right in roots]
    owner = proposition(terms, *roots[-1])
    start = 24 + len(definitions) + len(owner) + 12
    yield "all_13_fixed_word_emissions", envelope((definitions, owner, certificate(proofs=proofs))), checked(13, 928)
    for name, index, right in mutations:
        changed = list(proofs)
        changed[index] = record(5, roots[index][0], right, 1)
        yield name, envelope((definitions, owner, certificate(proofs=changed))), rejected(start + 20 * index + 12)
    changed = list(proofs)
    changed[-1] = record(5, *roots[-1], 2)
    yield "word_invalid_clause", envelope((definitions, owner, certificate(proofs=changed))), rejected(start + 20 * 12 + 16, 10)
    # Physical records remain complete, but Word admits exactly eight fields.
    # Its first ground row follows 256 Byte constants and the Nil constant.
    count_coordinate = 24 + len(definitions) + 8 + sum(map(len, terms[:257])) + 12
    for arity in (7, 9):
        changed_terms = list(terms)
        changed_terms[257] = record(1, 277, arity, *([1] * arity))
        changed_owner = proposition(changed_terms, *roots[-1])
        yield f"word_constructor_arity_{arity}", envelope((definitions, changed_owner, certificate(proofs=proofs))), rejected(count_coordinate, 8)
