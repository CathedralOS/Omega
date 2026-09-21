# Backend vocabulary rejection audit

Sweep of every wire-decode point under `omega-rust/omega/backend/` at
`22e6066e9f` where a byte-level marker, kind, tag, or schema field maps onto a
closed vocabulary. Re-recorded at `2dbfecd98e4`: every audited path is
byte-identical to `bc6c5788e0` (empty `git diff` per directory — decoders,
the `admit` seam and all cited tests unchanged; the only in-range edits were
clerical `suspension_crossing: None` fixture fields). `nodes.rs` still maps
the fall-through to `UnsupportedFamily` (`nodes.rs:39,378,435`). For each surface the audit asks two questions: does every
non-admitted value reject with a named diagnostic (no `unreachable!`, no
silent acceptance), and does a test exercise that rejection?

## Method

Enumerate each decoder in scope: every `match`/`from_tag` on wire bytes, every
fixed-magic or version check, every closed-enum reconstruction in
`decode_*`/`from_bytes`/`from_decoded_*` paths. Then locate the test that
proves rejection: a byte-mutation case, a field-substitution matrix, or a
named negative test. Gaps are closed either by a new negative test (when the
rejection already exists) or by adding the rejection itself.

## Catalog

### `component-description` — `component_description.rs`

`decode_component_description` enforces a fully canonical frame: magic,
length-bounded embedded Terminal artifact, then ordered rosters. Closed
vocabularies and their rejection labels:

| Wire element | Rejection | Coverage |
| --- | --- | --- |
| `DESCRIPTION_MAGIC` | `InvalidMagic` | `component_verification/tests.rs` `wire::magic` |
| overall length > `MAX_COMPONENT_DESCRIPTION_BYTES` | `RosterBoundExceeded("description byte length")` | **added** (`oversized_descriptions_reject_before_decoding`) |
| schema `u32` | deferred by design: `verify_component` rejects via `IncompatibleSchema` | `rejects_an_incompatible_schema` |
| frontier tag `u8` (1–3) | `UnsupportedTag("frontier tag")` | `wire::frontier-tag` |
| embedded artifact | `Corrupt("embedded artifact did not decode")` | `embedded artifact` case + terminal-codec's own `every_noncurrent_format_and_vocabulary_marker_rejects` |
| roster counts (every roster) | `RosterBoundExceeded(name)` | `wire::import-roster-bound` + shared `roster_len` |
| identity strings (len + UTF-8) | `IdentityInvalid("too long" / "not utf-8")` | "too long" existing; **"not utf-8" added** (`non_utf8_identities_reject`) |
| roster order (9 sites) | `NonCanonicalOrder(name)` | existing per-roster battery |
| entry kind `u8` (1–7) | `UnsupportedTag("entry kind")` | `wire::entry-kind-tag` |
| entry evidence `u8` (0/1) | `UnsupportedTag("entry evidence")` | `wire::entry-evidence-tag` |
| authority class `u8` (1–3) | `UnsupportedTag("authority class")` | **added** |
| authority evidence `u8` (0–3) | `UnsupportedTag("authority evidence")` | **added** |
| service coordinate 0 | `Corrupt("zero service identity")` | **added** (`zero_service_identity_in_a_bound_rejects`) |
| custody kind `u8` (1–8) | `UnsupportedTag("custody kind")` | **added** |
| custody evidence `u8` (0/1) | `UnsupportedTag("custody evidence")` | **added** |
| obligation kind `u8` (1–5) | `UnsupportedTag("obligation kind")` | **added** |
| realization presence `u8` (0/1) | `UnsupportedTag("realization presence")` | **added** |
| trailing bytes / truncation | `Corrupt("trailing bytes" / "truncated …")` | `wire::trailing` / `wire::truncated` |

