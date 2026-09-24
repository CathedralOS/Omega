# FLOAT-FMA-NATIVE-TRANSPORT — record

Re-verified at `75650d2e94` on linux x86-64 (cargo; `mbx` absent on this
host). The row re-mines the transport legs enumerated on canonical sibling
X86-FMA-PROVIDER-TRANSPORT; measured state at tip:

Delete this record once legs (b) and (c) land and the three
native-realization FMA transport fences are removed.

## Leg (a) — landed

`legalize scalar-FMA unit operations against exact constant sources`
(`2a6e06f1c496`, on main since ~21:09Z): `legalization/scalar_graph_input/
target/unit.rs` dispatches `AbstractOperation::NearestIeeeFloatFusedMultiplyAdd`
into `unit/ieee_float.rs::validate`, binding psi_operation/result/format,
binding the settlement's terminal_operation/format, and requiring
left/right/addend to name the exact retained `IeeeFloatImmediate` records
so same-valued or same-typed producers cannot substitute; the result lands
as an `IeeeFloat` home requirement with the format's float shape.
`control_flow/sources.rs::definition` supplies the same `Home` row so
downstream consumers replay it.

## Legs (b)/(c) — still fenced at tip

- `native_realization/object_emission.rs:39-44` still returns
  "FMA provider transport is not implemented in the common instruction
  pipeline" — pinned PASS by
  `windows_imports_and_mxcsr_custody::retained_x86_fma_and_source_evaluated_import_stop_at_fma_transport`.
- `native_realization/optimization_stage.rs:27-32` still returns
  "optimized nearest-FMA custody — retained nearest-FMA occurrences
  require the ordinary custody-preserving pipeline".
- `native_realization/program_entry.rs:44-46` still returns
  "receipt-coupled ProgramEntry realization does not yet consume retained
  IEEE-FMA occurrence custody".
- No `FusedMultiplyAdd`/`VFMADD`/`x86_scalar_fma` production references
  exist in `07_selected-instructions-to-selected-instructions/src`,
  `08_selected-instructions-to-register-homes/src`,
  `register-homes-to-post-allocation-machine/src`, or
  `machine-emission/src` beyond the `x86_fma.rs` encoder seam —
  the downstream carry (s2s carry, s2rh XMM allocation, post-allocation
  machine plan, machine-emission VFMADD + canonical MXCSR envelope +
  `x86_scalar_fma*` object records) is unimplemented.

## Implementing surface the transport needs

The bounded Unit lane makes each FMA a real stream member, so leg (b)'s
first edit is a `SelectedInstructionKind` variant (format + settlement
custody, three XMM-class operand uses, one XMM result def) in
`omega-rust/omega/representations/selected-instructions`, plus the
`InstructionMachineEffects`/alternative rows it joins against. Downstream
surfaces: `legalized-operations` (a legalized FMA kind so `nodes.rs` admits
the abstract node instead of falling to `_ => NodeRejection::
UnsupportedFamily` at nodes.rs:378),
t2si `selection/construction` (kind + XMM `RegisterConstraintKey`), s2s
carry, s2rh XMM allocation (no `X86Xmm` handling exists in the allocator
today — `X86Xmm` appears only in `calling-conventions` ABI plumbing),
`register-homes-to-post-allocation-machine` plan node, and the
`machine-emission` callsite for `emit_feature_required_x86_scalar_fma`
filling `function.x86_scalar_fma`/`_occurrences`/`x86_floating_control`
record fields that `image-emission` already validates.

## Claim map over the residual surfaces (2026-09-21 ~13:0xZ)

The w9-era fences on this row have all expired; current live claims:

- `omega-rust/omega/representations/selected-instructions` (whole crate) +
  `s2s/src/lib.rs` + `s2s/src/rewrites/{fixed_view,allocation_recovery,
  selected_lowering/literal_fold,literal_folds}` +
  `s2rh/src/{assignment/post_allocation_manifest,rewrites/
  rematerialization}` + `tests/architecture/optimizer_source_organization`:
  DURABLE-CODEC-RELOCATION (Devin / w10-w10-14), expires 21:02Z.
  `claims.py claim --path omega-rust/omega/representations/
  selected-instructions` returned exit 2 naming that owner — leg (b)'s
  first edit is fenced in-wave.
- `s2s/src/rewrites/confluence_run_relocation/{admission,validation}.rs`
  + `s2s/src/rewrites/window_hazards.rs`: NEW-CONFLUENCE-RUN-SPECULATABLE-
  SHARED-PREDICATE.
- `native-realization/src/{native_product,native_realization/
  behavior_exclusions,retained_native_product.rs}`: BUILD-EXCLUSION-
  REALIZATION (w10-06), exp 20:53Z — the three FMA fence files
  (object_emission, program_entry, optimization_stage) are NOT in it.
- `target-operations-to-selected-instructions`, `machine-emission`,
  `legalized-operations`, `register-model`, `physical-instructions`,
  `register-homes`, `representations/target`: no live claim.
- No live X86-FMA-PROVIDER-TRANSPORT claim; Jarod's `legalization/` fence
  (exp 09:07Z) lapsed.

Conclusion: leg (b) is blocked on `omega-rust/omega/representations/
selected-instructions`, held in-wave by DURABLE-CODEC-RELOCATION until
~21:02Z — handoff candidate when that claim drains. The remaining stages
(t2si selection, s2rh XMM allocation, machine plan, machine-emission
callsite, native-realization fence removal) are unfenced at this revision.

## Re-verification — `72fc66d6c3` (Zergling-126, linux x86-64)

State unchanged; the two fence sites drifted a few lines without content
change: `object_emission.rs` transport-stop now at `:42` (was `:34-38`),
`optimization_stage.rs` "optimized nearest-FMA custody" now at `:29` (was
`:27-30`). Still no `FusedMultiplyAdd`/`VFMADD`/`x86_scalar_fma` references
in s2s/s2rh/machine-emission `src` — the only hits remain the `isa-x86_64`
`fma.rs` encoders and the object_emission stop message. Live claims still
hold the residual surfaces (X86-FMA-PROVIDER-TRANSPORT on `legalization/`,
exp 09:07Z; UEFI-PHYSICAL-SEMANTIC-ENTRY on the `native_realization`
emission files, exp 08:44Z). No unfenced implementable slice.
