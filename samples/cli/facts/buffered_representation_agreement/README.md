# Buffered conversion preserves the value

A caller encodes a pair into its buffer, then decodes it once. The returned pair
must equal the original for every admitted pair, and the caller must keep the
same writable storage. All nine places now return zero in native and Terminal
execution on Linux x86-64. Each corrupt-decoder twin rejects its intended field
promise. The full Representation interface and
other-host observations remain parent work.

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

A generic conversion call may need concrete type arguments before its reference
transfers are known. The compiler keeps that unresolved obligation while checking
later calls. Fresh borrows of owned local storage still receive ordinary lifetime
and conflict checks; loaded references still need complete source information.
The [live-borrow control](../../../../tests/omega/fail/borrows/template_prefix_keeps_owned_borrow_live/main.omg)
rejects a write before the borrowed pointer's last use. The
[later-call control](../../../../tests/omega/fail/borrows/template_prefix_keeps_concrete_output_obligation/main.omg)
rejects a missing concrete call relation even after an unresolved generic call.

The [local witness](local/main.omg) encodes a tag followed by the arbitrary Pair's
two bytes. The writer promises which bytes it stores; the reader promises which
fields those bytes produce. The caller consumes the selected round-trip contract
directly. Every place transports the completed Result before that consumption;
forwarding promises preserve the incoming Result without requiring an equality
guard against the expected Pair.

The [reject twin](invalid_local/main.omg) changes only the decoder's final load
from slot 2 to slot 1. For input 17/42, this would return 17/17. Capacity, success
paths and storage handling remain fixed. Each twin must reject that field law,
without unrelated formation or ownership failures. All nine positives and twins
remain here as executable samples. Main's corpus cleanup removed their duplicate
test folders; the original run reports retain those earlier observations.

A separate runtime caller uses ordinary TryFrom, whose interface supplies no
preservation promise. It examines both actual returned Pair fields, buffer bytes
and original length, then writes through the returned buffer and checks that
write and the untouched neighbors. Main traps on any wrong observation. Inputs
include unequal 17/42 and endpoint 0/255 pairs, short capacity and malformed tags;
the join and selected-body places also check both branches and body markers.
All nine places satisfy these authored runtime oracles in the run below.

The constant place owns its temporary buffer and runs the same conversion and
borrowed consumers during semantic evaluation. Only owned Pair observations,
byte snapshots, extent checks and write markers leave that evaluation. The live
mutable reference stays inside it. Main checks those observations and runs the
same observer at runtime.

