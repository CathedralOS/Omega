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
execution of the new candidate. Candidate `0e3420875128245c5a7704d7e875aa7ff7a057d0` passes that schema
refusal and rejects the policy twin for the intended obligation, but its
positive then refuses the independently initialized backing recast: `runtime
indexed value has no retained subject provenance`. Diagnostic reductions with
the unchanged retained runtime-index consumer isolate the recast; default
construction and the reduced program without recasting check. Native execution
was not reached in that candidate. Candidate `dd3a6c22f839ebb2c7d45df8b54938df8d5ef6a0` then passes
checking and the authored native exit 0. Its policy twin, an existing const
policy mismatch, and an existing captured-runtime-index mismatch all reject
for their intended obligations. Terminal declines reference formation at
operation 35; no sample-stage observations were reached. The corresponding
checking rows retain their actual producer and default `macos_arm64` check
target; native execution was Linux x86_64. Dual-engine acceptance remains open.

The repair reads argument roles from the exact declaration's telescope for
both graph construction and replay. Numeric spelling supplies no authority,
and the policy owner/application remains an exact nominal edge. This does
not add programmable union layout or extend the C record policy to sums.
Broader schema/case composition and parent acceptance remain open.

The runtime-index consumer must use a prepared recast's exact produced loan
application for const-only indices. Its source backing has a different shape;
ordinary recast validation still owes geometry, validity and authority. Targets
with runtime value indices retain their existing source-capture checks. The
original fact and its initialized-backing oracle remain unchanged.
