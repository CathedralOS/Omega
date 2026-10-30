# Generic whole-array production

Owning requirement: **STRUCTURAL-BORROW-IDENTITY / GENERIC-FIXED-ARRAY-VIEWS**
in [TASKS.md](../../../../../TASKS.md), claim
`1cfe3386ac36457daa85c6e8cc5164e7`. This is another production of the
[whole-array view fact](../README.md), preserving its four-entry extent and
`require_four(view.len)` operation across the same nine places.

`whole_view<Element, const Count: u64>` forms the ordinary shared range view
from `&[Element; Count]` and promises `result.len == Count`. The caller consumes
that returned fact directly. Every short twin changes the backing extent and
transport contract to three while retaining the consuming requirement four.
Runtime checks occur after that consumption and cannot establish its proof.

The [root](main.omg) and its [twin](invalid/main.omg) accompany
[local](local/main.omg), [field](field/main.omg),
[parameter](parameter/main.omg), [call result](call_result/main.omg),
[match join](match_join/main.omg), [const name](const_name/main.omg),
[nested machine](other_state/main.omg), [another machine](other_machine/main.omg)
and [target body](target_body/main.omg), each with its `invalid/main.omg` twin.
Additional compositions exercise a [shadowed const binder](shadowed_extent/main.omg),
a [nonzero field offset and selector](offset_selector/main.omg), and
[selectors before and after a stored reference](stored_reference_selectors/main.omg).
The latter observes both distinct backing arrays and both original addresses.

At fixed base `178897a3f89a7158387d7df5d4c14ee42e772701`, the unchanged frozen
root wrongly refused `whole_view`'s ensures clause. Optimized Linux producer
`1a7a8b4950900035d021861c09817bf49753337606c84af373d6d1958a2ff0e8`
is retained with its source/oracle and input identities. The same root also
refused on main `e4d9c74577f9ed221697b73113b045220364f9de`, independently
observed with optimized Windows producer
`9c54bc1e2ce01743cb51ae02e84b86fc74a5c5fd961f6923130d9e4e75f97c3b`.

The repair records immutable fixed-array extent on the completed actual
before a slice formal attenuates its type. Logical operator applications retain
exact open type/const binder identities under the ordinary application telescope.
Storage-address production, selection and independent target-row rejoin consume
physical path/selector geometry without requiring a standalone declaration for
an inline scalar field. Typed loan presentation and access checks stay with
their existing owners.

Validation uses producer `2a8584b4303a273e7881c7532211b0f80b2066d1`, with
verified effective optimization level 1 and matching complete producer input
identities on Linux x86-64 and Windows x86-64. Each host observed native exit 0
and Terminal Psi exit 0 for all 13 public positives and all 13 identical corpus
pins. All 13 short twins and ten additional [boundary controls](controls/)
reject for their authored reasons. Focused fixed-base comparisons cover the
changed generic/operator rules and all storage-address source readers, with no
lost existing outcomes. Qualification receipts, actual engine streams, profiles,
source bundles and the zero-deletion/original-byte golden guards remain in the
claim evidence; golden rows retain their actual producer observation stamp.

Establishment comes from the actual fixed-array type and completed range
contract. Original address and element-value checks establish backing identity;
all nine transports establish composition. Wrong extents/binders, narrowed or
replaced descriptors, conflicting writes, denied pointer traversal and both
selector bounds retain the soundness boundaries. Actual Terminal/native agreement
covers checked lowering, descriptor layout/calling plans, instruction/frame
realization and emitted PE/ELF execution on those two hosts.

All four hosted entry bindings are authored and all target-scoped source bodies
are checked. Matching macOS ARM and Linux ARM execution acceptance remains open;
these two-host observations do not establish it. On the development producer,
an exploratory array of returned slice descriptors lost its initializer length
before the consuming call. That frozen lead remains in the claim evidence for
the next production under this fact. This repair does not close the parent's
other indexed-borrow or cross-host recast requirements.
