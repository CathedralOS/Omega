"""Both symbol namespaces and every child position, in both ground tables."""

from ground_wire import ZERO, clause, failure, function, owner_row, record, theory, vector, witness_row


CONSTRUCTORS = (record(1, 0), record(2, 0), record(1, 3, 1, 2, 1))
FUNCTION = function((1, 2, 1), (clause((record(0, 0),), body=1),))
DEFINITIONS = theory(CONSTRUCTORS, (FUNCTION,), sorts=2)
OTHER = record(1, 2, 0)


def cases():
    for table in ("owner", "witness"):
        prefix = (ZERO, OTHER) if table == "owner" else (OTHER,)
        start = (owner_row(DEFINITIONS, prefix) if table == "owner"
                 else witness_row(DEFINITIONS, (ZERO,), prefix))

        def invalid(name, application, coordinate):
            owners = prefix + (application,) if table == "owner" else (ZERO,)
            witnesses = () if table == "owner" else prefix + (application,)
            return vector(f"{table}_{name}", owners, witnesses, definitions=DEFINITIONS,
                          expected=failure(coordinate))

        for tag, symbol, absent in ((1, 3, 4), (2, 1, 2)):
            for bad_symbol in (0, absent):
                yield invalid(f"tag{tag}_symbol_{bad_symbol}_before_arity",
                              record(tag, bad_symbol, 1, 0), start + 8)
            for count in (0, 2, 4):
                yield invalid(f"tag{tag}_arity_{count}_before_children",
                              record(tag, symbol, count, *([0] * count)), start + 12)
            for index in range(3):
                children = [1, 2, 1]
                children[index] = 0
                yield invalid(f"tag{tag}_child_{index}_reference", record(tag, symbol, 3, *children),
                              start + 16 + 4 * index)
                children[index] = 1 if index == 1 else 2
                yield invalid(f"tag{tag}_child_{index}_sort", record(tag, symbol, 3, *children),
                              start + 16 + 4 * index)
            yield invalid(f"tag{tag}_earlier_sort_before_later_reference",
                          record(tag, symbol, 3, 2, 2, 0), start + 16)
            yield invalid(f"tag{tag}_middle_reference_before_later_sort",
                          record(tag, symbol, 3, 1, 0, 2), start + 20)
    # Constructor1 and function1 have different arities. Tag is not cosmetic.
    yield vector("function_identity_cannot_use_constructor_signature", (record(2, 1, 0),),
                 definitions=DEFINITIONS, expected=failure(owner_row(DEFINITIONS) + 12))
    yield vector("constructor_identity_cannot_use_function_signature", (record(1, 1, 3, 0, 0, 0),),
                 definitions=DEFINITIONS, expected=failure(owner_row(DEFINITIONS) + 12))
