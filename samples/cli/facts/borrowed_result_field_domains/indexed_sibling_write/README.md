# Borrowed field domain after an indexed sibling write

Borrowed results retain their checked field domains and selected element, so inspection reads that element.

Owning task: [CANONICALIZE-SCALAR-RANGE-CONTRACTS](../../../../../TASKS.md),
field propositions through nonliteral and indexed writes with current storage
versions. This is another production of the existing borrowed-result fact,
retaining Ascii[4], two Cell elements, index < 2 and the inspect operation.
Each Cell has two unqualified u64 stamps. An independent stamp_index < 2
selects the sibling slot written before inspect consumes the storage predicate.
The indexed destination must preserve the disjoint storage field's proposition
without inventing exact pointer or element identity.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [program](../local/indexed_sibling_write/main.omg) | [twin](../local/indexed_sibling_write/invalid/main.omg) | Frozen H2 returned reference. |
| field | [program](../field/indexed_sibling_write/main.omg) | [twin](../field/indexed_sibling_write/invalid/main.omg) | Returned reference stored in Held.selected. |
| parameter | [program](../parameter/indexed_sibling_write/main.omg) | [twin](../parameter/indexed_sibling_write/invalid/main.omg) | Returned reference and independent stamp selector passed to consume. |
| call result | [program](../call_result/indexed_sibling_write/main.omg) | [twin](../call_result/indexed_sibling_write/invalid/main.omg) | Additional carry return before the write. |
| match join | [program](../match_join/indexed_sibling_write/main.omg) | [twin](../match_join/indexed_sibling_write/invalid/main.omg) | Both reference-producing arms executed. |
| const name | [program](../const_name/indexed_sibling_write/main.omg) | [twin](../const_name/indexed_sibling_write/invalid/main.omg) | FIRST and SECOND construct the same concrete cells. |
| nested machine | [program](../other_state/indexed_sibling_write/main.omg) | [twin](../other_state/indexed_sibling_write/invalid/main.omg) | Explicit arguments forwarded to nested onward. |
| another machine | [program](../other_machine/indexed_sibling_write/main.omg) | [twin](../other_machine/indexed_sibling_write/invalid/main.omg) | Explicit arguments forwarded to sibling onward. |
| selected provider body | [program](../target_body/indexed_sibling_write/main.omg) | [twin](../target_body/indexed_sibling_write/invalid/main.omg) | Selection, indexed write and inspect inside Selected.get, returning u8. |

Every place performs selected.stamps[stamp_index] = 0xF1 then inspect(selected).
The twin adds only selected.storage[0] = 255u8 between those operations, and its
authored diagnostic requires the missing Ascii field obligation at inspect.
No caller guard restores that obligation. The provider place uses ordinary
Build selection for all four hosted targets and needs no opaque boundary
promise about a returned pointer's relation to Table.

Main crosses the independent selectors: first outer element 0 / inner slot 1,
then outer element 1 / inner slot 0. Initial stamps are [11,17] and [13,19].
After the first call it captures all four stamps; after the second it checks
final stamps [11,0xF1] and [0xF1,19], returned bytes 65/66 and all eight payload
bytes. These footer checks follow the consuming calls. They are runtime
canaries, never guards supplying inspect's own domain requirement. Missing,
wrong-slot, wrong-element and overly wide stores reach bad() and crash Trap.
Counterfactuals omitting the stamp store or using the outer selector for the
inner slot were executed: both yielded Terminal Trap and native signal 5.

| Requirement | Evidence |
| --- | --- |
| Establishment | Concrete cells establish Ascii; choose/relay carry the checked field promise, and inspect consumes it after the indexed sibling write in every place. No caller guard establishes it. |
| Identity and lifetime | Independent cell/stamp selectors, four stamp canaries and eight payload bytes distinguish both selected values, preserved neighbors and writes. Only complete aligned possible storage is projected; it does not become exact pointer identity or access authority. |
| Composition | All nine transports retain the sentence, bound and operation. Both match arms run; parameter and selected-provider contracts include both selector bounds. The provider performs selection/write/inspection internally and returns u8. |
| Soundness | All nine twins reject inspect's missing Ascii obligation after the sole added overlapping byte write. Stale descriptor, overwritten result, recast backing and borrowed-storage controls retain their expected behavior. |
| Psi 05–07 | All nine positives execute through Terminal Psi with exit 0, preserving calls, views, indexed stores and crash routes. |
| Omega 00–02 | All nine positives realize their authored macos_arm64 entry and agree with Terminal Psi. Four hosted entries are authored; execution on the other three hosts is unverified. |
| Omega 03–06 | Native exit 0 establishes selected-cell and independent stamp-slot behavior across calls, with the canaries and payload bytes checked afterward. |
| Omega 07–09 and emission | Retained native images execute with exit 0 and agree with Terminal Psi. Deliberately missing and misdirected stores trap in both engines. |

Fixed base: `861910090c171e544f99a7a0605c34bd49f50dad`. Frozen H2 local positive
wrongly refused inspect's Ascii obligation and both Table storage return obligations;
its overlapping twin also refused. Actual optimized B0 producer was
`5260151f7c061451f3f72d570daa3340e653c3e8`; compiler inputs match the fixed base.
The eight other places were added afterward and have no claimed B0 observation.
H1 predates the independent stamp selector and remains separate history.

Fresh optimized R1 CLI producer `aa686301b549825cd3caf76e9fa64d9f9368c56f` ran
15 positives (nine places and six focused controls) with native/Terminal exit
0/0, 16 invalid controls with their authored rejection reasons, and two Trap
counterfactuals. Actual optimized corpus-runner producer
`cdaaea7fffc3ceee150ec935c69c6a72d3969760` checked 31 pinned source
fixtures (15 accepted, 16 rejected), and ran all 15 positive pins with native/Terminal
exit 0/0. These producers share the compiler input tree; their source revisions
remain distinct. Focused whole-range `[..]` and bounded-range `[0..2]` compositions each
run with native/Terminal exit 0/0; their overlapping twins reject inspect's
Ascii requirement. Their source snapshots and products are under `range-r1/`
in the evidence directory. The affected Rust crate check and captured-window
coordinate test pass. Existing selected golden semantics and every unselected row are unchanged.

Evidence: `build/borrowed-result-indexed-sibling-20261029/`, including frozen
`h2-source-snapshot.json`, `baseline-h2-observations.json`, producer receipts,
`validation-r1/customer-validation/`, `validation-r1/fault-validation/`,
`record-corpus-{source,both}.json` and `golden-content-audit.json`. All nine
positives and twins are pinned under `tests/omega/{pass,fail}/facts/` using
`borrowed_result_field_domains_indexed_sibling_write_<place>_{exit,rejected}`.
The owning scalar-contract task retains its other requirements.