[Run 41917901703](https://github.com/CathedralOS/Omega/actions/runs/41917901703),
at exact producer source `d97ab1ffb60d787137a56d861a03e45dd49219ee`,
executes all nine positives with native 0, Terminal 0 and engine agreement.
All nine twins reject only the Reader's field guarantees. The complete request
has 35 source observations, 17 native/Terminal agreements, nine ordinary CLI
projects and eighteen sample results. Every CLI project completes update and
both-engine execution; all nine sample positives return zero and their nine
twins reject as intended. All four strict storage controls also return zero in
both engines, including collection metadata and the guarded array descriptor.
Original reports, optimized profile metadata, the source bundle and complete raw
log are retained with [PR 1819](https://github.com/CathedralOS/Omega/pull/1819).
These observations keep their exact producer identity when prose or regression
records change.

[Integrated run 41955214793](https://github.com/CathedralOS/Omega/actions/runs/41955214793)
on source `923ef974b33cfdd86565a5a24aef4efc90b6a4bb` also passes after merging
the newer call and cleanup checks from main. It checks the 21 surviving source
controls, executes eleven regression programs at native 0 / Terminal 0, and
passes all nine CLI projects and eighteen sample checks. Every corrupt-decoder
twin still matches its intended refusal. The nominal cleanup hook again prints
exactly `OD` in both engines. Programs, Build files and expected results are
unchanged; main's removal of duplicate test folders and runtime dumps is retained.

The direct named constant call and positional implementation through a constant
also execute at native 0 and Terminal 0 in this run. Their precondition and
runtime-service refusal controls reject. Four hosted Build entries are authored;
these runtime observations are Linux x86-64 only.

[Cleanup controls 41948975171](https://github.com/CathedralOS/Omega/actions/runs/41948975171)
on source `01b061017f95a89d09d11f18f19cc36e553dee20` also pass: four
native/Terminal exit-zero agreements and two intended rejects. The nominal hook
prints exactly `OD` in both engines; its count-prerequisite twin rejects. The
other programs check case retirement, retained operand cleanup and replacement
with an untouched sibling. These are focused regression controls, not a new
fact-row closure.

Four additional storage controls failed in earlier focused runs. The qualified-array
and byte-view element programs returned native 0 but declined in Terminal Psi;
reference rebinding failed during lowering, and nominal cleanup failed native
legalization. [Fixed-base comparison 41922376454](https://github.com/CathedralOS/Omega/actions/runs/41922376454)
reproduces all four on exact base
`b750758f9e4f2981abf643595b2f099752172e56`, at the same stages and operations.
Their complete diagnostics match; the array comparison removes only the later
outer diagnostic field `storage=None`. No failed positive from that comparison
is recorded as passing. The failure diagnostics retain the supplying reference
contracts and operation name for the next repair.

The [original valid-source baseline](https://github.com/CathedralOS/Omega/actions/runs/41546898552)
and [earlier compiler repairs](https://github.com/CathedralOS/Omega/pull/1562)
retain the earlier contract failures. The full standard Representation
interface, reflection adapters and other hosts remain parent work.

The programs exercise the following obligations. The parent task retains the
separate controls above, broader library support and other-host execution:

| Requirement | How the programs exercise it |
| --- | --- |
| Establishment | The writer proves its byte promises from its stores; the reader proves its Pair fields from the bytes it reads. |
| Value identity and lifetime | The input Pair keeps its saved value. The returned buffer permits a caller-visible write, and the frame borrow has a separate lifetime from its buffer. |
| Composition | The public round-trip promise justifies the consuming call directly. Each place carries the completed Result before that call. |
| Ownership and proof boundaries | Reading borrows the frame while its owner keeps the mutable buffer. Only owned observations leave constant evaluation. The corrupt decoder must fail its field promise. |
| Checked source to Terminal Psi | All nine places return Terminal exit 0, including the owned constant evaluation and selected implementation. |
| Build selection, layout and calls | All four hosted entries are bound. Selected-body markers and neighboring fields/bytes detect the wrong implementation or misplaced storage. |
| Instructions, registers and frames | All nine places return native exit 0 and agree with Terminal on Linux x86-64. |
| Encoding and native image | The emitted programs execute and agree with Terminal Psi. Other-host observations remain open. |
| Runtime falsifiability | Main traps on a wrong Pair, byte, length, write, marker or neighbor; it exercises both Pair inputs, both join arms and explicit failures. |

| Place | Completed-result transport | Reject twin |
| --- | --- | --- |
| [Local](local/main.omg) | Local completed result | [Wrong last-byte load](invalid_local/main.omg) |
| [Field](field/main.omg) | Result field between checked neighbors | [Wrong last-byte load](invalid_field/main.omg) |
| [Parameter](parameter/main.omg) | Affine Result parameter | [Wrong last-byte load](invalid_parameter/main.omg) |
| [Call result](call_result/main.omg) | Returned Result consumed directly | [Wrong last-byte load](invalid_call_result/main.omg) |
| [Match join](match_join/main.omg) | Result through both arms, distinct markers | [Wrong last-byte load](invalid_match_join/main.omg) |
| [Constant name](const_name/main.omg) | Owned observations of the completed operation | [Wrong last-byte load](invalid_const_name/main.omg) |
| [Nested machine](other_state/main.omg) | Result returned by a nested machine | [Wrong last-byte load](invalid_other_state/main.omg) |
| [Other machine](other_machine/main.omg) | Result forwarded by another named implementation | [Wrong last-byte load](invalid_other_machine/main.omg) |
| [Selected implementation](target_body/main.omg) | Result through a Build-selected body, marker and neighbors | [Wrong last-byte load](invalid_target_body/main.omg) |


Governing contracts: [codecs](../../../../wiki/spec/layouts/codecs.md),
[completed returns](../../../../wiki/spec/language/state_contracts.md#returns-and-proof-use),
[proof identity](../../../../wiki/spec/proofs/contracts.md#data-identity-in-facts)
and [fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md).
The full standard Representation interface, reflection adapters and the
Omega-written producer remain parent work.
