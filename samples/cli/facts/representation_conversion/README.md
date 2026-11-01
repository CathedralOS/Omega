# Selected fallible representation conversion

Parent: **LIBRARY-REPRESENTATION-FAMILIES**.

**Explicit fallible representation conversions preserve the semantic byte across two selected formats, and malformed input never yields a destination.**

The ordinary core `TryFrom<Source, Destination, Error>` contract selects all
three endpoints explicitly. `convert` delegates to that exact implementation;
`compose` decodes to the semantic intermediate before invoking the selected
encoder. An error from the first operation returns without invoking the second.
Neither operation acquires buffers, allocation, services or capture state.
Conversions that need those resources must carry them in their source request.

The authored first format has tag 165 and complements the semantic byte. The
second has tag 90 and flips its high bit. The witness decodes first-frame bytes
165/212 to semantic byte 43, encodes second-frame bytes 90/171, and decodes the
second format back to 43. A first frame with tag zero must fail. The two frame
types deliberately have equal geometry; their selected maps remain distinct.
Each adjacent invalid control changes only the selected first decoder to the
second format's decoder, which cannot satisfy the first endpoint application.

This differs from the nearest `fixed_layout_storage` row: that row preserves an
established field fact through representation storage and compatible views.
This operation executes two fallible conversion maps with different semantic
endpoints and consumes their result cases. It is not a storage recast or layout
coercion. The negative obligation is exact conversion-map satisfaction, rather
than the nearest row's constructor field bound.

The established Linux places are local, field and parameter, with exact corpus mirrors
and all four hosted ProgramEntry bindings. Their source checks must establish
each exact map and result payload. Composition must preserve payload custody
and distinguish success from failure through checked-to-Terminal lowering.
Provider settlement and calling plans must retain both frame and result-case
homes; native instruction selection, encoding and emission must produce the
same byte observations as Terminal execution. The native oracle traps on a
wrong tag, payload, decoded value or malformed-frame result.

On retained optimized Mac producer `3d5c5150f08151ca111b2e6b31545d1646085611`
(image `090ee60d87c91f8e13b23f0bd14c8f323ed4e3946f524ae7833557cfae419bee`),
ordinary admission rejects the core `Result` declaration because the compiler
reserves that name as an unimplemented builtin. Removing that obsolete
reservation in both producers permits ordinary library ownership while leaving
the primitive shadowing fence intact.

The first hosted candidate `de69ac978923e0abb6b92d773a5f14a46a249768`
(run 41337846412, optimized Linux image
`f487f925ba43e5f280d5914e55a4843f014cc3e25fd05926270680003101a290`)
passes the primitive control and all three wrong-endpoint controls. Each
positive gets through the new ordinary `Result` declaration, then rejects the
terminal call `Second::try_from`: its evidence binder already has an exact
requirement-child symbol, but terminal-call normalization only admitted an
unresolved implementation member. Normalizing the exact child through the
ordinary expression-call route preserves selected-map specialization instead
of inventing a body or treating it as an internal control edge. The producer
preparation took 830.546 seconds and checking took 7.693 seconds. The failed
run is retained as failed.

Candidate `0b47ca900d70c293c27879f640e914684aa58139` (run 41339570194,
optimized Linux image
`de6b05740701a4858c29d7bd4cfb5521dab792e4f50659daf27981e2a8254b02`)
gets past terminal selection, then refuses `compose` at its generic stored-output
relation. The flow graph names the selected public requirement while the authored
call keeps its evidence child; the output-obligation query compared only raw
call symbols and mistook that exact edge for a missing relation. It now rejoins
the bound-owned child/signature edge before reading the occurrence's complete
applied contract. Unknown inventories and concrete writable slots keep their
existing fail-closed rules. Preparation took 678.634 seconds; checking took
16.514 seconds.

Candidate `dde0f3a03bcd5f2ba4027428611db2f9b2b4fb47` passes the complete
scoped run [41341600988](https://github.com/CathedralOS/Omega/actions/runs/41341600988).
Optimized Linux image
`4933b4983b31d9f563e9941b32ce213de6bee1fcff44f8910959d1a9ad66f04b`
executes all three unchanged positives at native/Terminal exit zero. All three
wrong-endpoint twins and both independent controls reject for their intended
reasons. Preparation took 648.217 seconds, authentication 1.756 seconds,
checking 23.268 seconds, corpus execution 16.547 seconds and sample execution
38.618 seconds. The eight canonical checking rows retain their actual source,
timings and diagnostics; checking used its default macos_arm64 profile, while
the execution observation is Linux x86-64. The source is retained on
`evidence/representation-conversion-20261101-dde0f3a03bcd`; historical failed
runs remain failed. These are observations of the recorded producer, not
re-executions of later documentation commits.

The independent [primitive shadow control](controls/primitive_shadow/invalid/main.omg)
continues to reject an ordinary declaration named `Int`; retiring `Result` does
not weaken the actual primitive fence.

The [stored-output control](controls/stored_output/invalid/main.omg) installs a
reference through a selected generic map, then tries to mutate that reference's
backing before the destination's last read. Retained Mac090 rejects that exact
conflict in 7.064 seconds. The repaired query must retain the same refusal;
joining an evidence edge must not erase a concrete output slot or its loans.

The final six places are established with matching endpoint twins:
[call result](call_result/main.omg), [match join](match_join/main.omg),
[named constant](const_name/main.omg), [nested machine](other_state/main.omg),
[another machine](other_machine/main.omg) and
[selected target implementation](target_body/main.omg). [Run 41344570814](https://github.com/CathedralOS/Omega/actions/runs/41344570814)
qualifies exact source `1763580d349887a897aa594ce7b7bc14cb27f0ac` with optimized
Linux image `cd08536ad00ece089d112e9ae0ebfd39e8f8a6b5a3f89d1207fd9290b0f60fea`:
all six positives and their exact corpus mirrors exit zero in native and
Terminal execution, and all six endpoint twins reject for the intended
`TryFrom` bound. Preparation took 1117.499s, authentication 1.790s,
checking 31.084s, corpus execution 24.313s and sample execution 49.020s. The
canonical checking rows keep their actual default macos_arm64 profile and
revision; native agreement is Linux x86-64. No compiler or library repair was
needed for this batch. Its source bundle and
`evidence/representation-conversion-places-20261101-1763580d3498` retain that
exact measured source, separate from later documentation or integration commits.
The call-result and match places exercise both valid and malformed outcomes;
swapping their routes changes 43 to 0 and traps. Constants retain whole first-frame
values; nested and relay calls carry the same explicit frame into conversion.
Build selects named frame-delivery implementations for the target place, and
separate 17/29 markers make the selected provider observable.

All nine places and their exact endpoint twins are now established on Linux
x86-64. The first three retain their earlier producer observations above;
those unchanged programs were not repeated for this final batch. Both
independent primitive-shadow and stored-output controls retain their original
successful rejection evidence.

Other hosts, universal agreement proofs,
the `Representation` agreement bundle, reflection-specialized generic codecs,
independent-package/resource-backed adapters and the parent capability remain
open. Finite runtime observations do not establish derived codec trust or a
universal round-trip theorem.

Rules: [representation conversions](../../../../wiki/spec/layouts/codecs.md#representation-conversions),
[exact satisfaction](../../../../wiki/spec/language/conformances.md), and
[case patterns](../../../../wiki/spec/language/patterns.md).
