"""Every finite nibble pair with explicit public/helper composition."""

from .proofs import LiteralRows, diagnostic, mutation, nibble_rows, record


def cases(definitions):
    rows = LiteralRows()
    for left in range(16):
        for right in range(16):
            call, result, final, _ = nibble_rows(rows, left, right)
    # 256*(7.5+7.5+24) + index769 + final-root4 = 10757.
    yield "ordering_all_256_nibble_pairs", *diagnostic(definitions, rows, call, result, 9984)
    yield "ordering_nibble_wrong_clause", *mutation(definitions, rows, call, result, 767, record(5, rows.term(2, 52, rows.term(1, 274)), result, 15), 16, 10)
    yield "ordering_nibble_reversed", *mutation(definitions, rows, call, result, 5, record(5, rows.term(2, 37, rows.term(1, 260)), rows.term(1, 284), 2), 12)
    yield "ordering_nibble_missing_helper", *mutation(definitions, rows, call, result, 766, record(5, call, result, 16), 12)
