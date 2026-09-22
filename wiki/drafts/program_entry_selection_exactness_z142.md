# PROGRAM-ENTRY-SELECTION-EXACTNESS — scope verification (2026-09-20, `c267df86ac`)

Bare mined stub at `TASKS.md:8710`. The named clause is
`wiki/spec/build/entry_roots.md`: "The entry selection identifies an
exact free machine or, when the entry shape allows it, a machine with
one `&mut self` receiver." Verified at `c267df86ac`: the exactness
surface is landed end to end.

## Landed surface

- `build-evaluation/src/admission/selection.rs::
  select_compiler_program_entry` — exact-row selection; exactly-one
  loaded boundary schema per root slot; exactly one evaluated calling
  plan per semantic/physical requirement; `exact_boundary_entry_plan()`
  identity+custody check; one exact source unit for the physical entry
  requirement; `&mut self` receiver only when the entry shape allows.
- `assembled-syntax-to-checked-compilation/src/checking/
  execution_settlement.rs` — `selected_program_entry` settlement plus
  `program_entry_semantic_binding_role`.
- `native-realization/src/native_product/admission.rs::admit` — the
  downstream gate: "native-artifact production requires one exact
  selected program entry" (None rejected).

## Live sibling legs on adjacent surfaces

- EXACT-PROGRAM-ENTRY-MULTIPLICITY (devin-w9, 04:43Z) —
  `typed-trees-to-checked-trees/src/execution/terminal_unit/{returns,
  composed_control,scalar_targets}` + `selected-dispatch/service_custody`
  (upstream selection surface).
- DIVISION-CANARY-ENTRY-BINDING (Jarod, 03:05Z) — runtime entry-binding
  fixtures.
- BACKEND-STARTUP-ENTRY-MECHANICS (z175, 04:10Z), OCREQ-REQUEST-ENTRY-
  BINDING (z25, 03:30Z), UEFI-PHYSICAL-SEMANTIC-ENTRY (22:59Z) —
  adjacent entry lanes.
- The prior z161 same-item claim (held `checked-interpreter`) drained.

## Outcome

The named exactness clause is already implemented and gated; remaining
upstream legs are claimed. No independent unclaimed slice — record only.
Coordinator may fold the stub into the program-entry cluster
(SINGLE-PROGRAM-ENTRY-SELECTION, EXACT-PROGRAM-ENTRY-MULTIPLICITY,
PROGRAM-ENTRY-SELECTION-DIVISION).
