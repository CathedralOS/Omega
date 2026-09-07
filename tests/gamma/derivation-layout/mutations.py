"""Authored field corruptions and exact LAYOUT failure coordinates."""

from wire import certificate, changed_word, envelope, example, rejected, words


def cases():
    sections = example()
    original = envelope(sections)
    starts = (24, 124, 196)
    ends = (124, 196, 228)
    for start, end in zip(starts, ends):
        for offset in range(start, start + 4):
            altered = bytearray(original)
            altered[offset] ^= 1
            yield f"magic_byte_{offset}", bytes(altered), rejected(offset), 2, 60
        for offset in range(start + 7, end, 4):
            altered = bytearray(original)
            altered[offset] |= 128
            yield f"word_high_bit_{offset}", bytes(altered), rejected(offset), 2, 60

    for section in range(3):
        for remainder in (1, 2, 3):
            altered = list(sections)
            altered[section] += b"\x00" * remainder
            yield (f"partial_word_section_{section}_{remainder}", envelope(altered),
                   rejected(ends[section]), 2, 60)
        for extent in range(4):
            altered = list(sections)
            altered[section] = sections[section][:extent]
            yield (f"short_magic_section_{section}_{extent}", envelope(altered),
                   rejected(starts[section] + extent), 2, 60)

    # Absolute word coordinates in the published 228-byte example.
    mutations = (
        ("constructor_table_extent", 32, 999, 32),
        ("constructor_record_extent", 36, 999, 36),
        ("constructor_argument_extent", 44, 1, 44),
        ("function_table_extent", 64, 999, 64),
        ("function_record_extent", 68, 999, 68),
        ("function_argument_extent", 76, 99, 76),
        ("unknown_mode", 84, 2, 84),
        ("clause_table_extent", 92, 99, 92),
        ("clause_record_escape", 96, 7, 96),
        ("template_table_extent", 104, 99, 104),
        ("template_record_escape", 108, 4, 108),
        ("unknown_template_tag", 112, 3, 112),
        ("owner_table_extent", 128, 99, 128),
        ("owner_record_extent", 132, 99, 132),
        ("ground_variable_tag", 136, 0, 136),
        ("unknown_owner_tag", 136, 3, 136),
        ("owner_argument_extent", 144, 1, 144),
        ("late_owner_argument_extent", 180, 2, 180),
        ("witness_table_extent", 200, 99, 200),
        ("proof_table_extent", 204, 99, 204),
        ("proof_record_extent", 208, 99, 208),
        ("unknown_proof_tag", 212, 6, 212),
        ("proof_reflexivity_surplus", 212, 1, 224),
        ("proof_transitivity_missing_field", 212, 3, 228),
        ("congruence_premise_extent", 212, 4, 224),
    )
    for name, field, value, coordinate in mutations:
        yield name, changed_word(original, field, value), rejected(coordinate), 2, 60

    altered = changed_word(original, 84, 9)
    altered = changed_word(altered, 116, 0x80000000)
    yield "word_scan_before_earlier_unknown_mode", altered, rejected(119), 2, 60
    altered = list(sections)
    altered[0] = changed_word(altered[0], 4, 0x80000000) + b"X"
    yield "partial_word_before_earlier_high_bit", envelope(altered), rejected(124), 2, 60
    altered = changed_word(original, 84, 9)
    altered = changed_word(altered, 128, 0x80000000)
    yield "theory_grammar_before_later_section_high_bit", altered, rejected(84), 2, 60
    altered = bytearray(original)
    altered[24] ^= 1
    altered[31] |= 128
    yield "magic_before_word_scan", bytes(altered), rejected(24), 2, 60

    for section in range(3):
        altered = list(sections)
        altered[section] += words(0)
        yield (f"surplus_section_word_{section}", envelope(altered),
               rejected(ends[section]), 2, 60)
    # A valid proof prefix does not excuse the following malformed proof record.
    altered = (sections[0], sections[1], certificate(proofs=(words(3, 1, 0, 0), words(3, 6, 0, 0))))
    yield "late_proof_error_after_valid_prefix", envelope(altered), rejected(228), 2, 60
