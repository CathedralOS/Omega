# Selected layout in active-case visitation

Parent: `LIBRARY-REPRESENTATION-FAMILIES`; composition of the existing
[active-case fact](../../README.md), with the same fact sentence and operation.

A common field and an active payload use
`LayoutStorage<Payload, Spread<40u64>>`. Typed visitation must preserve the exact
policy application while borrowing its fields. The callback observes every
saved scalar, common-before-payload order, one selected case, and selected
40-byte/8-byte geometry. A nullary case still visits its common field exactly
once. Independently initialized backing observes the three sparse fields.

The [invalid twin](invalid/main.omg) changes only the callback constraint to
`Spread<48u64>`. Both applications intentionally return identical geometry;
the callback body accepts both, keeping its own formation valid. Only the
actual field application's coverage obligation is removed.

Before repair, authenticated opt-level 1 CLI
`090ee60d87c91f8e13b23f0bd14c8f323ed4e3946f524ae7833557cfae419bee`,
source `3d5c5150f08151ca111b2e6b31545d1646085611`, refuses the unchanged
composition: `a schema identity requires a resolved declaration: nominal
reference 40 in named(integer-const(40))`. The affected graph traversal is
unchanged at the repair base. This is a retained-producer baseline, not an
execution of the new candidate. Fresh candidate observations remain pending.

The repair reads argument roles from the exact declaration's telescope for
both graph construction and replay. Numeric spelling supplies no authority,
and the policy owner/application remains an exact nominal edge. This does
not add programmable union layout or extend the C record policy to sums.
Broader schema/case composition and parent acceptance remain open.
