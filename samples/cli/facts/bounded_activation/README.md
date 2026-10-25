# Bounded activation establishment

Owning requirement: **RUNTIME-SIZED-ACTIVATION-CONTRACT** in `TASKS.md`.
Contract: [activation storage](../../../../wiki/spec/resources/activation_storage.md).

`main.omg` commits three bytes under bound eight, writes and reads byte two
as 37, releases, then requires checked rejection of nine. The frozen source
is unchanged from the genuine failing baseline. All companions bind the four
hosted targets; execution evidence is Linux x86_64 only.

The baseline producer at `53cd2938437a29356a48d6eaf29f14de07a4bb99` plus the
proposed core API rejected the authored program with seven bodyless `try_claim`
and `bytes` diagnostics (exit 200, 6.9475 seconds). This is a missing feature,
not an existing-program regression. No baseline engine executed.

On base `c1e25ff99985dab2f06b6b858586cac478da0c2c`, with
`C:\tmp\tgt-c1e2\debug\omega.exe` and stdin closed,
`omega run --both samples/cli/facts/bounded_activation/local/main.omg` exited
200 before either engine ran:

```text
terminal-artifact production failed: Artifact(Semantic(MalformedStructuralFoundation("activation claims must release in reverse establishment order in their owning activation")))
```

The authored places already release the single claim after its admission case,
then reject a request for nine. The flow's stack had that same home as its
newest live claim. A call that borrows the committed bytes publishes a fresh
claim id for that home. The judgment treated the fresh id as a different
sibling. It now requires the released place to be the newest live home, with
the admission case already selected, and rejects a claim id that still names
another live claim. `nested_lifo` was already exit 0 on that base.
Non-LIFO `bounded_activation_release_order_rejected` still rejects with the
same diagnostic.

The exact sealed core declarations now establish caller-activation provenance.
Checked calls retain a closed static bound and runtime committed operand;
Terminal operations retain the bound, conditional claim and release identity.
Publication and optimized-graph validation replay actual CFG custody and loan
lineage. Native storage reserves the full bound in the fixed frame, including
probe and unwind coverage; Terminal keeps stable backing per activation.
Successful committed bytes are initialized before readable loans are exposed.
Requests above the bound produce `Rejected`, without clamping or trapping.

