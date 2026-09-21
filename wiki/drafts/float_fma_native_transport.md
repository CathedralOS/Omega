# FLOAT-FMA-NATIVE-TRANSPORT — record

Re-verified at `94e764a6da` on linux x86-64 (cargo; `mbx` absent on this
host). The row re-mines the transport legs enumerated on canonical sibling
X86-FMA-PROVIDER-TRANSPORT; measured state at tip:

## Leg (a) — landed

`legalize scalar-FMA unit operations against exact constant sources`
(`2a6e06f1c496`, on main since ~21:09Z): `legalization/scalar_graph_input/
target/unit.rs` dispatches `AbstractOperation::NearestIeeeFloatFusedMultiplyAdd`
into `unit/ieee_float.rs::validate`, binding psi_operation/result/format,
binding the settlement's terminal_operation/format, and requiring
left/right/addend to name the exact retained `IeeeFloatImmediate` records
so same-valued or same-typed producers cannot substitute; the result lands
as an `IeeeFloat` home requirement with the format's float shape. Lane
re-verified: `nextest -p target-operations-to-selected-instructions
-E 'test(~ieee_float)'` — 5/5 including
`fused_multiply_add_replays_exact_constant_sources` and
`fused_multiply_add_rejects_any_operand_drift`.

## Legs (b)/(c) — still fenced at tip

- `native_realization/object_emission.rs:34-38` still returns
  "FMA provider transport is not implemented in the common instruction
  pipeline" — pinned PASS by
  `windows_imports_and_mxcsr_custody::retained_x86_fma_and_source_evaluated_import_stop_at_fma_transport`
  (64.5s).
- `native_realization/optimization_stage.rs:27-30` still returns
  "optimized nearest-FMA custody — retained nearest-FMA occurrences
  require the ordinary custody-preserving pipeline".
- No `FusedMultiplyAdd`/`VFMADD`/`x86_scalar_fma` references exist in
  `selected-instructions-to-selected-instructions/src`,
  `selected-instructions-to-register-homes/src`, or
  `machine-emission/src` — the downstream carry (s2s carry, s2rh XMM
  allocation, post-allocation machine plan, machine-emission VFMADD +
  canonical MXCSR envelope + object records) is unimplemented.

## Claim map over the residual surfaces

- `legalization/` + `target/control_flow/sources.rs`:
  X86-FMA-PROVIDER-TRANSPORT (Jarod / w9), expires 09:07Z — canonical
  owner, active.
- `native_realization/{object_emission,program_entry,native_realization,
  optimized_fragment_projection,realization_request}.rs` + `lib.rs` +
  `optimized_semantic_wrapper_*`: UEFI-PHYSICAL-SEMANTIC-ENTRY (z88),
  expires 08:44Z.
- `selection/construction`: CALLBACK-PRIVATE-MATERIALIZATION (z55),
  09:13Z.
- `s2rh/unsequenced_spill_stages`: POC-SPILL-FAMILY-SEQUENCING, 06:53Z.
- `machine-emission/entry_exit_stub.rs`: EXCEPTION-ROOTS-AND-TIMER,
  08:59Z.
- `optimization_stage.rs` is the one unfenced residual file, but its
  fence is only removable once the transport it gates exists.

Conclusion: no unfenced implementable slice remains for this stub — the
transport work belongs to the canonical X86-FMA-PROVIDER-TRANSPORT claim
under its own sequencing. This pass contributes the re-verified landing
state of leg (a) and the exact residual map.
