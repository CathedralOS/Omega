# Optimizer Physical Pipeline

This brief owns the lowering-to-publication path. The architecture entrance is
[optimizer_architecture.md](../optimizer_architecture.md).

## Selected lowering

Abstract-to-target lowering now enters through a 68-line settlement and
installation-evidence coordinator. It descends first by function result
family, then through explicit scalar setup, special-form, conditional, and
straight-line routes or structural direct-call and return routes. Unit,
boundary-settlement, cleanup, and structural-layout mechanics remain named
sibling responsibilities rather than hidden branches in one lowering file.

Target legalization and instruction selection produce explicit selected forms
over virtual registers. Fixed operands are constraints; they do not preassign
the entire program. Selected rules may fold exact incoming immediates or choose
equivalent target forms, but must preserve operation, edge, trap, provenance,
and fuel identities.

The mandatory lowering crate has two explicit entrances. `legalization/mod.rs`
joins canonical source projection to independent whole-plan replay;
`selection/mod.rs` joins selected-plan construction to independent validation.
Structural, scalar-function, leaf-expression, constraint, identity, and
fixture mechanics descend below those joins. The crate-level `lib.rs` is only
the 21-line responsibility map between the two stages, not a hidden third
coordinator.

Immediately below the legalization entrance, `catalog.rs` is the sole ordered
inventory for all twelve current forms: seven scalar, one plain Unit, and four
structural Unit. Each row names its typed recipe, producer matcher kind, exact
source-shape constraints, non-authoritative structural cost, and independent
validator kind. `source/matchers/` walks that catalog to recognize a form;
`replay/validators/` reconstructs membership without calling producer code.
Removing a row disables the form, and missing or ambiguous recipe lookup fails
closed. The Unit recipe families are retained in the V9 legalized-plan
identity. Structural selected-form validation separately reconstructs ABI
layout and call constraints without importing selection construction helpers.

## Register allocation

Allocation computes selected-CFG liveness, live-range fragments,
interference, allowed views, ABI constraints, clobbers, and spill legality.
Home assignment, copy insertion, spilling, coalescing, and bounded
rematerialization are separately validated decisions.

Register units model aliasing between views. Flags/predicates, vector lanes,
special registers, ABI reservations, call clobbers, and stack/frame constraints
are explicit target facts. Modulo scratch-register assignment is not an
allocator.

The temporary empty-selection compatibility assignment is navigable without
pretending to be that allocator. Its plan coordinator validates the entry
roster, its function coordinator retains provenance, and one exhaustive
carrier router descends into cleanup, boundary, Unit, structural, scalar,
placement, control, and expression families. This preserves the explicit
replacement boundary while the selected physical conveyor gains full
operation coverage.

## Post-allocation machine stage

This stage consumes physical symbolic instructions, selected liveness, homes,
and the physical register model. Its catalog currently contains exact families
such as:

- AArch64 compare-zero plus branch-nonzero to `CBNZ`;
- AArch64 shortest MOVN-seeded i64 materialization; and
- x86-64 zero i64 materialization via `XOR r64, r64` when every canonical
  RFLAGS unit is dead-out.

All produce variants of one validated post-allocation stage result. The result
contains the original source identity, exact rule identity, validated symbolic
plan, accounting, and custody receipt. Encoding dispatches on that typed plan;
the complete compiler route does not grow a new parallel carrier family for
each rule.

Direct homes and homes after selected lowering enter one
`StagedPostAllocationMachineFunctionRelativeRealization`. CBNZ, MOVN, and
XOR-zero therefore share the same encoding, layout, exit, realization, and
fragment source route. The former named CBNZ/MOVN complete-route carriers have
been removed; rule-specific values remain typed leaves borrowed from the shared
result.

Selected-lowering realization likewise enters fragment admission through one
`SelectedLowering` carrier whether or not a function-relative layout rule also
ran. Add/subtract folds do not acquire fragment route variants, and rel8 remains
an optional typed leaf of the selected-lowering realization rather than an
admission prerequisite. The fragment manifest records the generic phase source
kind while its selection and realization identities retain the exact rules.

The incoming-u12 add/subtract producer emits an immutable fold plan. Its
validator separately reconstructs source eligibility, register constraints,
the exact action roster, rewritten instructions, provenance and fuel custody,
dense identifiers, work usage, and transformed-plan identity. Validation does
not call the producer's transformation helpers; an architecture dependency
guard enforces that separation.

The adjacent machine catalog is also the architecture-admission point. CBNZ
and MOVN require AArch64; XOR-zero requires x86-64. Function-relative rel8
relaxation declares x86-64 in its adjacent layout catalog. Unsupported target
selection is rejected with the exact optimization, required architecture, and
actual architecture before rule dispatch; custody errors preserve this reason
instead of converting it to a generic phase-composition or root mismatch.
Linux, Windows, and UEFI x64 share x86-64 applicability, while Linux and macOS
Arm64 share AArch64 applicability. UEFI applicability does not grant its still
unimplemented publication authority.

## Encoding and layout

ISA crates own canonical form encoding/decoding and reconstructed effects. A
machine rule never hand-assembles bytes. Layout-independent encoding retains
row identity, decoded footprint, effects, provenance, and optimization
disposition.

