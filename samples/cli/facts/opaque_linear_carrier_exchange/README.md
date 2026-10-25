# Linear opaque component exchange

Owning task: **NORMALIZED-ABI-LOWERING**, retaining
**OPAQUE-BY-VALUE-BOUNDARY-ABI**. This row establishes the missing linear
ownership lane of independently compiled closed Inert opaque arguments and
results, under [opaque representations](../../../../wiki/spec/build/opaque_representations.md)
and [ownership](../../../../wiki/spec/language/ownership.md).

A linear closed Inert opaque occurrence preserves libc host 66051 across an independent component and retires exactly once, while sibling host 513 remains distinct.

Each positive transports the same actual address through its place before one
independent `AddressBridge::host` call. The address comes from a separate actual
independent `make` call and retains the selected nested C representation. A
sibling address carries 513 through a distinct call. The real host functions
construct and observe the values; equal-sized invented payloads cannot supply
the interpreter's admitted host atom. A guest obligation moves even though its
physical carrier is Inert and structurally copyable.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [unconsumed occurrence](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [unconsumed occurrence](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [unconsumed occurrence](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [unconsumed occurrence](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [unconsumed occurrence](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [unconsumed occurrence](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [unconsumed occurrence](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [unconsumed occurrence](other_machine/invalid/main.omg) |
| Target body | [program](target_body/main.omg) | [unconsumed occurrence](target_body/invalid/main.omg) |

Each twin replaces only `AddressBridge::host(sent)` with its expected scalar;
the unconsumed linear `sent` must refuse at normal scope exit. This distinguishes
linear exact-once debt from an affine value's permitted discard. The scalar
oracle and producer remain identical. All positives and producer bind all four
hosted targets. The const-name place transports construction inputs rather
than attempting to materialize an external runtime resource as a constant.

Construction establishes the exact opaque type and one root claim; component
arguments transfer existing caller claims by argument-relative paths, and
normal returns transfer their actual edge-authenticated root debt into the
receiving operation's local claim identity. No physical copy creates another
guest occurrence. Exact installed provider, boundary/type/application and host
token joins remain required; qualifiers, loans, backing and non-root receipts
require their own carrier rather than disappearing at this lane.

The frozen pre-repair compiler `fbc7d0be2f79ce14756ddc65a337c4ff620cd8e0`
executes the local positive natively with exit 0 but Terminal refuses
`inet_makeaddr requires two u32 scalars and one unqualified owned inert result`.
The local reject diagnoses `linear value sent reaches scope exit without being
consumed or transferred`. Versioned raw records are under the owning claim's
`build/opaque-copy/` run ledger. This is a demonstrated repair, not closure.

At `153b9a000b60`, the public CLI observes native 0 / Terminal 0 at all nine
places and the nine unconsumed-debt controls reject. Independent review also
replays local, field and true nested transports, plus an added duplicate-consume
control. The corpus's additional retained-target audit exposed a separate
linear-result refusal at `normalized_foreign.rs:71`. Its `9337a2b70540` repair
retains the entire checked result, including claim IDs and paths, under exact
source/target equality while evaluating physical geometry independently. All
nine linear corpus positives then observe 0/0, and the checked gate observes
18 copy/linear positives plus 24 intended rejects. That corpus run remains
nonaccepted overall because a copy-row match join timed out natively.

Each linear fixture also requests a retained-target negative control through
`expected_native_linear_result_claims.txt`: an unmodified source-produced audit
must pass, while clearing or substituting its target-only root claim must fail
source custody. The control clones target proposals and retains the exact
validated source; it cannot mutate the admitted artifact. Execution of this
new control and final exact-candidate review remain pending.

Remaining acceptance includes pinned corpus records, matching-host observations
beyond Windows, the general nested-machine source/product migration prerequisite,
and normal main landing. The parent additionally retains qualified and
view-backed exchange, composition drift, outbound retention and remaining host
ABI lanes.
