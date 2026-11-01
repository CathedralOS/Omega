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

The initial places are local, field and parameter, with exact corpus mirrors
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
run is retained as failed; native/Terminal acceptance is still pending.

The independent [primitive shadow control](controls/primitive_shadow/invalid/main.omg)
continues to reject an ordinary declaration named `Int`; retiring `Result` does
not weaken the actual primitive fence.

The remaining six transport places, other hosts, universal agreement proofs,
the `Representation` agreement bundle, reflection-specialized generic codecs,
independent-package/resource-backed adapters and the parent capability remain
open. Finite runtime observations do not establish derived codec trust or a
universal round-trip theorem.

Rules: [representation conversions](../../../../wiki/spec/layouts/codecs.md#representation-conversions),
[exact satisfaction](../../../../wiki/spec/language/conformances.md), and
[case patterns](../../../../wiki/spec/language/patterns.md).
