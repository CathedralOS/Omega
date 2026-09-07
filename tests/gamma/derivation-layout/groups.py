"""Missing fixed groups, record boundaries, and tag-before-payload precedence."""

from wire import certificate, clause, envelope, function, nested_record, proposition, record, rejected, theory, words


def cases():
    base = (theory(), proposition(), certificate())

    def failure(name, section, value, coordinate):
        sections = list(base)
        sections[section] = value
        start = 24 + sum(map(len, sections[:section]))
        return name, envelope(sections), rejected(start + coordinate), 2, 60

    for field_count in (0, 1, 2):
        section = b"GTH1" + words(*([0] * field_count))
        yield failure(f"missing_theory_group_{field_count}", 0, section, len(section))
    for root_count in (0, 1):
        section = b"GPR1" + words(0, *([0] * root_count))
        yield failure(f"missing_proposition_roots_{root_count}", 1, section, len(section))
    yield failure("missing_ground_table_count", 1, b"GPR1", 4)
    for count_fields in (0, 1):
        section = b"GCE1" + words(*([0] * count_fields))
        yield failure(f"missing_certificate_count_{count_fields}", 2, section, len(section))

    for signature in ((), (1,)):
        section = theory((record(*signature),))
        yield failure(f"short_constructor_signature_{len(signature)}", 0,
                      section, 16 + 4 * len(signature))
        section = theory(functions=(record(*signature),))
        yield failure(f"short_function_signature_{len(signature)}", 0,
                      section, 20 + 4 * len(signature))

    function_fields = (
        ("missing_mode", (1, 0), 28),
        ("unknown_mode_before_missing_selected", (1, 0, 9), 28),
        ("missing_selected", (1, 0, 0), 32),
        ("missing_clause_count", (1, 0, 0, 0), 36),
        ("clause_count_exceeds_container", (1, 0, 0, 0, 1), 36),
        ("function_surplus", (1, 0, 0, 0, 0, 77), 40),
    )
    for name, fields, coordinate in function_fields:
        yield failure(name, 0, theory(functions=(record(*fields),)), coordinate)

    for fields in ((), (0,), (0, 0)):
        value = nested_record(words(1, 0, 0, 0, 1) + record(*fields))
        yield failure(f"short_clause_group_{len(fields)}", 0,
                      theory(functions=(value,)), 44 + len(fields) * 4)
    value = nested_record(words(1, 0, 0, 0, 1) + record(0, 0, 0, 77))
    yield failure("clause_surplus", 0, theory(functions=(value,)), 56)
    yield failure("constructor_surplus", 0, theory((record(1, 0, 77),)), 24)

    for name, fields, failure_after_tag in (
        ("empty", (), True), ("unknown_tag_only", (9,), False),
        ("ground_variable_only", (0,), False), ("application_tag_only", (1,), True),
        ("application_missing_count", (2, 0), True),
        ("application_surplus", (1, 0, 0, 77), False),
    ):
        coordinate = 12 + 4 * len(fields) if failure_after_tag else 12
        if name == "application_surplus":
            coordinate = 24
        yield failure("owner_" + name, 1, proposition((record(*fields),)), coordinate)
        yield failure("witness_" + name, 2, certificate((record(*fields),)), coordinate)

    for name, fields, coordinate in (
        ("empty", (), 56), ("unknown_tag_only", (9,), 56),
        ("variable_missing_slot", (0,), 60), ("variable_surplus", (0, 0, 77), 64),
        ("application_missing_header", (1,), 60),
        ("application_vector_extent", (2, 0, 1), 64),
    ):
        section = theory(functions=(function(clauses=(clause((record(*fields),)),)),))
        yield failure("template_" + name, 0, section, coordinate)

    for rule in range(1, 6):
        yield failure(f"proof_{rule}_missing_conclusions", 2,
                      certificate(proofs=(record(rule),)), 20)
    for name, fields, coordinate in (
        ("empty", (), 16), ("unknown_tag_only", (9,), 16),
        ("symmetry_missing_premise", (2, 0, 0), 28),
        ("transitivity_one_premise", (3, 0, 0, 0), 32),
        ("congruence_missing_count", (4, 0, 0), 28),
        ("congruence_vector_extent", (4, 0, 0, 2, 0), 28),
        ("unfolding_missing_clause", (5, 0, 0), 28),
        ("symmetry_surplus", (2, 0, 0, 0, 77), 32),
        ("transitivity_surplus", (3, 0, 0, 0, 0, 77), 36),
        ("congruence_surplus", (4, 0, 0, 0, 77), 32),
        ("unfolding_surplus", (5, 0, 0, 0, 77), 32),
    ):
        yield failure("proof_" + name, 2, certificate(proofs=(record(*fields),)), coordinate)
