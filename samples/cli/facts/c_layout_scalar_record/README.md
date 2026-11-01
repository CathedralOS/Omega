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

## Current evidence

Optimized Linux x86_64 producer `47e16a76404460206b0a7382be7b93dd3d3de6db`,
image `46349017e1955bd3102a8ac99497818bd07e85fdc398520b63d286abb4aeb892`,
executes the complete root (including alias extent/alignment queries), the ordinary
static-alias generic call, and the existing nominal size-query positive. All three
agree between native and Terminal execution. The five schema/policy/copy-bound
controls reject as intended. Pilot 41274535924 took 822.814 seconds to prepare the
optimized producer and 35.463 seconds for its selected case stage.

The same run exposed two remaining arithmetic failures: the existing nominal
subtraction control was accepted, and the unused initializer reached an internal
malformed-operation refusal. Source formation must remain independent of runtime
body publication, and ready scalar obligations must reject before executing a
root that would overflow. The two frozen programs retain their arithmetic;
their expected messages name the canonical target-closure range refusal for
`[-1, -1]` outside the `u64` bounds. Fixes and affected execution are pending;
this pilot is not an overall pass. Earlier query-free Mac ARM64 diagnostic
observations remain historical and are not qualification of the full witness.

The nine transport places, complete nested C records/platform migration,
other-host execution and the parent capability remain open. The installed
SDK15.5 arm64 oracle confirms Darwin stat 144/8 and timespec 16/8, including
nanoseconds and reserved fields; no platform source migration is claimed here.
