"""Invocation-local template memo cannot substitute a prior environment."""

from substitution_wire import IDENTITY, NAT, ZERO, compared, ordinary, record, theory, vector


def cases():
    constructors = NAT + (record(1, 0), record(1, 2, 1, 1))
    rows = (ZERO, record(2, 1, 1, 1), record(1, 3, 0), record(2, 1, 1, 3))
    yield vector("environment_changes_and_ground_memo_stays_structural",
                 compared(1, 7) + compared(0, 8) + compared(0, 13) + compared(1, 20), rows,
                 definitions=theory(constructors, (IDENTITY,)), entry="session")
    pair = ordinary((record(0, 0), record(0, 1), record(1, 4, 2, 1, 2)), (1, 1), 3)
    rows = (ZERO, ZERO, record(1, 2, 1, 1), record(1, 2, 1, 2), record(1, 3, 0),
            record(2, 1, 2, 3, 5), record(1, 4, 2, 4, 1))
    yield vector("false_template_retains_completed_ground_child",
                 compared(0, 14) + compared(1, 16) + compared(0, 28), rows,
                 definitions=theory(constructors, (pair,)), entry="retention")