New coverage lives in `component_description/tests.rs`. The
`every_closed_wire_vocabulary_rejects_a_non_admitted_tag` sweep mutates each
envelope byte of a populated canonical fixture to a non-admitted tag value and
asserts the complete label set — a future tag field that forgets its
rejection fails the test by absence, not by a pinned position.

### `executable-installation` — `container.rs`, `container_bytes/decoding.rs`

`decode_executable_container` covers the full closed vocabulary: fixed magic,
reserved-zero fields, `format_marker` `u16` (v1/v2 only), canonical header
length, declared-vs-actual total length, bounded section count, canonical
directory offset, `architecture` tag (1/2), section `flags` (bit 0 only),
section `kind` `u16` with required/optional rules per kind and per format
marker, unique-kind tiling, informational-identity recomputation, per-kind
payload records (relocation `kind` 1–5, relocation `target_kind` 1/2,
placement `phase` 1–3, presence flags 0/1 for range/regime/scope), entry
alignment, and relocation/entry count bounds.

Coverage: `container_bytes/tests.rs`
`executable_container_wire_rejects_every_one_field_substitution` plus targeted
cases (`stale_container_marker_bytes_reject`,
`malformed_counts_unknown_required_and_identity_drift_reject`, truncation /
overlap / reserved-bit cases) hit every one of these labels. **No gap.**

### `native-artifact` — `callable_entry/codec.rs` + `model.rs`

`OptimizedOrdinaryCallableEntry{Record,Manifest}::decode` cover magic,
version, stage tag, hardening tag, architecture, object format, scalar/integer
type tags, calling-policy, register (tag,index) pairs, exit policy, entry
assumption, disposition, unavailable status, lengths, UTF-8, trailing bytes,
and recomputed identity.

Coverage: `compiler/tests/callable_entry_custody.rs` exercises every decode
error variant in a one-field-substitution matrix, including the retired
leaf-only exit-policy tags. **No gap.**

### `machine-emission` — `function_realization/codec/`

`FunctionRelativeOptimizationRealizationManifest::decode` covers magic,
version (12), stage, selected-lowering completion status, x86 branch
relaxation status, post-allocation optimization status and kind, action-count
overflow, architecture, object format, layout policy, scope, frame
disposition, unavailable status, trailing bytes, and recomputed identity.

Coverage: `compiler/tests/realization_custody.rs` maps every
`ManifestDecodeError` variant to a mutation case. **No gap.**

### Out of scope / no wire decode

- `native-artifact` `native_artifact.rs` and `dynamic_elf.rs`: post-decode
  validators and producers — no byte-vocabulary admission of their own.
- `component-candidate`: thin producer calling `decode_component_description`.
- `images/image-*`: encoders and in-memory model validation, not byte
  decoders; origin vocabularies are compile-time enums.
- `layout`, `register-environment`, `plans/*`, `machine-services`, ABI and
  calling-convention crates: no wire decoders found.
- ISA `selected_form_encoding` decoders, `object/object-file` codecs,
  `external-roots`, and `component_verification.rs` sit under sibling claims
  at audit time; each already routes unknown values through named diagnostics.

## Operation vocabulary — `target-operations-to-selected-instructions`

The other half of this item's territory, audited at `3a505ad6ff`: every
`AbstractOperation` family that reaches the backend is either legalized and
selected or cleanly refused — never silently miscompiled, never panicked on.
The classification seam is `legalization/scalar_graph_input/nodes.rs::admit`.

### Admission (`nodes.rs` + `control.rs`)

`admit` classifies every `OptimizationNode.operation`:

- seventeen non-scalar families (`EstablishRecord`, `PrimitiveLocalStore`, …)
  return `Ok` immediately with no scalar row;
- `BoundaryCall` admits conditionally;
- each listed scalar family admits only under its payload guards
  (`scalar_shape`, `valid_literal`, `saturating_carrier`,
  `supports_wrapping_divide_i64`, `supports_signed_wrapping_remainder`,
  `exact_cast_has_native_carriers`), with malformed payloads re-classified;
