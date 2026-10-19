# Placed Stable write

Fact: an exact permitted Stable field operation writes its original storage.

This is a prerequisite of BUMP-ALLOCATOR-CANARY, not allocator completion.
Each program starts with initialized ordinary RAM, obtains an exclusive placed
view, and writes 29 through its field accessor. The original RAM must contain
29 after the loan ends. The runtime comparison is the observable oracle, not a
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
permission twins reject. Additional corpus witnesses cover ordinary returned
and stored backing references, static callbacks, and actual allocation alignment.
One insufficient caller rejects even when another caller supplies aligned backing.
This establishes permitted reads and writes over initialized Stable RAM, not
destructive transfer, Vacant initialization, allocator conservation or reset.
Pinned corpus copies live under `tests/omega/{pass,fail}/facts/placed_stable_write`.
Build logs and actual producer identities stay in the qualification/claim record.
