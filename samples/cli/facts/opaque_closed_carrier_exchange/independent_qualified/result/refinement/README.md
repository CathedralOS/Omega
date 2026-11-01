# Inherited routed qualification across the result edge

Parent: NORMALIZED-ABI-LOWERING. Another production of the
[qualified-input row](../../README.md), through the
[checked-result production](../README.md); it copies the row's sentence.

Routed opaque input membership survives transport and an independent installed call, preserving libc host numbers 66051 and 513 and replacement 1027 while retaining the disjoint sibling.

The `Classified::Ready` child inherits `Classified` and its exact `classify`
issuer. The checked factory qualifies the issuer's result as `Ready`; the
receiver transports it into the qualified consuming call without reclassifying
it. A checked component return may present `Parent + Child` as `Child` only
after graph entailment succeeds in both directions. Raw host results keep their
separate provenance gate.

The [local witness](local/main.omg) differs from the checked-result local
witness only by the `Classified::Ready` annotations and two non-zero canary
fields beside the replaced field. Its [reject twin](local/invalid/main.omg)
keeps the receiver identical; [its producer](producer_invalid/main.omg) omits
only the issuer call and rejects at its own return.

At base `7399f90b98d` the compiler refused this parent-to-child qualification
at source checking: a missing `Classified::Ready` return requirement and a cast
that could not establish a non-vacuous domain. The repair is
[PR #302](https://github.com/CathedralOS/Omega/pull/302); it changed Psi domain
inheritance, not transport through any place.

Controls:

- [Raw host result](controls/raw_native_result/main.omg) and
  [checked-component laundering](controls/component_launders_raw_result/main.omg)
  reject native admission at `UncheckedFactory::make`.
- [Scalar parent tag](controls/scalar_parent_tag/main.omg), pinned as
  `opaque_inherited_result_scalar_parent_tag_exit`.

The pinned copies are
`tests/omega/{pass,fail}/facts/opaque_independent_inherited_result_through_local_*`.
Before this fold the production carried all nine places; the eight others
repeated the row's transport, and their twins all reported the same producer
diagnostic as the checked-result twin.
