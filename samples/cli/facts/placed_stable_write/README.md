# Placed Stable write

Fact: an exact permitted Stable field operation writes its original storage.

This is a prerequisite of BUMP-ALLOCATOR-CANARY, not allocator completion.
Each program starts with initialized ordinary RAM, obtains an exclusive placed
view, and writes 29 through its field accessor while a second, disjoint placed
view is held by the same invocation. The original RAM must contain 29 and
the companion must retain 43 after the loans end. The runtime comparison is the observable oracle, not a
post-hoc proof granting access to the write. Each reject twin changes only the
plan's write permission, including its named constant in `const_name`.

Run each place with `omega run --both <place>/main.omg`. All four hosted targets
are bound; actual execution evidence currently covers macOS ARM64 only.

| Place | Transport exercised |
| --- | --- |
| local | Local exclusive placed loan |
| field | Loan retained in and moved from a record field |
| parameter | Placed loan passed to an ordinary helper |
| call_result | Loan returned through an explicit lifetime |
| match_join | Borrowed source interpreted in either branch, then joined |
| const_name | Named static permission consumed by the policy |
| other_state | Loan transported to a later state |
| other_machine | Exact operation inside a generic helper with an unrelated binder |
| target_body | Loan passed into the execution-selected target body |

## Requirements and evidence

- Establishment: `Placed::view_mut` borrows already initialized storage. This
  does not establish Vacant or Resident extent claims.
- Identity and lifetime: the selected field target retains its plan, exact
  accessor/value tuple, exclusive source loan and original backing.
- Composition: the nine places transport that same write permission; no source
  spelling authorizes a new primitive or a new physical allocation.
- Soundness: every twin removes write permission. Additional fixtures under
  `tests/omega/{pass,fail}/layouts/placed_primitive_*` cover wrong value/accessor
  types, shared mutation, generic forwarding, static callbacks and private issuance.
- Psi 05–07: the selected catalog target becomes the existing placed boundary
  operation. Loan transport and retirement survive lowering into Terminal Psi;
  both-engine execution must agree on the original-storage result.
- Omega 00–02: the exact plan and actual backing determine provider admission,
  field geometry and structural argument calling plans.
- Omega 03–06: native address materialization must retain backing geometry through
  joins and calls; a descriptor home is not its referent allocation.
- Omega 07–09 and emission: the host image must execute the authored writeback
  oracle, not merely publish or pass checking.

## Host evidence and remaining scope

The nine places execute with native/Terminal exit 0 on macOS ARM64; all nine
permission twins reject. Each place is pinned twice: the check-side golden
records its twin as `rejected`, and `tests/omega/corpus_native_macos_arm64.txt`
records the positive as `built ... exit:0` with `jobs=3`. The native row is the
measurement that the host image executes the writeback oracle; the check row
alone would not move if a regression built and then exited wrongly.

The pin canonicalization re-measurement (producer `9ff26a870215` for the
sample gate; opt-level 1 corpus runner built from the landed tree for the
goldens; host `macos_arm64`, `--jobs 2`) renamed the fixture families to
`<fact>_through_<place>_{exit,rejected}` and key-renamed their golden rows:
`python3 tools/sample_native_gate.py --cli <producer> --filter
cli/facts/outcome_structural_evidence,cli/facts/placed_stable_write --jobs 2`
reports 19/19 samples meeting their documented oracle and 18/18 intended-invalid
controls matching their authored rejection (620 s wall); `python3
tools/corpus_gate.py --record --filter
facts/outcome_structural_evidence,facts/placed_stable_write` at `88723a749c16`
re-records the nine check rows as `checked` (twins `rejected` with their
fragment), the nine native rows as `built ... exit:0`, and the nine both-leg
rows as `exit:0 interpreter-exit:0 interpreter:agrees`.

The obligation is construction-gated, and the probes that establish that are in
[the discrimination record](DISCRIMINATION.md).
The write is observable through the placed home: corrupting only the consumer's
demanded value traps both engines (`native signal 5`, `interp crash: Trap`).
Violating the construct's own licence refuses before runtime realization — a
permission derived from a runtime-read plan value still runs, a folded-false
permission refuses, and seeding the permission from a runtime parameter is
inadmissible at the plan machine signature. No admissible seeding defers the
licence to runtime, so no runtime trap exists for it; this is a construction-gate
class, not a value/arithmetic bound. Additional corpus witnesses cover ordinary returned
and stored backing references, static callbacks, and actual allocation alignment.
One insufficient caller rejects even when another caller supplies aligned backing.
This establishes permitted reads and writes over initialized Stable RAM, not
destructive transfer, Vacant initialization, allocator conservation or reset.
Pinned corpus copies live under `tests/omega/pass/facts/placed_stable_write_through_<place>_exit`
and `tests/omega/fail/facts/placed_stable_write_through_<place>_rejected`.
Build logs and actual producer identities stay in the qualification/claim record.

The simultaneous-view repair retains a conservative pre-interpretation enclosure
separately from scalar referents and lifetime dependencies. Inline returned
views count as one completed argument while their producing input holds remain
live against later arguments. The controls under `controls/` reject aliasing
placed views and later writes to either the selected or unselected input of a
reference-returning call. An unrelated later write executes with both engines.
These controls pin the distinction between pointer geometry and declared lifetime
debt; an enclosure cannot supply value equality or access authority.
