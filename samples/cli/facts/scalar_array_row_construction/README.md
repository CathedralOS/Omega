# Scalar array construction from fixed-array rows

A nested scalar array construction [a, b] flattens fixed-array rows into row-major element reads of their storage, preserving each row's element values across all nine places.

Owning Task: `STRUCTURAL-BORROW-IDENTITY` / `GENERIC-FIXED-ARRAY-VIEWS` ([TASKS.md](../../../../TASKS.md))
Reserved Claim Ticket: `bbdfef14137f4cb4bf31bccde1f6de91`

## Requirements and Evidence

- **Establishment:** A closed primitive nested array construction `[a, b]` where a row is a runtime `[u64; 2]` subvalue establishes one projected scalar element read per row-major leaf (`a[0]`, `a[1]`, `b[0]`, `b[1]`) rather than re-evaluating or copying the row.
- **Identity and lifetime:** Each leaf read observes the row's existing storage at its canonical place, so `result[i][j] == row_i[j]` retains exact element identity while the row keeps its single ordinary structural evaluation across all 9 places.
- **Composition:** Fixed-array rows compose the construction through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming leaf comparison; `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in one row's element carrier (`[u32; 2]` for `[u64; 2]`), leaving the literal's element type unresolved.
- **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time because a `[u32; 2]` row cannot supply `[u64; 2]` element leaves, strictly preventing carrier mismatches in flattened constructions.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Projected row leaves lower as structural scalar observations at fixed-index coordinates of the row's canonical place in canonical target-neutral `PSITERM` form, executing with identical semantics in the Terminal Psi interpreter (`omega run --both`).
