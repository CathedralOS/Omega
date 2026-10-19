# Retain an issued linear claim while adding a label

Fact: an issued linear claim can gain a route-free label without losing its
issued qualification. Each place observes payload 29 after qualification and
consumes the same linear claim exactly once. Both control branches run.

This is a prerequisite for BUMP-ALLOCATOR-CANARY: the compiler must use declared
custody routes, not infer restrictions from unrelated domains on a linear
carrier. It does not implement allocation, placement initialization or retirement.

Run each place with `omega run --both <place>/main.omg` after ordinary package
preparation. Each must report native exit 0 and Terminal interpreter exit 0.
The nine places are `local`, `field`, `parameter`, `call_result`, `match_join`,
`const_name`, `other_state`, `other_machine`, and `target_body`. Every place's
`invalid/main.omg` replaces issuance with construction of the same raw carrier;
it must reject missing `Claim::Issued` provenance. The same programs are pinned
under `tests/omega/{pass,fail}/facts/linear_routed_label_<place>`.

## Requirements exercised

- Establishment: the exact `issue` machine establishes `Issued`; the cast adds
  only the empty `Label` obligation. `Tagged` is their transparent conjunction.
- Identity and lifetime: the issued value is moved, not reconstructed or copied;
  the final consumer observes its payload and discharges its linear lifetime.
- Composition: the same fact survives the nine places. The const place names the
  payload constant; it does not pretend a linear claim is a reusable constant.
- Soundness boundaries: each raw-carrier twin must fail issuance. Separate core
  memory controls retain `Vacant`/`Resident` protections under explicit routes.
- Psi 05–07: normalized atomic memberships and whole-value custody must survive
  proof erasure and `PSITERM` execution. No borrow-restoration or dynamic-dispatch
  claim is made by this row.
- Omega 00–02: ordinary hosted Build entry selection, record layout and the
  structural argument/result calling plans carry the same claim and payload.
- Omega 03–06: selected instructions, register homes and activation frames must
  preserve the value through calls and state transitions without duplicate moves.
- Omega 07–09 and emission: native image emission must agree with Terminal
  execution on the payload and exit oracle. Four hosted target bindings are
  authored; execution evidence is limited to the host actually measured.

## Current frontier

The original positive failed the carrier-wide issuer heuristic. Removing that
heuristic exposed alias-constituent checking, whole-value projection custody,
atomic domain publication and checked target-family result propagation gaps.
All nine places execute with native and Terminal exit 0 on macOS ARM64; their
nine raw-carrier twins reject missing issuance. Focused fixtures additionally
exercise nested qualification, retained field siblings, moved-value reuse, exact
resident indices and invalid issuance inside an unselected target body. The
parent still needs
actual-backed vacancy, initialization, placed read-back, retirement and exact
backing return, followed by allocator coexistence, failure conservation and reset.
