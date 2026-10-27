# Retained array fields in original nested backing

Parent: [NORMALIZED-ABI-LOWERING](../../../../../TASKS.md); continuation of
[the nested record fact](../README.md).

Completion consumes Pending and returns the original nested array Buffer with its declared ResidentValue and MarkerValue predicates restored, permitting exact subtraction; C writes the middle cell to 73 while marker 91, neighbors 305419896 and 1515863100, and the earlier copy 37 stay unchanged.

`Buffer` owns three copyable `Cell` records in a fixed array. `ResidentValue`
is a closed scalar predicate (0 through 2147483647, excluding 42), while
`MarkerValue` bounds the marker from 0 through 100. The actual C provider receives
one original 32-byte allocation, writes the middle element to 41 at registration,
retains its pointer, and writes 73 at completion. After completion the caller
proves exact subtraction directly from the restored declaration predicates and
checks both neighbors, marker, observed output and the earlier scalar copy.
Neither an equality guard nor the old scalar contents establishes those predicates.

The nine places transport the same pending claim and original backing:
[local](local/main.omg), [field](field/main.omg),
[parameter](parameter/main.omg), [call result](call_result/main.omg),
[match join](match_join/main.omg), [const name](const_name/main.omg),
[nested machine](other_state/main.omg), [another machine](other_machine/main.omg),
and [target body](target_body/main.omg). Each has an `invalid/main.omg` twin
that reads the middle cell while its exclusive retained loan remains live.
The complete row and twins are pinned under
`tests/omega/{pass,fail}/facts/foreign_nested_array_backing_*`.
Every positive build declares all four hosted targets. The match invokes both
incoming branches with distinct route tags and checks the selected route.
The const place materializes a copyable array before registering its runtime copy;
a retained pending occurrence is never a constant.

[Computed completions](controls/computed_completions/main.omg) additionally passes
a forwarded pending result directly to completion and nests completion,
registration and completion in one strict argument graph. Both runs check the
same field, neighbor and snapshot oracles. The
[invalid constant](controls/invalid_const/main.omg) changes one array initializer
to 2147483648 and must reject for its false `ResidentValue` constraint.
Both controls are pinned alongside the row and included in the retained foreign
host gate.

At fixed source `44b2274fb223fa7e88d86144052b911cc6834632`, optimized producer
`4d336559156` (SHA256
`111d3e944aed98fbb3855790680e2fcbb555d5a95643d3fbdef8ad73ab08424e`)
rejects this valid local witness at `drop(buffer)`: all four declared leaf-domain
duties were lost after completion. The earlier unqualified array row runs in
native and Terminal execution on macOS and completed all three remote host jobs;
that evidence does not qualify the new predicate continuation on those hosts.

The source repair follows the actual captured lifetime inputs, their complete
root/leaf rosters and strict evaluated call graph. A normal consuming completion
restores only the declared pure scalar duties on the exact returned storage;
returned or overlapping live loans keep exclusion. Historical owned holder
identity is distinct from the mutable contents observed before a call.
Terminal catalog, reference and host-storage consumers preserve the same complete
scalar predicate. A declared scalar restriction can entail a proposed pure
closed scalar membership at its own exact path, but cannot establish an issuer,
Content policy, unknown prerequisite, stronger predicate or unrelated storage.
Existing explicit mutable preservation debts and access checks remain separate.

This implements [declared mutable field duties](../../../../../wiki/spec/language/dependent_values.md#mutable-arguments-and-declared-field-domains)
and [scalar declaration invariants](../../../../../wiki/spec/terminal-psi/structural_access.md).
The [applied safe boundary contract](../../../../../wiki/spec/build/foreign_storage.md#provider-owned-views-and-completion)
remains the provider trust point; the compiler does not prove opaque C compliance.
Geometry supplies no ownership or domain authority. C lifecycle traps and layout
assertions exercise actual retained foreign behavior.

Raw fixed-baseline sources, optimized producer/profile receipts, actual engine
outputs and retained products for this continuation live in
`/Users/zcanann/Omega-build/retained-refined-array-backing-2026-10-27/`.
The local nine-place row is qualified on macOS; new-predicate execution on the
other hosts remains pending. This row does not establish borrowed views,
tagged/dynamic carriers, routed independently qualified descendants,
pointer graphs or owned aggregate result publication.