- the catch-all returns `NodeRejection::UnsupportedFamily`.

`nodes::validate` maps `Malformed` to `SourceCustodyMismatch` and
`UnsupportedFamily` to `LegalizationError::UnsupportedScalarOperation`,
carrying the machine and the operation itself. Terminators are classified by
`control.rs`: `Crash`, `StructuralCase`, `ReturnStructural`, `ReturnUnit`,
`Return`, `Jump` (only with `residual_affine_discards.is_empty()`), and
`Conditional` admit; every other operation in terminator position refuses as
custody-invalid.

Fourteen well-formed families currently refuse at this seam:
`StoreDynamicDescriptor`, `WriteOnlyIndexedPrimitiveStore`,
`MoveStructuralField`, `StoreStructuralField`, `AtomicEvent`,
`EstablishTrivialAffineLocal`, `CallUnitWithDynamicArguments`,
`CallStructuralScalarWithDynamicArguments`, `CallDynamicScalar`,
`CallStoredDynamicScalar`, `CallDynamicUnit`, `PortWrite`,
`NearestIeeeFloatFusedMultiplyAdd`, `SaturatingIntegerMultiply`. Each is a
deliberate "not yet realized" admission boundary (e.g. FMA is tracked by
FLOAT-FMA-NATIVE-TRANSPORT), not a hole.

### Selection coverage

Every `LegalizedScalarInstructionKind` variant (51 today) has a construction
arm in `selection/construction/scalar_graph.rs`: the outer `match
&operation.kind` is exhaustive with no wildcard, so an uncovered kind fails at
compile time, not at runtime. Per-target encodability is data-driven through
`SelectedConstraintKeys` `Option` fields — a key absent on a target (e.g.
`hosted_read_byte` is `Some` only on `linux_x64`) refuses via
`.ok_or_else(invalid)` as `SelectedInstructionError`. Admitted-but-unencodable
is therefore a named refusal, never a silent miscompile; its
`SourceCustodyMismatch` variant label does misname a capability gap
(observation only, not a defect). The single `unreachable!` in selection
(shift dispatch) is reachable only from the four shift-kind arm patterns and
is guarded by them.

### Pre-validation matchers

Every `match` on `node.operation` outside `nodes.rs`/`control.rs` is either a
selective probe before admission (`primitive_locals`, `byte_views`, `header`,
`aggregate_results` — each can only produce `Err`/`None` on the shapes it
examines and otherwise defers to `admit`) or a post-`validate` replay
(`source/scalar_graph::instruction`, `validate_target`), whose catch-alls
return `SourceCustodyMismatch`. None panic on vocabulary.

## Findings

1. **component-description tag coverage gap (fixed here):** six vocabulary
   tags plus the UTF-8 identity check, the zero-service coordinate, and the
   description byte-length bound had no decode-level rejection test. All
   rejections already existed; the new `component_description/tests.rs`
   sweep pins them.
2. **Adjacent observation (not fixed, not a backend surface):** a single-byte
   corruption inside the *embedded Terminal artifact* can make the
   `terminal-codec` decoder attempt a ~400 GB allocation (a roster count is
   trusted for `Vec::with_capacity` before bounds validation). The Psi-side
   codec is outside this audit's claimed paths; worth a follow-up board item
   for allocation-before-validation in terminal-codec readers.
3. **Operation-vocabulary seam verified, pin added:** the
   `UnsupportedScalarOperation` route produces a named
   `LegalizationError` carrying machine and operation, which propagates
   through `OptimizedSelectionPipelineError::Legalization` and
   `OptimizedVerifiedPhysicalPipelineError::Selection` to
   `selected_physical_pipeline_failed` — an ordinary `Vec<Diagnostic>`, no
   abort. `admission_vocabulary_tests.rs` pins the `UnsupportedFamily`
   classification for refused families and the named-diagnostic mapping.
   (The pre-existing `PortWrite` leg in `tests/legalization/scalar_call_unit.rs`
   asserts only `.is_err()` on an abstract-side plan mutation that refuses at
   custody derivation — it does not exercise the named route.)