The encoding entrance joins construction to a separate `validation/` rung.
That rung checks roots and normalized optimization custody, then descends
independently through ordinary rows, structural rows, and aggregate
counts/identity. Row validation consumes candidate bytes only through the
target-owned baseline, MOVN, XOR-zero, and structural-call decoders; an
architecture guard forbids imports of producer row/structural encoders. CBNZ
dispositions are reconstructed from the typed optimization plan while its
unresolved branch remains explicit deferred control.

Function-relative layout resolves labels, branch extents, and exact byte
offsets. Its independent admission rung re-derives layout policy, canonical
block order, function/block spans, row offsets, structural call/return spans,
and aggregate identity from admitted pre-layout dispositions. Candidate x86-64
and AArch64 conditional branches are decoded with target-owned validators;
x86 displacement is relative to the branch end, while AArch64 displacement is
relative to the instruction address. Candidate evidence, fused CBNZ register
reads/effects, and structural unresolved-fixup custody must match that replay.
Layout rules such as x86 rel32-to-rel8 relaxation consume a complete baseline
layout and return a validated replacement. Baseline and selected byte counts
remain replayable.

## Exit and publication custody

The pipeline retains one chain:

```text
validated stage result
  -> selected-form encoding receipt
  -> resolved-layout receipt
  -> whole-function exit contract
  -> function-relative realization manifest
  -> fragment/text/object manifests
  -> optimized artifact manifest
  -> ordinary callable-entry manifest
```

Each boundary recomputes child identities and rejects detached, reordered,
truncated, trailing, or cross-source data. Generic artifact layers bind child
identities and do not need a new schema merely because a new exact rule exists.
The genericization changed the data carried at three serialized boundaries:
function-relative realization is v9, while fragment emission and fragment text
placement are v8. Their records retain the exact selected-lowering selection
or post-allocation optimization, not a broad optimization level.

The Terminal-Psi-to-native stage now exposes its full physical composition as
small owning entrances. Source-entry settlement replays declaration and
calling-plan custody. Native realization then chooses the ordinary or exact
selected input, admits provider executions/installations, emits machine code,
and replays object/image assembly. The optimized ProgramStorage encoding and
wrapper-object boundaries remain separate joins: encoding projects and replays
the target template; object construction joins settlement, semantic contract,
composite object, manifest, and custody. Provider projection, machine routes,
artifact assembly, semantic replay, object validation, models, codecs, and
diagnostics descend into named leaves.

For UEFI, the physical adapter contract is settled but not yet implemented in
this chain. A generated ABI shell invokes one checked bootstrap adapter;
physical-arrival and firmware-service postconditions supply its opaque
premises. The adapter establishes Loaded Image correspondence and independent
initial storage, proves resource composition, crosses
`ProgramStorageEntry::enter`, calls the semantic continuation, reclaims
returning-profile resources, and maps normal Unit return to success. Crash,
trap, and abort remain non-returning. Optimization selection still grants no
firmware image or publication authority until that adapter, native-image
validation, and selected-build publication join the same custody chain.

## Composition policy

During bring-up, phase compositions are deliberately narrow. A route accepts
only exact implemented sets and rejects the rest with a closed error. The
long-term coordinator executes each stage catalog over one typed stage carrier;
it must not encode an optimization name in top-level route variants.

## Required tests

- disabled selection preserves baseline bytes and identities;
- wrong target, form, allocation, or live-out facts reject;
- producer and independent validator disagree on corrupted plans;
- canonical encoders reject alternate or trailing byte forms;
- exact byte deltas and offsets replay after layout;
- every custody boundary rejects one-field identity corruption; and
- direct, selected-lowering-composed, and final artifact paths retain the same
  full selection identity.

The catalog matrix covers all 14 current exact names across all five native
target constructors: 60 admitted cells and 10 typed architecture rejections.
Target-independent Psi, selected-lowering, and allocation-recovery rules are
explicit declarations, not untested fallthrough behavior.

Current XOR-zero coverage proves both direct and selected-lowering routes
through fragment, object, and callable publication. Target-register-environment
coverage selects and corrupts the exact scalar-call ABI row for System V AMD64,
Microsoft x64, AAPCS64, and Darwin AAPCS64 across Linux x64, Windows x64, UEFI
x64, Linux Arm64, and macOS Arm64. It checks argument/result views, implicit
control and stack facts, every individual call clobber, preserved-unit
injection, platform ABI substitution, preservation-convention drift, and the
Microsoft structural-Unit call row. This does not claim general call-crossing
allocation coverage: general scalar calls are not yet represented in the
selected CFG.

The ordinary empty-selection compiler route is deliberately outside this
optimizer custody chain. Its four-target byte and artifact-metadata baseline is
locked by the no-selection golden compiler test; only an explicit nonempty
selection constructs the optimizer-side prephysical carrier.

Allocation recovery has one final function-relative carrier. Its closed source
taxonomy has `FixedViewCopies` and `ActiveResidentRematerialization` leaves;
each leaf retains its exact upstream receipt and post-allocation transformation
identity, while the carrier owns the common machine plan, selected-form
encoding, resolved layout, whole-function exit contract, and realization
manifest. The physical pipeline therefore returns one `AllocationRecovery`
variant, and fragment admission consumes one `AllocationRecoveryV1` source
kind. Fixed-view copies and active-resident rematerialization do not own
parallel publication routes. Fragment and fragment-text manifests use schema
v9 because the source-kind tag now denotes this generic carrier.
