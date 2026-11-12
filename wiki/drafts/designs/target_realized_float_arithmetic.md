# Target-realized float arithmetic

Ratified 2026-11-09 as documentation. Canonical `f32` and `f64` operations are
realized by the selected target's declared realization table, not by Build
provider selection. The rule lives in
[numeric values](../../spec/language/numeric_values.md#target-realization-and-control-state).
No compiler support or fixture outcome is claimed here. Delete this record when
`TARGET-REALIZED-FLOAT-ARITHMETIC` on [TASKS.md](../../../TASKS.md) lands and
the spec section suffices.

## Context

Core declares each canonical float operation as a standalone boundary operator
with a contract pinned to `FloatSemantics`. `NativeFloats satisfies
FloatNativeOperations` is a sealed, bodyless map whose members carry
`satisfies` edges to those operators; the compiler catalog supplies intrinsic
execution only through that map. Core's build program then selected
`NativeFloats` for each of 104 operators on each target: 98 in a helper run for
five targets, six directed-rounding fused multiply-adds in a helper run for the
two ARM targets. Every compile checked and ran those 104 selections first.
Replacing the file with a single selection cut whole-process compile time by
about 9% on the recorded host.

Owner question `bulk-provider-selection` asked how one Build call could
cover many slots. Every option batched the bootstrap: a target-set form, a
provider-derived selection, a 98-element list, a split implementation trait, a
named slot group. The question behind them was why the bootstrap existed.

## Decision

1. `f32` means IEEE binary32 and `f64` means IEEE binary64. No realization
   changes a contract; posits or any other arithmetic need their own types.
2. Canonical float operations are compiler-owned like integer primitives and
   `Float::meaning32`. For each one a target package declares a realization: a
   catalog intrinsic, a checked Omega machine satisfying the contract, or
   unsupported. Mainstream targets ship their tables with core.
3. The realization plan retains the chosen entry with its ISA-feature admission
   and floating-control obligations. Reports name the realization used.
4. A program using an unsupported operation rejects, naming the operation and
   the target constraint. Nothing is silently substituted.
5. Build programs neither select nor override canonical float realizations.
   Provider selection remains for boundary services such as time, clocks,
   filesystems and devices.
6. A program that needs a particular algorithm calls a named machine. Checked
   software routines remain usable as target realizations, including for
   targets without floating-point state.
7. `bulk-provider-selection` closes without a bulk-selection form. No target-set
   form, no provider-derived selection, no slot list, no named slot group. A later need
   to group genuinely replaceable services brings its own customer.

## Given up

A build program can no longer substitute its own implementation of one
canonical float operation. This is deliberate: no customer exists, and adding
interposition later is a smaller change than removing it after libraries
depend on it.

## Migration

- Declare per-target realization entries for the 104 canonical operations and
  bind the public operators to them; move the intrinsic binding off the
  `NativeFloats` sealed map or fold that map into the catalog.
- Delete `configure_float_defaults` and `configure_directed_fma_defaults` from
  core's build program and the `NativeFloats` selection route.
- Keep the operation-to-realization records, differential-evidence identities,
  and the checked-software realization route.
- Baseline, remove the bootstrap, then measure the equivalent workload. Profile
  any remaining `select_provider` cost inside this item; it is implementation.

## Acceptance

- Float, FMA, directed-rounding, special-value and conversion fixtures keep
  their documented behavior under `omega run --both`.
- Fused and unfused arithmetic and each rounding mode stay distinct operations.
- Reports identify each realization with its ISA-feature and floating-control
  admissions.
- An operation with no realization on a target rejects, naming both.
- Checked software realizations contribute their real calling, storage and
  machine-state costs.
- Ordinary float-using programs make no float provider selections, and core's
  build program contains none.