## Evidence

Recorded at `22e6066e9f`; re-witnessed at `bc6c5788e0`; re-recorded against
`2dbfecd98e4` (linux x86-64). HEAD does not compile — `terminal-verifier`'s
`StructuralTypeShape` matches went non-exhaustive when `04f2fdbb853ea` added
`ElementView` — so the suites were run at `40a3556906dc1` (the breaking
commit's parent), where every audited path is byte-identical to
`2dbfecd98e4`:

- `git diff bc6c5788e0..2dbfecd98e4 -- <every audited directory>` — empty of
  semantic edits; only `suspension_crossing: None` fixture fields and an
  import-list style change, all from `09b96a4a1f05a`.
- `cargo nextest run -p component-description --lib` — 22/24 pass at
  `40a3556906dc1`. All vocabulary-rejection pins stand
  (`every_closed_wire_vocabulary_rejects_a_non_admitted_tag`,
  `non_utf8_identities_reject`, `zero_service_identity_in_a_bound_rejects`,
  `oversized_descriptions_reject_before_decoding`, `wire::*` battery). Two
  `component_verification::tests` fixtures fail with
  `InvalidSuspensionCallPlan { reason: UnmarkedCallSide }` — an unrelated
  pre-existing regression introduced when `09b96a4a1f05a`'s
  suspension-crossing demand began validating the fixtures' unmarked
  `BoundaryCall` operations; the audit surface itself is untouched.
- `cargo nextest run -p target-operations-to-selected-instructions -E
  'test(~admission_vocabulary)'` — 2/2 pass at `40a3556906dc1`.
- Earlier runs: 24/24 + clippy/fmt clean at `bc6c5788e0`.

## Build-spec closed vocabularies

Catalog of the compiler-owned (backend-side) vocabularies the build specs
declare closed to authored extension, and where each authored-extension
attempt is rejected at `340e2b5ca4`. "Closed" means a package or source
file cannot spell a new member — extending the vocabulary requires compiler
support, not a declaration. Delete once a permanent spec section owns this
inventory or the closed sets stop moving.

### `std::calling` policy vocabulary

Source: [calling_plans.md](../spec/build/calling_plans.md) — "policies choose
from closed primitives; they cannot emit instructions, supply relocation
bytes, inspect private carriers, or bypass validation. Extending the
primitive register, placement, control, or machine-state vocabulary requires
compiler support." Its quantities use `u64`, not `addr`; narrowing to a
compiler field requires a checked range conversion.

Enforcement: `provider-planning/src/calling_policy_plans/`. Authored policy
values are build-time-evaluated into `BuildTimeValue` and decoded through
`build_time_decoding.rs`, which matches every closed type's variant list
explicitly — `ValueShape`, `ValuePlacement`, `ValueLocation`, `IndirectKind`,
`MachineRegister` (the full per-ISA register list), `RegisterSet`,
`MachineRegime` (`X86Long64`, `Aarch64A64`), `MachineStateSet`,
`EntryStack`, `Preemption` — and every non-member spelling returns
"`<X>` case `<name>` is outside the compiler-owned vocabulary" (or
struct/case/int/bool/text kind errors at lines ~490–538). A policy cannot
supply relocation bytes or instructions because no primitive decodes to
either.

- Relationship shape: `plan_computation.rs` rejects a boundary trait with
  zero/multiple `Calling<C>` relationships ("exactly one concrete policy"),
  a relationship carrying ≠1 policy argument, an unevaluable policy
  argument, a selected policy type with no `::plan` machine, and a plan
  whose parameter/result count or authored ABI shape disagrees with the
  boundary signature (`validate_materialized_boundary_plan_result` →
  `invalid_authored_plan`).
- Callback destinations: `boundary_signatures.rs` rejects a native callback
  parameter naming an unknown machine binder; `callback_bindings.rs`
  rejects unclosed target contexts retaining materialization rows,
  multi-row destinations ("exactly one is required"), over-capacity
  registrar demands, and invalid target-closed outbound materialization.
- Evidence: `compiler/tests/calling_policy_plans/policy_evaluation.rs`
  exercises accepted and rejected authored policies end-to-end.

Residual: none located for vocabulary closure. The `u64`-only carrier rule
is enforced structurally — `addr` has no `BuildTimeValue` decoding path —
rather than by a named diagnostic.

### Checked-assembly catalog

Source: [assembly.md](../spec/language/assembly.md) +
hardware_materialization.md §Checked instructions — entry/exit operations
(`iretq`, `sysret`, `eret`) are deriver-only; provider-only checked `lidt`
requires consumer CPU/table publication authority; "the compiler owns that
instruction contract".

Enforcement: `language-core/src/inline_assembly/mod.rs` `asm_catalog_entry`
is the closed catalog — an unrecognized mnemonic returns `None` and the
discharge site (`validation/src/machine_calls/effects/asm_discharge.rs`)
rejects. Recognized but unauthorable spellings carry refusal reasons:
`Refused(HiddenControlExit)` for `ret`/`call`/`br`/`blr`-family control
edges (checked `jmp state(...)` is the admitted route), `DeriverOnly`
availability for `iretq`/`sysret`/`sysretq`/`eret` (only derived entry/exit
machinery discharges their state-plan contracts), and `lidt`'s contract
carries `IdtControlAuthority`. Instructions recognized without a complete
source contract are `Refused`, not silently absent.

### Hardware materialization sealed sources

Source: [hardware_materialization.md](../spec/build/hardware_materialization.md)
— "a closed symbolic vocabulary distinguishes sealed data symbols and
entry-stub identities"; "no numeric entry address or arbitrary-offset
writer is exposed"; a field consumed by the loader must fit the format's
native relocation vocabulary.

Enforcement: the closed vocabulary is the type itself —
`RelocationTarget::Entry(EntryStubId) | Data(DataSymbolId)`
(`program-entry-plan/src/post_handoff_writer/`) — so no spelling outside
the two sealed kinds exists. Writer generation consumes a normalized plan
and a resolver restricted to the admitted artifact
(`lower_post_handoff_writer_fragment`), and
`validate_lowered_post_handoff_writer` replays the fragment; compact
fingerprints are report coordinates, not authority. The loader-consumed
leg is enforced by construction: image-elf/-macho/-pe emitters write only
their format's legal relocation forms; a source has no syntax to name one.

Residual: none located at the vocabulary boundary. (Writer/IDT consumer
policy correctness is a separate validation surface, e.g.
`layout_plans/interrupt_descriptor_tables.rs`.)

### Installation-bound reach vocabulary

Source: [external_roots.md](../spec/build/external_roots.md) §Installation-bound
reach — `reaches <= Bound` is one bounded abstract row; ordinary callable
contracts cannot carry an unresolved row; installation rejects any remaining
unresolved row after bound substitution.

Enforcement: the bound row is normalized at manifest retention and
substituted through the complete root closure at install
(`external-roots` program-local root installation ledger reject sites);
unresolved-row rejection is the `install` path's obligation — see
installation_ledger.rs rejects. Residual: full bound-substitution coverage
across multi-operation protocols is folded into the external-roots board
items.

### Cross-cutting note

Every closed vocabulary here rejects by *decoding exhaustion* (a closed
match/type, unknown member → error) rather than by an authoring check —
there is no grammar for a package to spell a new register, machine-state
flag, relocation form, or catalog entry at all. The only authored-text
surface is the assembly mnemonic string, gated by `asm_catalog_entry`.
New vocabulary members therefore require a compiler-side change in the
named crate, which is the spec's intent; the audit's residual is keeping
each closed set's decoder total as the catalogs grow.
