# Buffered conversion preserves the value

A caller encodes a pair into its buffer, then decodes it once. The returned pair
must equal the original for every admitted pair, and the caller must keep the
same writable storage. All nine places now return zero in native and Terminal
execution on Linux x86-64. Each corrupt-decoder twin rejects its intended field
promise. The broader affected checks, regression records, review and main
publication remain unfinished.

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
All nine places satisfy these authored runtime oracles in the run below.

The constant place owns its temporary buffer and runs the same conversion and
borrowed consumers during semantic evaluation. Only owned Pair observations,
byte snapshots, extent checks and write markers leave that evaluation. The live
mutable reference stays inside it. Main checks those observations and runs the
same observer at runtime.

[Run 41779178635](https://github.com/CathedralOS/Omega/actions/runs/41779178635),
at exact producer source `bb8bcac3d4dd40c6f13d47518d27ee52b07126dd`,
executes all nine positives with native 0, Terminal 0 and engine agreement.
All nine twins reject only the Reader's field guarantees. These observations
belong to that producer; later source or documentation changes are not new
executions. The optimized producer, original reports, source bundle and complete
raw log are retained with the PR's evidence.

Additional valid constant-call controls still stop before execution. Their
repair must preserve the exact arguments, public precondition and which
services the call may use. Those controls remain unaccepted. The final
affected checks also need all nine ordinary CLI projects, eighteen positive and
invalid sample requests, four explicit exit-zero postchecks, and recorded
regression outcomes. Earlier eight-project results use a different producer
and do not establish that final acceptance. Four hosted Build entries are
authored; this run supplies Linux x86-64 execution only. Other-host execution
remains parent work.

The [original valid-source baseline](https://github.com/CathedralOS/Omega/actions/runs/41546898552)
and [compiler changes](https://github.com/CathedralOS/Omega/pull/1562) retain the
earlier diagnostics and repairs. [Comparison 41772865951](https://github.com/CathedralOS/Omega/actions/runs/41772865951)
and [comparison 41774777402](https://github.com/CathedralOS/Omega/actions/runs/41774777402)
attribute six earlier failures to fixed main `db92349b9c2ef6e6e3705372ba6754b3e70e4874`.
Four controls stop at earlier formation or borrowing-source refusals; two
default-body positives cannot lower their returned expression. The current
producer reproduces those failures. They remain separately unaccepted, and
wrong-reason rejections do not prove their intended obligations. The
[nested-machine sibling-write control](../../../../tests/omega/fail/borrows/nested_machine_carried_field_sibling_rejected/main.omg)
rejects its intended live-borrow write and remains a required control.
These comparisons supply source attribution, not runtime or performance evidence.

The programs establish the following obligations; integration and the remaining
affected checks still need to finish:

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
