# C scalar-record layout

Fact: library C layout preserves scalar values with C ABI field offsets, trailing padding and transparent alias identity.

Parent: `LIBRARY-REPRESENTATION-FAMILIES` in [TASKS.md](../../../../TASKS.md).
The ordinary [core policy](../../../../source/library/core/c_layout.omg)
computes scalar placement from the target-resolved schema. It selects no compiler
format registration and grants no foreign or storage authority.

The root record has `u8`, `u64` and `u16` fields. Its C member offsets are 0, 8
and 16, with extent 24 and alignment 8. Values 17, 42 and 9 survive interchange
between `CLayout<Record>` and its complete `LayoutStorage` expansion. A shared
view of fully initialized, aligned `[u64; 3]` backing observes the same fields
at the independently authored offsets. We do not recast uninitialized record
padding to bytes. Size and alignment queries through the alias check the complete
extent and alignment. Every observed discrepancy traps.

`invalid/wrong_policy` substitutes a distinct named policy with identical geometry;
transparent aliases do not erase policy identity. The other controls reject a
nested native record, numbered fields and a sum. The current policy supports
complete nonempty, unnumbered scalar records only; it refuses repeated or nested
shapes, empty records and sum cases rather than claiming their native Omega
geometry is C ABI evidence. Ordinary scalar domains and access obligations remain
in force; policy geometry grants no atomic or concurrent access permission.

`controls/static_type_argument` independently passes an alias as an ordinary
generic call's static type argument. Its twin changes only an erased alias
argument from copyable `u64` to affine `Resource`; the alias still owes `[copy]`
when its expansion is simply `u64`. This checks ordinary formation rather than
introducing a layout-query exception.

`invalid/unused_initializer` keeps an unused declaration's target-dependent
constant-formation obligation: subtracting 17 from the 16-byte packet extent
must reject even when no executable body reads that constant.

## Nearest existing row

[Imported representation aliases](../transparent_generic_aliases/imported_representation/README.md)
already prove alias/explicit identity with two `u64` fields placed at authored
constant offsets. This row adds reusable schema-driven C placement: mixed-width
`u8/u64/u16` fields require gaps at bytes 1–7 and 18–23, offsets 0/8/16 and a
24-byte extent. Independent initialized backing and size/alignment observations
establish that physical placement. The ordinary policy walks reflected fields
and computes padding; it does not contain a per-record placement table.

## Current evidence

Optimized Linux x86_64 producer `4e6a0f454c1c57634066be59be9d2a4c2a64fe05`,
image `651947e3b3ce37e7b2dce51abd43c5c1e35bc26a698a39af14087b07edf3d574`,
executes the complete root and the existing nominal size-query local with native
and Terminal exit 0. Pilot [41277462630](https://github.com/CathedralOS/Omega/actions/runs/41277462630)
reused the identical producer bytes: preparation 170.904 seconds, checking 16.822
seconds, and native/Terminal execution 13.959 seconds.

The same producer's [four-case observation](https://github.com/CathedralOS/Omega/actions/runs/41276586247)
checks both positives and rejects both arithmetic controls. The unused initializer
produces bounds `[-1, -1]`; the payloadless nominal sum uses a four-byte tag, so
its subtraction produces `[-4, -4]`. Both refuse values outside the `u64` bounds.
The nominal expected message initially assumed `-1`; it now matches the saved
exact `-4` refusal. Program sources and obligations are unchanged. That workflow
remains recorded as failed because of the old expectation; its actual refusals
are reused, not relabeled as a green workflow. Preparation was 570.377 seconds
and checking 24.038 seconds.

The ordinary static-alias generic positive/copy-bound twin and the four
schema/policy controls retain their passing observations from optimized producer
`47e16a76404460206b0a7382be7b93dd3d3de6db`, image
`46349017e1955bd3102a8ac99497818bd07e85fdc398520b63d286abb4aeb892`, in
[pilot 41274535924](https://github.com/CathedralOS/Omega/actions/runs/41274535924).
The later changes affect deferred scalar formation and evaluation ordering;
these unchanged cases contain no deferred layout-query arithmetic. Their alias
admission, policy geometry and incompatibility judgments are unchanged. That
first workflow's accepted arithmetic negative and internal initializer error
remain historical failures, repaired by the later producer.

Validation uses the focused runtime and refusal cases; no full Rust suite was run. Earlier query-free Mac ARM64
diagnostics are historical, not qualification of this complete root.

The nine transport places, complete nested C records/platform migration,
other-host execution and the parent capability remain open. The installed
SDK15.5 arm64 oracle confirms Darwin stat 144/8 and timespec 16/8, including
nanoseconds and reserved fields; no platform source migration is claimed here.

## Current integration

Main publishes calling-policy code with the shared Product and closes its
actual invocation under a distinct scalar receipt. This replaces the earlier
producer-scoping workaround; that redundant layer was removed. The remaining
compiler changes are ordinary static-alias normalization and early refusal of
independently ready scalar debts.

Integrated optimized Linux source `f2e527c568c2f1c685fdb40c0be45cdf8f57509d`
(on published main `fbd577583f3`), CLI image
`be4025835a8f589976a3d5223f71865bb0c8c0b35b48d301ba69a2ce80815d9a`,
passes the exact four affected cases in
[pilot 41285540003](https://github.com/CathedralOS/Omega/actions/runs/41285540003).
Both positives pass checking and native/Terminal execution with exit 0. Both
arithmetic controls reject with their exact expected bounds. Preparation took
1111.795 seconds, checking 22.970 seconds and native/Terminal execution 12.828
seconds. Historical observations above retain their actual producer identities;
the unchanged source alias and schema/policy controls are reused as described.
No further compiler build is required for documentation or queue revisions.