All nine standard transport places (`local`, `field`, `parameter`, `call_result`,
`match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) and
their `invalid/` reject twins carry the bounded activation claim
(`Activation::try_claim<8>(3)`, committed byte write/read `37u8`, LIFO release,
and over-bound `9` rejection) across all four hosted targets alongside the
mechanism witnesses:

| Place / Witness | Positive (`main.omg`) | Reject twin (`invalid/main.omg`) | Obligation |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | - | Root probe committing 3 bytes under bound 8, writing/reading byte 2 as 37u8, releasing, and observing `Rejected` for 9. |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | Local `ActivationClaim` binding (`live`), `bytes` slice borrow, write/read, release, and over-bound check. |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | Linear `ActivationClaim` transported through `Holder.claim` field (`&mut holder.claim`). |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Runtime `committed: u64` parameter passed to `exercise(3)` and `exercise(9)`. |
| call_result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Helper machine `run_attempt() -> bool` establishes, accesses, releases, and verifies over-bound rejection. |
| match_join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `ActivationClaimAttempt` unified across conditional `match` arms before claim access and release. |
| const_name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Symbolic constants (`BOUND`, `COMMITTED`, `OVER_BOUND`, `MARKER`) supply static bound and runtime extents. |
| nested machine (`other_state`) | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Linear `ActivationClaim` carried by the terminal call from `hop(claim)` to `established(claim)`. |
| other_machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Cross-machine `Accessor::write_read(bytes)` accesses the borrowed activation slice before release. |
| target_body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Target-scoped `target_exercise` bodies across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`. |
| `field_transport`, `parameter_committed`, `named_attempt`, `named_bound`, `computed_bound`, `generic_bound`, `zero_bound`, `unsigned_max_request`, `nested_lifo`, `multiple_bounds`, `cleanup_pure`, `page_bound`, `extent_composition` | `<witness>/main.omg` | `tests/omega/fail/facts/bounded_activation_*_rejected` | Specialized mechanism witnesses for generic/computed/page bounds, co-live LIFO claims, bound composition, and pure nominal cleanup hooks. |

Matching `bounded_activation_*` and `bounded_activation_through_<place>_{exit,rejected}` corpus fixtures pin both engine exits as zero and verify source-check rejections.
Source assertions check extent and contents; those values are not printed
measurements. Perturbing a single token in the root oracle (`observed == 37u8` -> `observed == 36u8`) while retaining the store of 37 causes both engines to trap (`interp crash: Trap`, `native exit -1073741795`).

## Pinned fixtures

- Standard 9-place positive fixtures (9):
  - `tests/omega/pass/facts/bounded_activation_through_local_exit`
  - `tests/omega/pass/facts/bounded_activation_through_field_exit`
  - `tests/omega/pass/facts/bounded_activation_through_parameter_exit`
  - `tests/omega/pass/facts/bounded_activation_through_call_result_exit`
  - `tests/omega/pass/facts/bounded_activation_through_match_join_exit`
  - `tests/omega/pass/facts/bounded_activation_through_const_name_exit`
  - `tests/omega/pass/facts/bounded_activation_through_other_state_exit`
  - `tests/omega/pass/facts/bounded_activation_through_other_machine_exit`
  - `tests/omega/pass/facts/bounded_activation_through_target_body_exit`
- Standard 9-place reject fixtures (9):
  - `tests/omega/fail/facts/bounded_activation_through_local_rejected`
  - `tests/omega/fail/facts/bounded_activation_through_field_rejected`
  - `tests/omega/fail/facts/bounded_activation_through_parameter_rejected`
  - `tests/omega/fail/facts/bounded_activation_through_call_result_rejected`
  - `tests/omega/fail/facts/bounded_activation_through_match_join_rejected`
  - `tests/omega/fail/facts/bounded_activation_through_const_name_rejected`
  - `tests/omega/fail/facts/bounded_activation_through_other_state_rejected`
  - `tests/omega/fail/facts/bounded_activation_through_other_machine_rejected`
  - `tests/omega/fail/facts/bounded_activation_through_target_body_rejected`
- Additional mechanism & control fixtures:
  - `tests/omega/pass/facts/bounded_activation_{main,field_transport,parameter_committed,named_attempt,named_bound,computed_bound,generic_bound,zero_bound,unsigned_max_request,nested_lifo,multiple_bounds,cleanup_pure,cleanup_crossing,page_bound,extent_composition}_exit`
  - `tests/omega/fail/facts/bounded_activation_{escape,stale_loan,wrong_activation,release_order,extent_composition}_rejected`

| Obligation Class | How This Fact Row Establishes It |
|---|---|
| **Establishment** | `Activation::try_claim<8>(3)` checks `committed <= bound` (`3 <= 8`) and establishes a caller-scoped linear `ActivationClaim` whose borrowed `&mut [u8]` slice has runtime length `3`, while `Activation::try_claim<8>(9)` returns `ActivationClaimAttempt::Rejected` without trapping or clamping ([activation storage](../../../../wiki/spec/resources/activation_storage.md)). |
| **Identity and lifetime** | `ActivationClaim::bytes(&mut live)` ties the mutable slice loan `&mut [u8]` to the linear `ActivationClaim` owner (`live` or `holder.claim`), requiring all slice accesses (`write_read(bytes)`) to complete before `ActivationClaim::release(live)` retires the claim. |
| **Composition** | Across all nine standard transport places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`), the linear `ActivationClaim` and its `&mut [u8]` view compose through record fields, state hops, cross-machine calls, and target bodies. |
| **Soundness boundaries** | Each `<place>/invalid/main.omg` attempts `ActivationClaim::release(...)` while `bytes` is still borrowed and used afterward, failing closed in source checking with `local borrow \`bytes\` is still active`; `escape`, `wrong_activation`, and `release_order` reject escaping claims, wrong activation release, and non-LIFO release. An admitted claim keeps its caller home across a suspending nominal cleanup of a different carrier. An unselected admission still has no storage role and cannot cross. |
| **Checked-to-Terminal Psi lowering (`Psi 05–07`)** | `Activation::try_claim`, `ActivationClaim::bytes`, and `ActivationClaim::release` lower into canonical `PSITERM\0` bounded-activation operations and execute in the Terminal Psi interpreter (`omega run --both`). |
| **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)** | `ProgramEntry` settles for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`; fixed-bound activation storage and slice descriptor calling plans validate without any external provider. |
| **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)** | Backend lowering reserves the static bound in the activation frame, composes worst-case stack usage (WCSU) and page probes (`page_bound`), and zero-initializes committed bytes before exposing `&mut [u8]`. |
| **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)** | Native PE/ELF/Mach-O images emit and execute on `windows_x86_64` and `linux_x86_64` with exit code `0`, agreeing with Terminal Psi (`omega run --both`). |

The four original one-obligation negative controls under `tests/omega/fail/facts` reject
escape and stale loans during source checking, and wrong activation and non-LIFO
release during Terminal publication, before either engine executes. The cleanup
crossing keeps that same fact and changes only the hook from a pure leaf to a
boundary-reaching machine. On `26348267c99` that crossing was refused during
Terminal publication with `activation claim cannot remain live across a possible
suspension in nominal cleanup`. The admitted caller claim now stays in that
home across the parked hook. Both engines exit 0 and the two byte writes
produce `AA`. An unselected admission still cannot cross.

This establishes the bounded serial path through source checking, checked-to-
Terminal lowering, current optimized semantics, ordinary calling plans, fixed
frame realization, instruction selection and native image emission, including
retention of an admitted claim across that suspension. No provider supplies
the grant. `extent_composition` keeps two 8-byte claims live inside bound 16
and then takes one of the exclusive 16-byte and 12-byte branches; both engines
exit 0. Two co-live bounds that sum past the closed demand reject before
establishment with `activation co-live bound leaves closed stack demand open`.
`claim_split` splits one established bound-16 claim into covering children and
merges those children back into that claim. Both engines exit 0 for the local
program and for field, parameter, call result, match join, const name, another
state, another machine, and target body. A cut that leaves the upper child
empty rejects before those children are established with `activation claim
split does not cover its parent`. Partial mutable-owner windows and further
target execution remain open. Open demand still rejects before establishment.
Backing does not move, and the split does not mint a grant from an address
and length.

Invocation-local ignored `build/activation-slot4` retains producer identities,
canonical passages with source revisions, raw observations, qualification and
review receipts. Historical failed experiments remain historical evidence.
