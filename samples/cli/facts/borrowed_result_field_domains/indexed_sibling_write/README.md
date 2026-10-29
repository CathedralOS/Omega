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
wrong-slot, wrong-element and overly wide stores are intended to reach bad()
and crash Trap; that source-level prediction has no engine observation yet.

| Requirement | Source design and remaining evidence |
| --- | --- |
| Establishment | Concrete cells satisfy the standing Ascii field domain; choose/relay return the checked reference and inspect consumes its field fact after the indexed sibling store. Checking remains pending for the eight new places. |
| Identity and lifetime | The outer selection and inner stamp selector are independent. Exact destination versions must invalidate overlapping storage while preserving unrelated payload facts. No lifetime or carrier type supplies an exact origin. |
| Composition | The nine existing transports retain the same fact, bound and operation. Both match arms are exercised; parameter and provider contracts retain the selector bounds. |
| Soundness | Every twin adds the same overlapping non-ASCII payload store and retains the positive's contracts and canaries. All nine intended rejection reasons still require observation. |
| Psi 05–07 | Sources require view selection, nested indexed stores, calls and crash routing through Terminal Psi. No Terminal execution is claimed. |
| Omega 00–02 | All positives author four hosted ProgramEntry bindings; target_body also selects the provider for each host. Layout, calling and provider realization remain unverified. |
| Omega 03–06 | Intermediate/final stamp values and payload bytes are intended to expose wrong coordinates and clobbers across calls. Native observations remain pending. |
| Omega 07–09 and emission | Expected exit 0 is authored for each positive; actual native/Terminal agreement and retained products remain pending. |

The published fixed base is 861910090c171e544f99a7a0605c34bd49f50dad.
Historical H2 evidence is retained in
build/borrowed-result-indexed-sibling-20261029/h2-source-snapshot.json and
baseline-h2-observations.json. Its actual optimized B0 CLI producer is
5260151f7c061451f3f72d570daa3340e653c3e8. The frozen local positive exited 1
at inspect's Ascii obligation and both Table storage return obligations; its
overlapping twin also exited 1 with the authored inspect refusal. This is
historical source-check evidence for the exact H2 local files, which this
fixture expansion leaves unchanged. It is not engine acceptance or a baseline
claim for the eight new places. The earlier H1 result predates the independent
stamp selector and remains separate history.

Optimized R1 CLI producer aa686301b549825cd3caf76e9fa64d9f9368c56f checks
the unchanged H2 local positive with exit 0. Its twin still exits 1 with the
authored inspect Ascii refusal. The exact source hashes and commands are retained
in candidate-r1-local-observations.json beside candidate-r1-producer.json and
the raw local diagnostics in the same build directory. These are local source
checking results only: no native/Terminal execution is claimed, and source
results for the eight new places remain pending.

All nine positives and twins are copied under tests/omega/{pass,fail}/facts/
using borrowed_result_field_domains_indexed_sibling_write_<place>_exit and
borrowed_result_field_domains_indexed_sibling_write_<place>_rejected. Positive
pins author native and interpreter exit 0; negative pins copy the inspect
diagnostic. These are source pins awaiting execution, with no corpus golden
recording in this fixture expansion. The parent task retains its remaining
acceptance; neither this production nor the parent is declared complete.
