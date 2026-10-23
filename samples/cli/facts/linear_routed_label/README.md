# Retain an issued linear claim while adding a label

Fact: an issued linear claim can gain a route-free label without losing its
issued qualification. Each place observes payload 29 after qualification and
consumes the same linear claim exactly once. Both control branches run.

This is a prerequisite for BUMP-ALLOCATOR-CANARY: the compiler must use declared
custody routes, not infer restrictions from unrelated domains on a linear
carrier. It does not implement allocation, placement initialization or retirement.

Run each place with `omega run --both <place>/main.omg` after ordinary package
preparation. Each must report native exit 0 and Terminal interpreter exit 0.
Every place's `invalid/main.omg` replaces issuance with construction of the
same raw carrier; it must reject missing `Claim::Issued` provenance.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the `issue` result is moved into the local `claim` and cast to `Tagged` |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the issued claim is stored in `Holder.claim`; the field projection is cast |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the claim arrives as an explicit `label(claim: Claim in Issued)` parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `forward(issue(29))` returns the claim; the call result is cast |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `choose` yields the claim from both arm states; the join result is cast |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | the payload arrives as the named constant `PAYLOAD = 29` |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state `label` receives the claim and casts it |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | the forwarding machine `label` returns `Claim in Tagged` |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies has its own `label` machine |

## Pinned Fixtures

The row is pinned by 18 fixtures: nine
`tests/omega/pass/facts/linear_routed_label_through_<place>_exit` fixtures, each
a byte-identical copy of its place program and `build.omg` (`ProgramEntry` for
all four hosted targets, `Expected exit: 0`), and nine
`tests/omega/fail/facts/linear_routed_label_through_<place>_rejected` twins,
each a byte-identical copy of its `invalid/` program pinning the diagnostic
`construction value is not proven in domain Claim::Issued`. Recorded rows: 18 `checked` with
`check-target:macos_arm64` and 18 `rejected` with `expected` in
[`corpus_outcomes.txt`](../../../../tests/omega/corpus_outcomes.txt), 9
`built exit:0 interpreter-exit:0 interpreter:agrees` in
[`corpus_both_macos_arm64.txt`](../../../../tests/omega/corpus_both_macos_arm64.txt).
The tenant of record is **DOMAIN-ISSUER-ROUTES** (`TASKS.md`): preserve the
selected-domain qualification rule and the exact claim transport that
`facts/linear_routed_label` pins.

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
