# Buffered conversion preserves the value

A caller supplies a buffer, encodes a pair and decodes it once. The returned
pair must equal the original for every admitted pair, and the caller keeps the
same output storage. A successful tag alone is insufficient.

Parent: **LIBRARY-REPRESENTATION-FAMILIES**. The exact remaining requirement is
resource-backed semantic preservation under the selected conversion contracts,
with explicit storage, failures and sufficient capacity. The earlier
[capacity row](../resource_representation/README.md) proves success and resource
custody; the [fixed-owned agreement row](../representation_agreement/README.md)
uses pure value maps. This row concerns the actual completed effectful call and
its current byte contents. It does not repeat the calls as mathematical terms.

**A completed buffered round trip preserves every semantic field and returns the caller's output storage.**

The [local witness](local/main.omg) retains the three-byte requirement,
arbitrary two-byte Pair, caller-owned mutable output and explicit failure.
Reading borrows the frame while its owner retains the mutable output resource.
The encoder promises which bytes it writes, and the decoder promises which Pair
fields those bytes produce. One generic caller uses the selected round-trip
contract. A separate runtime caller invokes that same complete round trip through
ordinary TryFrom, whose interface supplies no preservation promise. It checks
both fields in the actual returned request, its buffer contents and its original
length, then writes through the returned buffer. The caller verifies that write
and the unchanged neighboring bytes. Runtime checks use unequal 17/42 and
endpoint 0/255 pairs, a short buffer and a malformed tag.

The [reject twin](local/invalid/main.omg) changes only the decoder's final byte
load from slot 2 to slot 1. Its capacity, success paths and resource handling
remain identical. It must fail semantic preservation, rather than formation or
an unrelated obligation.

The optimized baseline at `3cfc757f135a4c93fd3bdd4c356c2b55aba10a6d`
([run 41546898552](https://github.com/CathedralOS/Omega/actions/runs/41546898552))
refuses the valid source: it cannot form the successful payload after excluding
the only failure case, and it loses the saved public decoder call when selecting
its implementation. The corrupt twin fails for those same unrelated reasons, so
that refusal is not preservation evidence.

No completed place is claimed. The programs now carry the completed Result
through their respective places before consumption. Forwarding promises preserve
the incoming Result; they never require equality to the expected Pair. Their
compiler proofs, native/Terminal observations and intended reject diagnostics
remain pending, so the nine-place row stays open.
The [current source check](https://github.com/CathedralOS/Omega/actions/runs/41702001857)
at `25c7948b99244262d63b27c7f31f746d7c051acb` proves the encoder's byte
promises and the buffer-capacity requirement at restoration entry. All 28 field
formation conflicts are resolved: each case field retains its own source, and
formation distinguishes lifetime obligations and opposite cases of the same
saved Result. Local and field still have six restoration proof failures.
Three additional reference controls reject before their intended obligations,
and a sibling control times out. Their attribution is pending a same-host
comparison of retained producers; these are not accepted controls yet.

The [stale-byte control](../../../../tests/omega/fail/contracts/forwarded_data_identity_does_not_preserve_mutated_contents/main.omg)
reads a byte, replaces it, then asks to prove that the old and current bytes are
equal. It now rejects precisely that equality at `need_unchanged`, without the
earlier unrelated ownership failure. Five independent storage positives check,
and three overlapping-loan or stale-descriptor controls reject their intended
obligations. These are source checks, not execution observations. Decoder twins
still need their intended field-law failures after the common restoration
failures are repaired.

Eleven separate controls retain native/Terminal zero exits from
[run 41672891758](https://github.com/CathedralOS/Omega/actions/runs/41672891758).
These observations establish their recorded producer's behavior; the current
compiler repairs still need their own affected checks.

The [constant-call checks](https://github.com/CathedralOS/Omega/actions/runs/41573401092)
execute four independent method, binder, application and lifetime programs with
native/Terminal exit zero; wrong-binder and wrong-region controls reject.
The full standard Representation interface, reflection adapters, other hosts
and the Omega-written producer also remain parent work.

The constant place runs the same operation and its full borrowed consumers over
local storage during semantic evaluation. Only owned observations cross the
constant boundary: Pair checks, byte snapshots, extent checks and write markers.
The live mutable reference stays inside that evaluation. Main verifies those
constant observations and runs the same observer at runtime.

The following obligations remain open until the programs check, execute and
reject their controls as authored:

| Requirement | How the programs exercise it |
| --- | --- |
| Establishment | The writer must prove its byte promises from its stores. The reader must prove its field promises from the bytes it reads. |
| Value identity and lifetime | The input pair keeps its saved value, and the returned buffer permits a write visible to the caller. The frame borrow has a separate lifetime from its buffer. |
| Composition | The public round-trip promise justifies the consuming call directly. Each place carries the completed Result before that call. |
| Ownership and proof boundaries | Reading borrows the frame while its owner keeps the mutable buffer. Only owned snapshots leave constant evaluation. The corrupt decoder must fail its field promise. |
| Checked source to Terminal Psi | Both engines must run the complete operation once, preserve the borrowed storage and return the authored result. No buffered witness has passed Terminal execution yet. |
| Build selection, layout and calls | All four hosted entries are bound. The selected-body place checks which implementation ran, and neighboring fields and bytes detect misplaced storage or arguments. |
| Instructions, registers and frames | Native execution must preserve both pair fields and the returned buffer across the nested and selected calls. This remains unverified. |
| Encoding and native image | The emitted program must execute and agree with Terminal Psi. Other-host observations remain open. |
| Runtime falsifiability | Main traps on a wrong pair, byte, extent, marker or neighbor. It exercises both pair inputs, both join arms, short capacity and malformed input. |

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
