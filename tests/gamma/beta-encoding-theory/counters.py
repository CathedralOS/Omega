"""Fixed closed carry derivations, not a normalizer or Beta proof producer."""

from lexical import certificate, checked, envelope, proposition, record, rejected
from literal_rows import LiteralRows


# Each tuple states input, result, and the number of carried low bytes. The
# expected words are literal independent arithmetic examples, low byte first.
SUCCESSORS = (
    ("zero", (0, 0, 0, 0, 0, 0, 0, 0), (1, 0, 0, 0, 0, 0, 0, 0), 0),
    ("varied", (42, 17, 239, 128, 0, 254, 31, 65), (43, 17, 239, 128, 0, 254, 31, 65), 0),
    ("carry_1", (255, 17, 239, 128, 0, 254, 31, 65), (0, 18, 239, 128, 0, 254, 31, 65), 1),
    ("carry_2", (255, 255, 239, 128, 0, 254, 31, 65), (0, 0, 240, 128, 0, 254, 31, 65), 2),
    ("carry_3", (255, 255, 255, 128, 0, 254, 31, 65), (0, 0, 0, 129, 0, 254, 31, 65), 3),
    ("carry_4", (255, 255, 255, 255, 0, 254, 31, 65), (0, 0, 0, 0, 1, 254, 31, 65), 4),
    ("carry_5", (255, 255, 255, 255, 255, 254, 31, 65), (0, 0, 0, 0, 0, 255, 31, 65), 5),
    ("carry_6", (255, 255, 255, 255, 255, 255, 31, 65), (0, 0, 0, 0, 0, 0, 32, 65), 6),
    ("carry_7", (255, 255, 255, 255, 255, 255, 255, 65), (0, 0, 0, 0, 0, 0, 0, 66), 7),
    ("carry_1_alt", (255, 0, 128, 254, 19, 1, 255, 127), (0, 1, 128, 254, 19, 1, 255, 127), 1),
    ("carry_2_alt", (255, 255, 0, 254, 19, 1, 255, 127), (0, 0, 1, 254, 19, 1, 255, 127), 2),
    ("carry_3_alt", (255, 255, 255, 254, 19, 1, 255, 127), (0, 0, 0, 255, 19, 1, 255, 127), 3),
    ("carry_4_alt", (255, 255, 255, 255, 19, 1, 255, 127), (0, 0, 0, 0, 20, 1, 255, 127), 4),
    ("carry_5_alt", (255, 255, 255, 255, 255, 1, 255, 127), (0, 0, 0, 0, 0, 2, 255, 127), 5),
    ("carry_6_alt", (255, 255, 255, 255, 255, 255, 0, 127), (0, 0, 0, 0, 0, 0, 1, 127), 6),
    ("carry_7_alt", (255, 255, 255, 255, 255, 255, 255, 0), (0, 0, 0, 0, 0, 0, 0, 1), 7),
    ("highest_bit_crossing", (255, 255, 255, 255, 255, 255, 255, 127), (0, 0, 0, 0, 0, 0, 0, 128), 7),
    ("maximum_minus_one", (254, 255, 255, 255, 255, 255, 255, 255), (255, 255, 255, 255, 255, 255, 255, 255), 0),
    ("maximum_overflow", (255, 255, 255, 255, 255, 255, 255, 255), None, 8),
)


def successor_rows(values, expected, carries):
    rows = LiteralRows()
    byte = lambda value: rows.term(1, value + 1)
    word = lambda children: rows.term(1, 277, *children)
    result = lambda child: rows.term(1, 281, child)
    inputs = [byte(value) for value in values]
    public = rows.term(2, 36, word(inputs))
    output = rows.term(1, 280) if expected is None else result(word([byte(value) for value in expected]))
    current = rows.term(2, 35, *inputs)
    chain = rows.proof(5, public, current, 1)
    work = 46  # Public: row1 + clause1 + index10 + traversal18 + variables16.
    markers = {}
    for position in range(min(carries + 1, 8)):
        condition = rows.term(2, 26, inputs[position])
        increment = rows.term(2, 25, inputs[position])
        next_children = list(inputs)
        next_children[position] = increment
        pending_word = word(next_children)
        success = result(pending_word)
        carried = list(inputs)
        carried[position] = byte(0)
        fallback = rows.term(1, 280) if position == 7 else rows.term(2, 34 - position, *carried)
        choice = rows.term(2, 27, condition, success, fallback)
        is_carry = position < carries
        boolean = rows.term(1, 258 if is_carry else 257)
        normalized = rows.term(2, 27, boolean, success, fallback)
        selected = fallback if is_carry else success
        unfold = rows.proof(5, current, choice, 1)
        predicate = rows.proof(5, condition, boolean, values[position] + 1)
        success_reflexive = rows.proof(1, success, success)
        fallback_reflexive = rows.proof(1, fallback, fallback)
        congruence = rows.proof(4, choice, normalized, 3, predicate, success_reflexive, fallback_reflexive)
        selection = rows.proof(5, normalized, selected, 2 if is_carry else 1)
        normalized_step = rows.proof(3, current, normalized, unfold, congruence)
        selected_step = rows.proof(3, current, selected, normalized_step, selection)
        chain = rows.proof(3, public, selected, chain, selected_step)
        # Helper Unfold is 80 (63 at high byte); predicate is byte+6;
        # two Ref6, Cong16, choose8/9, two Trans14, chain Trans7.
        work += (63 if position == 7 else 80) + values[position] + 6 + 6 + 16 + (9 if is_carry else 8) + 14 + 7
        markers.update(choice=choice, normalized=normalized, selection=selection,
                       congruence=congruence, success_reflexive=success_reflexive,
                       predicate=predicate, selected=selected)
        if is_carry:
            inputs = carried
            current = fallback
    if expected is not None:
        incremented = byte(expected[carries])
        increment_proof = rows.proof(5, increment, incremented, values[carries] + 1)
        premises = []
        for position in range(8):
            premises.append(increment_proof if position == carries else rows.proof(1, inputs[position], inputs[position]))
        final_children = list(inputs)
        final_children[carries] = incremented
        final_word = word(final_children)
        word_congruence = rows.proof(4, pending_word, final_word, 8, *premises)
        value_congruence = rows.proof(4, success, output, 1, word_congruence)
        rows.proof(3, public, output, chain, value_congruence)
        # Byte Unfold byte+6, seven Ref21, Word Cong41, Value Cong6,
        # final Trans7. Proof index is charged separately below.
        work += values[carries] + 81
        markers.update(pending_word=pending_word, final_word=final_word,
                       word_congruence=word_congruence, increment_proof=increment_proof,
                       increment=increment, incremented=incremented)
    return rows, public, output, work + len(rows.proofs) + 5, markers


