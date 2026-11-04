# Buffered conversion preserves the value

A caller encodes a pair into its buffer, then decodes it once. The returned pair
must equal the original for every admitted pair, and the caller must keep the
same writable storage. Local, field, match-join and nested-machine programs now
return zero in native and Terminal execution. Five other places still fail source
checking, so the nine-place row remains unfinished.

**A completed buffered round trip preserves every semantic field and returns the caller's output storage.**

Parent: **LIBRARY-REPRESENTATION-FAMILIES**. This row proves value preservation
for an actual buffered conversion, including its storage, failure paths and
three-byte capacity requirement. The earlier
[capacity row](../resource_representation/README.md) covers success and storage
custody; the [fixed-owned agreement row](../representation_agreement/README.md)
covers pure value maps.

The compiler previously lost the connection between the completed decoder call
and the fields returned by a helper. It rejected the valid preservation promise.
The repair retains the original typed fields across helper returns, completes
nested field selection and carries the checked whole-value identity into
Terminal Psi. References retain their descriptor and access meaning: identity
of the containing value grants no fact about later buffer contents. Byte facts
still require the correct storage version and are invalidated by overlapping
writes.

The [local witness](local/main.omg) encodes a tag followed by the arbitrary Pair's
two bytes. The writer promises which bytes it stores; the reader promises which
fields those bytes produce. The caller consumes the selected round-trip contract
directly. Every place transports the completed Result before that consumption;
forwarding promises preserve the incoming Result without requiring an equality
guard against the expected Pair.

The [reject twin](local/invalid/main.omg) changes only the decoder's final load
from slot 2 to slot 1. For input 17/42, this would return 17/17. Capacity, success
paths and storage handling remain fixed. Each twin must reject that field law,
without unrelated formation or ownership failures. The nine positives and twins
are also retained under
[tests/omega/pass/facts](../../../../tests/omega/pass/facts/) and
[tests/omega/fail/facts](../../../../tests/omega/fail/facts/).

A separate runtime caller uses ordinary TryFrom, whose interface supplies no
preservation promise. It examines both actual returned Pair fields, buffer bytes
and original length, then writes through the returned buffer and checks that
write and the untouched neighbors. Main traps on any wrong observation. Inputs
include unequal 17/42 and endpoint 0/255 pairs, short capacity and malformed tags;
the join and selected-body places also check both branches and body markers.
The four running buffered places satisfy these authored runtime oracles; the
other five have no execution observations because source checking rejects them.

The constant place owns its temporary buffer and runs the same conversion and
borrowed consumers during semantic evaluation. Only owned Pair observations,
byte snapshots, extent checks and write markers leave that evaluation. The live
mutable reference stays inside it. Main checks those observations and runs the
same observer at runtime.

