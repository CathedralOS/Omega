# Buffered conversion preserves the value

A caller encodes a pair into its buffer, then decodes it once. The returned pair
must equal the original for every admitted pair, and the caller must keep the
same writable storage. Seven places now return zero in native and Terminal
execution. Constant name and selected implementation still fail source checking,
so the nine-place row remains unfinished.

**A completed buffered round trip preserves every semantic field and returns the caller's output storage.**

Parent: **LIBRARY-REPRESENTATION-FAMILIES**. This row tests value preservation
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
Seven buffered places satisfy these authored runtime oracles. Constant name and
selected implementation have no execution observations because checking rejects them.

The constant place owns its temporary buffer and runs the same conversion and
borrowed consumers during semantic evaluation. Only owned Pair observations,
byte snapshots, extent checks and write markers leave that evaluation. The live
mutable reference stays inside it. Main checks those observations and runs the
same observer at runtime.

[Run 41747418767](https://github.com/CathedralOS/Omega/actions/runs/41747418767),
at exact producer source `6fd663e9e8c6b834b3578664238b33db6ffc8f44`,
checks and executes seven places: local, field, parameter, call result, match
join, nested machine and other machine. Each returns native 0 and Terminal 0
with engine agreement. The returned-data identity control and three constant-call
identity controls also return zero in both engines.

The length guarantee now survives forwarding because the checker joins the
saved descriptor identities before normalization can replace their subjects.
The constant program retains its exact selected public method, then reaches a
separate argument-admission failure. Two places still fail checking:

| Place | Remaining source obligation |
| --- | --- |
| Selected implementation | Preserve the returned Pair's identity with the input after forwarding. |
| Constant name | Admit the exact owned Pair argument during constant evaluation. |

The run checks 30 source rows: 11 positives check and 19 programs reject, including
those two unintended positive failures. All 17 selected negative expectations
match. The selected-implementation decoder twin also reports its positive's
consumer proof error, so it is not yet clean evidence of only the decoder law.
No infrastructure failure occurred.

The broader 72-source qualification, nine ordinary CLI projects and eighteen
positive/invalid samples remain required. This run measures Linux x86-64;
four hosted Build entries are authored, while other-host execution remains open.
The [earlier broader run](https://github.com/CathedralOS/Omega/actions/runs/41743667622)
at original source `3676a32990a7560230cc040907a4e1c704175908` retains its own
25 runtime observations and four explicit header-zero postchecks. Those results
belong to that earlier producer. No full acceptance or row closure is claimed.

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
