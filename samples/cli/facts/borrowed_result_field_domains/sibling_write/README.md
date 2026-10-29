# Returned field domain after a disjoint sibling write

Borrowed results retain their checked field domains and selected element, so inspection reads that element.

Owning task: [CANONICALIZE-SCALAR-RANGE-CONTRACTS](../../../../../TASKS.md),
field propositions through current-storage writes. This production extends the
existing borrowed-result fact owned by NESTED-MACHINE-UNIFICATION.
[Mutation and subject identity](../../../../../wiki/spec/language/state_contracts.md#mutation-and-subject-identity)
requires overlapping writes to invalidate observations while retaining facts about
unaffected storage. A lifetime or matching carrier type supplies no missing origin.

Every place returns a Cell through the same unknown-index choose/relay chain,
writes selected.stamp = 0xF1, then passes selected to inspect, which consumes the
standing Ascii requirement on its four-byte storage. The stamp value lies outside
ASCII and cannot establish that requirement. There are exactly two Cell elements.
The reject twin adds only selected.storage[0] = 255u8 after the sibling write.
Its oracle requires the missing Ascii field obligation at the inspect call.

| Place | Positive | Negative |
| --- | --- | --- |
| local | [program](../local/sibling_write/main.omg) | [reject twin](../local/sibling_write/invalid/main.omg) |
| field | [program](../field/sibling_write/main.omg) | [reject twin](../field/sibling_write/invalid/main.omg) |
| parameter | [program](../parameter/sibling_write/main.omg) | [reject twin](../parameter/sibling_write/invalid/main.omg) |
| call result | [program](../call_result/sibling_write/main.omg) | [reject twin](../call_result/sibling_write/invalid/main.omg) |
| match join | [program](../match_join/sibling_write/main.omg) | [reject twin](../match_join/sibling_write/invalid/main.omg) |
| const name | [program](../const_name/sibling_write/main.omg) | [reject twin](../const_name/sibling_write/invalid/main.omg) |
| nested machine | [program](../other_state/sibling_write/main.omg) | [reject twin](../other_state/sibling_write/invalid/main.omg) |
| another machine | [program](../other_machine/sibling_write/main.omg) | [reject twin](../other_machine/sibling_write/invalid/main.omg) |
| selected provider body | [program](../target_body/sibling_write/main.omg) | [reject twin](../target_body/sibling_write/invalid/main.omg) |

Every program calls both indices. Main checks the returned65/66 bytes, the first
stamp write and untouched neighbor before the second call, both final stamps,
and all eight unchanged payload bytes. No caller guard re-establishes Ascii.
The provider place performs this same operation inside the ordinary Build-selected
implementation; its abstract boundary returns the observed byte. It makes no
undeclared promise about a returned pointer's alignment within Table.

| Requirement | Evidence |
| --- | --- |
| Establishment | Checked choose/relay normal returns establish the complete field-domain roster; inspect consumes it after the sibling write. |
| Identity and lifetime | Captured descriptors retain complete possible storage coordinates independently of lifetime debt and exact pointer identity. Unknown selectors preserve element dimensions; coarse enclosures cannot accept field projections. |
| Composition | All nine transports carry the same field fact and operation. Joins retain every alternative's footprint; writes and complete intervening effects determine whether the promise survives. |
| Soundness | Every twin rejects the overlapping byte write. Existing stale-result, descriptor-replacement, loan and recast controls retain their authored outcomes. Recasts discard typed projection capability; footprints grant no access or new membership. |
| Psi05–07 | Each program runs in Terminal Psi with exit0, using the emitted descriptor, selection, field store and inspection. Its oracle observes caller storage after both calls. |
| Omega00–02 | All four hosted ProgramEntry bindings are authored. Native execution validates the selected provider, Cell/Table layout, borrowed arguments and scalar returns on macOS ARM64. |
| Omega03–06 | Distinct bytes and intermediate/final stamp canaries observe the selected element, store offset and live caller values across calls. |
| Omega07–09 and emission | Retained Terminal/native products execute with matching exit0. Omitting the stamp store or forcing element0 reaches Trap in Terminal Psi and the matching native signal5 presentation. |

Optimized baseline compiler inputs are fixed at
c0f864f5127636dcd5b80e7f63a2aab4ea6ded78. The valid witnesses refuse their Ascii
obligations on that baseline. The corrected provider witness was separately frozen
and baselined: the initial draft demanded an unexpressed spatial guarantee from
an opaque boundary and is retained only as historical, nonqualifying evidence.

CLI producer e033317ef7a586d1e8454746252f323b0a0176f1 executes all nine positives
with native/Terminal0/0 on macOS ARM64. All nine twins reject the inspect field
obligation. Matching pass/fail fixtures use the
borrowed_result_field_domains_sibling_write_<place> names under tests/omega/facts.
The focused corpus also covers five existing positive and six rejection controls:
29 source cases and14 native/Terminal cases, with no changed unselected golden rows.
Actual source hashes, independent producer receipts, locks, raw diagnostics,
engine TSVs and retained products are in build/borrowed-result-sibling-handback-20261029.
Other hosts have authored entries and no execution evidence from this run.

This production discharges preservation through disjoint sibling writes. The
parent's remaining construction, field/case propagation, indexed writes, recast
and source-migration acceptance remains on TASKS.md. Opaque origins and unobserved
stored-pointer crossings retain conservative answers; this row establishes no
new spatial interface contract or exact index identity.