[Focused run 41740995881](https://github.com/CathedralOS/Omega/actions/runs/41740995881),
at exact source `3676a32990a7560230cc040907a4e1c704175908`, checks LOCAL without
diagnostics and two identity positives. Its three negatives reject their intended
obligations. The local twin reports only four Reader decoder-law diagnostics;
the stale-byte control rejects its claim that a saved byte equals overwritten
current storage. This is checking evidence, not execution or all-place acceptance.

[Full qualification 41743667622](https://github.com/CathedralOS/Omega/actions/runs/41743667622)
completed failed at that exact producer, without an infrastructure error. Of 69
source rows, 27 check and 42 reject, including five unintended buffered positive
rejections. Of 30 runtime positives, 25 return native 0 and Terminal 0 with engine
agreement: local, field, match join and nested machine, plus 21 other controls.
All four explicit header-zero postchecks pass.

Five buffered places still fail source checking:

| Place | Remaining source obligation |
| --- | --- |
| Call result | Preserve the returned buffer's length >= 3 after forwarding. |
| Parameter | Preserve the returned buffer's length >= 3 after forwarding. |
| Other machine | Preserve the returned buffer's length >= 3 after forwarding. |
| Selected implementation | Preserve the returned Pair's identity with the input after forwarding. |
| Constant name | Retain the exact authored encode_sufficient selection during constant evaluation. |

The constant-call repair at `c5ad035483a77adc0b3dffaf3c70ede0d438438e`
joins the authored public method to the existing strict dispatch validator.
Source review and formatting pass; fresh constant execution remains pending.
The forwarded-result failures are still under investigation.

All 37 negative expectations match. Four decoder twins—call result, parameter,
other machine and selected implementation—also have the corresponding positive's
consume proof error. Those four are not yet clean evidence of the decoder law.
The canonical API stopped after the earlier failures, so nine ordinary CLI
projects and eighteen positive/invalid samples were not reached. The run measures
Linux x86-64; four hosted Build entries are authored, but other-host execution
remains open. No full acceptance or row closure is claimed.

The [original valid-source baseline](https://github.com/CathedralOS/Omega/actions/runs/41546898552)
and [compiler changes](https://github.com/CathedralOS/Omega/pull/1562) retain the
earlier diagnostics and repairs. Five additional controls remain unchanged and
unaccepted: the [four reference-control baseline checks](https://github.com/CathedralOS/Omega/actions/runs/41707158340)
and the [separate case-membership baseline check](https://github.com/CathedralOS/Omega/actions/runs/41742072821)
reproduce their early refusals or timeout at fixed main on identical inputs.
They are excluded from intended-control acceptance, with separate provenance.

The requirements below remain open until the fresh source and runtime results
satisfy them:

| Requirement | How the programs exercise it |
| --- | --- |
| Establishment | The writer proves its byte promises from its stores; the reader proves its Pair fields from the bytes it reads. |
| Value identity and lifetime | The input Pair keeps its saved value. The returned buffer permits a caller-visible write, and the frame borrow has a separate lifetime from its buffer. |
| Composition | The public round-trip promise justifies the consuming call directly. Each place carries the completed Result before that call. |
| Ownership and proof boundaries | Reading borrows the frame while its owner keeps the mutable buffer. Only owned observations leave constant evaluation. The corrupt decoder must fail its field promise. |
| Checked source to Terminal Psi | Four places return Terminal exit 0; the other five remain blocked during source checking. All-place execution remains required. |
| Build selection, layout and calls | All four hosted entries are bound. Selected-body markers and neighboring fields/bytes detect the wrong implementation or misplaced storage. |
| Instructions, registers and frames | Four places return native exit 0 and agree with Terminal. The remaining five places still need source and runtime acceptance. |
| Encoding and native image | The emitted program must execute and agree with Terminal Psi. Other-host observations remain open. |
| Runtime falsifiability | Main traps on a wrong Pair, byte, length, write, marker or neighbor; it exercises both Pair inputs, both join arms and explicit failures. |

| Place | Completed-result transport | Reject twin |
| --- | --- | --- |
| [Local](local/main.omg) | Local completed result | [Wrong last-byte load](local/invalid/main.omg) |
| [Field](field/main.omg) | Result field between checked neighbors | [Wrong last-byte load](field/invalid/main.omg) |
| [Parameter](parameter/main.omg) | Affine Result parameter | [Wrong last-byte load](parameter/invalid/main.omg) |
| [Call result](call_result/main.omg) | Returned Result consumed directly | [Wrong last-byte load](call_result/invalid/main.omg) |
| [Match join](match_join/main.omg) | Result through both arms, distinct markers | [Wrong last-byte load](match_join/invalid/main.omg) |
| [Constant name](const_name/main.omg) | Owned observations of the completed operation | [Wrong last-byte load](const_name/invalid/main.omg) |
| [Nested machine](other_state/main.omg) | Result returned by a nested machine | [Wrong last-byte load](other_state/invalid/main.omg) |
| [Other machine](other_machine/main.omg) | Result forwarded by another named implementation | [Wrong last-byte load](other_machine/invalid/main.omg) |
| [Selected implementation](target_body/main.omg) | Result through a Build-selected body, marker and neighbors | [Wrong last-byte load](target_body/invalid/main.omg) |


Governing contracts: [codecs](../../../../wiki/spec/layouts/codecs.md),
[completed returns](../../../../wiki/spec/language/state_contracts.md#returns-and-proof-use),
[proof identity](../../../../wiki/spec/proofs/contracts.md#data-identity-in-facts)
and [fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md).
The full standard Representation interface, reflection adapters and the
Omega-written producer remain parent work.
