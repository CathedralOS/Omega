"""Physically complete records, including deliberately invalid semantics."""

from wire import certificate, clause, envelope, example, function, layout, proposition, record, theory


def cases():
    def accepted(name, sections):
        return name, envelope(sections), layout(sections), 2, 60

    yield accepted("format_example_228_bytes", example())
    yield accepted("empty_tables_and_zero_sorts_are_not_formation", (
        theory(sorts=0), proposition(), certificate(),
    ))
    yield accepted("zero_wrong_and_forward_references_are_not_layout", (
        theory((record(0, 0),)),
        proposition((record(1, 0, 2, 2, 0), record(2, 0, 1, 2)), 999, 0),
        certificate((record(2, 0, 1, 999),), (record(1, 0, 999),)),
    ))
    templates = (record(0, 999), record(1, 0, 0), record(2, 0, 1, 0),
                 record(1, 0, 2, 0, 0), record(2, 0, 0))
    for mode in (0, 1):
        yield accepted(f"template_variants_mode_{mode}", (
            theory((record(1, 0), record(1, 2, 1, 1)), (
                function((1, 1), (clause(templates, constructor=0, body=999),),
                         mode=mode, selected=999),
            )), proposition(), certificate(),
        ))
    yield accepted("empty_clause_template_table", (
        theory(functions=(function(clauses=(clause(),)),)), proposition(), certificate(),
    ))
    yield accepted("empty_mode_one_clause_table", (
        theory(functions=(function(mode=1),)), proposition(), certificate(),
    ))
    yield accepted("multiple_clauses_physical_only", (
        theory(functions=(function(clauses=(clause(), clause()), mode=0),)),
        proposition(), certificate(),
    ))
    applications = tuple(record(tag, 0, count, *([0] * count))
                         for tag in (1, 2) for count in (0, 1, 3))
    yield accepted("both_ground_tables_application_variants", (
        theory(), proposition(applications), certificate(applications),
    ))
    proofs = (record(1, 0, 0), record(2, 0, 0, 0), record(3, 0, 0, 0, 0),
              record(4, 0, 0, 0), record(4, 0, 0, 1, 0),
              record(4, 0, 0, 3, 0, 0, 0), record(5, 0, 0, 0))
    yield accepted("five_proof_layouts_and_premise_vector_lengths", (
        theory(), proposition(), certificate(proofs=proofs),
    ))


def source_spine():
    # Literal field generation: each later authored next node names its predecessor.
    count = 46484
    terms = [record(1, 1, 0)] + [record(1, 2, 1, index) for index in range(1, count)]
    sections = (example()[0], proposition(terms, count, count), example()[2])
    request = envelope(sections)
    assert len(request) == 929848
    return "46484_row_source_spine", request, layout(sections), 1, 600
