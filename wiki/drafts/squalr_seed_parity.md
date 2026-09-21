# SQUALR-SEED-PARITY — re-verification ledger

Board row: `TASKS.md` `**SQUALR-SEED-PARITY.**` (:7583) — resolved merged alias
of SQUALR-GEOMETRY-PARITY's "finish the mapped Rust behavior still absent from
the seed" clause, adjudicated at `a3ab15b7611`, re-verified at `59610bf809`.
Re-verified on this host (linux x86-64) at `7241e02227` by Zergling-126
(claim `0bab89d3`, draft path only).

## Submodule board carries no seed-parity row — confirmed live

`samples/apps/squalr` is an uninitialized submodule in this checkout (empty
directory per the board's own note at :7491), so the submodule was verified at
its current `main` via a shallow read-only clone of
`github.com/CathedralOS/Squalr-Omega`. Its `TASKS.md` still carries exactly
four rows — **GEOMETRY-PARITY**, **SUPPLIED-BYTES-SCAN**, **CLI-COMMANDS**,
**TARGETS-AND-THROUGHPUT** — no seed-parity item. The name re-mines the
GEOMETRY-PARITY residual list, whose enumerated gaps each already name a
sibling row on the parent board: alignment string parsing
(SQUALR-ALIGNMENT-STRING-PARSING), clone/serialization
(SQUALR-CLONE-SERIALIZATION-PARITY), region alignment/expansion
(SQUALR-REGION-ALIGNMENT-EXPANSION), named trait operators
(SQUALR-NAMED-TRAIT-OPERATORS), Rust debug-only assertions
(SQUALR-GEOMETRY-PARITY), and the Windows validation leg.

## Live fence state — confirmed via `claims.py status`

- `SQUALR-NAMED-TRAIT-OPERATORS` (Devin, exp 10:24Z) — holds the seed-port leg
  the resolved alias attributes to it.
- `SQUALR-CLI-COMMANDS` (zergling-39, exp 15:14Z) — live claim on the CLI leg.

The implementing surface `samples/apps/squalr` stays with the port's own lane
inside the submodule. No independent slice exists under this name in the parent
repo; the correct in-fence artifact is this ledger.
