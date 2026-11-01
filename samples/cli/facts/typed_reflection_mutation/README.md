# Exclusive typed field visitation

Parent outcome: `WIRE-CODEC-LIBRARY`, the ordinary write path needed by selected decoder operations.

Fact: authorized typed mutation lends each non-erased field exclusively to the selected callback once and restores the caller's access afterward.

Each invocation increments one field with explicit wrapping arithmetic. The caller checks all three independently changed values, including a wrapped maximum, and exactly three calls. Missing, duplicated or wrong field projections, a wrong match branch, or an un-restored caller loan fails the oracle. The const subject is explicitly copyable and is materialized into ordinary caller-owned mutable storage before editing.

All nine standard places have an excluded-field-type reject twin and actual pass/fail pins. Every positive binds all four hosted targets and requires native/Terminal exit zero. This row establishes the exclusive subloan mechanism separately from shared inspection; it grants no access beyond the lexical author's ordinary authority.
