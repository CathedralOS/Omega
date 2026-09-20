# Backend vocabulary rejection audit

Catalog of the compiler-owned (backend-side) vocabularies the build specs
declare closed to authored extension, and where each authored-extension
attempt is rejected at `340e2b5ca4`. "Closed" means a package or source
file cannot spell a new member — extending the vocabulary requires compiler
support, not a declaration. Delete once a permanent spec section owns this
inventory or the closed sets stop moving.

## `std::calling` policy vocabulary

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

## Checked-assembly catalog

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

## Hardware materialization sealed sources

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

## Installation-bound reach vocabulary

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

## Cross-cutting note

Every closed vocabulary here rejects by *decoding exhaustion* (a closed
match/type, unknown member → error) rather than by an authoring check —
there is no grammar for a package to spell a new register, machine-state
flag, relocation form, or catalog entry at all. The only authored-text
surface is the assembly mnemonic string, gated by `asm_catalog_entry`.
New vocabulary members therefore require a compiler-side change in the
named crate, which is the spec's intent; the audit's residual is keeping
each closed set's decoder total as the catalogs grow.