def byte_cases(definitions):
    # Four bounded requests cover all clauses of each finite byte helper.
    for function in (25, 26):
        for lower in (0, 128):
            rows = LiteralRows()
            for value in range(lower, lower + 128):
                source = rows.term(1, value + 1)
                target = rows.term(1, (value + 1) % 256 + 1 if function == 25 else 257 + int(value == 255))
                left = rows.term(2, function, source)
                rows.proof(5, left, target, value + 1)
            owner = proposition(rows.terms, left, target)
            # Each row costs byte+6; index129 and final root4 are separate.
            work = 133 + sum(value + 6 for value in range(lower, lower + 128))
            yield f"counter_byte_{function}_{lower}", envelope((definitions, owner, certificate(proofs=rows.proofs))), checked(128, work)
            changed = list(rows.proofs)
            changed[-1] = record(5, left, target, lower + 127)
            start = 24 + len(definitions) + len(owner) + 12
            yield f"counter_byte_{function}_{lower}_wrong_clause", envelope((definitions, owner, certificate(proofs=changed))), rejected(start + 127 * 20 + 16, 10)


def cases(definitions):
    yield from byte_cases(definitions)
    for name, values, expected, carries in SUCCESSORS:
        rows, public, output, work, markers = successor_rows(values, expected, carries)
        owner = proposition(rows.terms, public, output)
        yield f"counter_{name}", envelope((definitions, owner, certificate(proofs=rows.proofs))), checked(len(rows.proofs), work)
        if name not in ("carry_3", "maximum_overflow", "highest_bit_crossing"):
            continue

        def mutation(label, index, replacement, field, code=12):
            changed = list(rows.proofs)
            changed[index - 1] = replacement
            mutated_owner = proposition(rows.terms, public, output)
            start = 24 + len(definitions) + len(mutated_owner) + 12
            coordinate = start + sum(map(len, changed[:index - 1])) + field
            return f"counter_{name}_{label}", envelope((definitions, mutated_owner, certificate(proofs=changed))), rejected(coordinate, code)

        selection = markers["selection"]
        yield mutation("missing_normalization", selection, record(5, markers["choice"], markers["selected"], 2 if carries == 8 else 1), 16, 10)
        congruence = markers["congruence"]
        yield mutation("wrong_condition_premise", congruence, record(4, markers["choice"], markers["normalized"], 3, markers["success_reflexive"], markers["success_reflexive"], markers["success_reflexive"]), 20)
        yield mutation("missing_congruence", congruence, record(1, markers["choice"], markers["normalized"]), 12)
        if expected is None:
            zero_word = rows.term(1, 277, *([rows.term(1, 1)] * 8))
            wrapped = rows.term(1, 281, zero_word)
            yield mutation("wraps_maximum", selection, record(5, markers["normalized"], wrapped, 2), 12)
        else:
            yield mutation("missing_increment_normalization", markers["increment_proof"], record(1, markers["increment"], markers["incremented"]), 12)
            for slot in range(8):
                wrong = list(expected)
                wrong[slot] = values[slot] if slot <= carries else expected[slot] ^ 1
                corrupted_word = rows.term(1, 277, *(rows.term(1, value + 1) for value in wrong))
                corrupted = rows.term(1, 281, corrupted_word)
                yield mutation(f"wrong_slot_{slot}", markers["word_congruence"] + 1, record(4, markers["selected"], corrupted, 1, markers["word_congruence"]), 20)
