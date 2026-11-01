# Qualified membership produced by a checked independent result

Parent: [NORMALIZED-ABI-LOWERING](../../../../../../TASKS.md#normalized-abi-lowering).
This is another production of the [qualified-input row](../README.md), not a
separate row, and copies its sentence.

Routed opaque input membership survives transport and an independent installed call, preserving libc host numbers 66051 and 513 and replacement 1027 while retaining the disjoint sibling.

The [local witness](local/main.omg) differs from the row's
[local place](../local/main.omg) in one line: `classify` runs inside the checked
provider's `classified_make` instead of in the caller. The membership then
crosses the installed result edge before the same transport and the same
qualified `AddressBridge::classified_host` call. Both directions are joined by
one routine, `component_memberships.rs::admit_memberships`, so the row's nine
places already cover the transport after the edge.

The [reject twin](local/invalid/main.omg) keeps the receiver identical. Its
[producer](invalid_producer/main.omg) omits only the `classify` call, and the
producer's own return rejects with `cannot prove default-domain field
requirement`. That diagnostic does not depend on the receiving place.

| Witness | Positive | Reject |
| --- | --- | --- |
| Local | [local](local/main.omg) | [issuer omitted](local/invalid/main.omg) |
| Inherited child domain | [refinement](refinement/README.md) | [issuer omitted](refinement/local/invalid/main.omg) |

Controls specific to the result edge:

- [Raw host result](controls/raw_native_result/main.omg) and
  [checked-component laundering](controls/component_launders_raw_result/main.omg)
  type-check but must reject native admission at `UncheckedFactory::make` with
  `cannot establish result provenance owned by another issuer`. A real C leaf
  is not the existing issuer. `tools/ci/compiler_foreign_records.py` runs both.
- [Import alias](controls/import_alias/main.omg) and
  [renumbered catalog](controls/renumbered_catalog/main.omg) keep the issuer's
  identity when the receiver names or numbers its catalog differently.

The result provenance repair landed in
[PR #295](https://github.com/CathedralOS/Omega/pull/295). The pinned copies are
`tests/omega/{pass,fail}/facts/opaque_independent_qualified_result_through_local_*`
and `opaque_independent_qualified_result_{import_alias,renumbered_catalog}_exit`.
Before this fold the production carried all nine places; the eight others
repeated the row's transport, and their twins all reported the same producer
diagnostic.
