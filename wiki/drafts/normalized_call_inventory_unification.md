# Normalized call-inventory unification design

NEW-NAL-CALL-INVENTORY-UNIFICATION-DESIGN. Measured at `d7d4a88331b` on
linux x86-64 (z157). This draft designs the call-inventory unification
flagged in **NORMALIZED-ABI-LOWERING** (TASKS.md ~:3126): "dynamic
arguments/dispatch still split the target call inventory by signature
shape. Extend the ordinary `Call` and its explicit result custody with
argument classes and callee sources realized by the target's calling
policy as native descriptor support lands. Preserve the foreign
formal-order mapping and independent checks; do not introduce another
call family for each newly supported signature combination."

## Problem

`target_operations/operations/unit.rs::TargetUnitOperation` today carries
eight call-family variants whose differences are signature shape, not
semantics:

| Variant | Callee source | Argument classes | Result custody |
|---|---|---|---|
| `Call` | direct `MachineId` + `NativeCallOrigin` | `TargetUnitScalarCallArgument` prefix + `Vec<TargetStructuralArgument>` | `TargetCallResult` (Unit/Scalar/Structural + reference results + returned claim transfers) |
| `StructuralScalarCallWithDynamicArguments` | direct `MachineId` | structural + `TargetDynamicDescriptorArgument` (each expands to `{data, table}` pair) | `AbstractResult` + `TargetUnitScalarHomeRequirement` |
| `StructuralUnitCallWithDynamicArguments` | direct `MachineId` | structural + dynamic descriptor | none |
| `StoredDynamicScalarCall` | `AbstractStoredDynamicDispatch` (stored descriptor reload) | one `TargetStructuralArgument` (the descriptor projection) | `AbstractResult` + scalar home |
| `DynamicScalarCall` | `AbstractReboundDynamicDispatch` | `initial_argument` + `rebound_argument` structural pair | `AbstractResult` + scalar home |
| `DynamicUnitCall` | `AbstractReboundDynamicDispatch` | same pair | none |
| `DynamicParameterScalarCall` | `AbstractParameterDynamicDispatch` + `parameter_abi` + `requirement` + `table_slot_byte_offset` + `dispatch_call_plan` | none beyond the descriptor parameter | `AbstractResult` + scalar home |
| `DynamicParameterUnitCall` | same | same | none |

plus two adjacent-but-separate leaves: `NormalizedForeignCall` (evaluated
import leaf: `boundary` + `provider_execution` + `binding`, foreign
scalar/structural argument rows, `Option<TargetUnitScalarHomeRequirement>`)
and `BoundarySettlement` (boundary leaf with its own scalar/structural/
byte-sequence argument classes and completion evidence).

Consequences of the split:

- **Every new signature shape mints another variant.** Scalar/Unit splits
  (`*ScalarCall*` vs `*UnitCall`) exist only because result custody is
  modeled per-variant instead of by the `TargetCallResult` enum `Call`
  already owns.
- **The Terminal declaration erases authored interleave.** The
  `NormalizedForeignCall` doc comment records that lowering admits
  structural arguments only while the scalar lane is empty because "a
  mixed signature cannot rejoin its exact plan positions without a wider
  coordinate." The same is true of every `*WithDynamicArguments` family:
  scalar prefix + structural tail + dynamic descriptor tail loses the
  authored order when classes interleave.
- **Each consumer must case-split N ways.** Legalization
  (`t2s/legalization/scalar_graph_input/target/unit.rs` +
  `indirect_calls.rs`) admits `Call`, `NormalizedForeignCall`, and the
  parameter-descriptor dynamic pair, but `StoreDynamicDescriptor`, the
  rebound and stored variants, and both `*WithDynamicArguments`
  variants have no legalization mirror — every family without one is
  dead code downstream until its mirror is ported.

## Producers and consumers (census)

- **Producer:** `a2t/lowering/unit/{dynamic,parameter_dynamic}.rs`,
  `lowering/unit/structural_scalar/dynamic_arguments.rs`,
  `lowering/control_flow/operations.rs` (the `Call` route).
  `terminal-psi-to-abstract-operations` routes the machine/operation
  classification upstream (`lowering/machine/operation/routing.rs`).
- **Validation:** `a2t/validation/structural_call_arguments.rs` +
  `structural_argument_sources.rs` replay each family's arguments against
  the declaration and calling policy.
- **Consumers:** `t2s/legalization/scalar_graph_input/target/unit.rs`
  (+ `indirect_calls.rs`) — admits `Call`, `NormalizedForeignCall`
  (structural args in the borrowed single-pointer mirror), and
  `DynamicParameter{Scalar,Unit}Call`; the stored/rebound dynamic calls
  and `*WithDynamicArguments` families stop here. Selection
  (`selection/scalar_call_abi/*`), construction/validation emitters, the
  ISA `selected_form_encoding` rows and `machine_effects` call models,
  and `image-emission` (`normalized_foreign_calls`,
  `dynamic_conformance_codec`, `function_fragments`) re-derive each
  family's native transport independently.
- **Sibling legs** (not this design's to pre-empt): the dynamic variants'
  missing legalization operand is owned by
  **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY**; callback transport by
  **CALLBACK-PRIVATE-MATERIALIZATION**; the NF structural-mirror legs sit
  under NORMALIZED-ABI-LOWERING.

## Design

One call operation with three orthogonal axes, replacing all eight
variants:

```rust
Call {
    // who is being invoked and how the target address is obtained
    callee: TargetCallCallee,
    // the complete ABI plan for the signature (unchanged carrier)
    call_plan: CallPlan,
    // argument stream in authored formal order (new; see below)
    arguments: Vec<TargetCallArgument>,
    // explicit result custody (unchanged: Unit | Scalar | Structural)
    result: TargetCallResult,
    // shared edges (unchanged)
    claim_transfers: Vec<ClaimTransfer>,
    requirement_obligations: Vec<ObligationId>,
    crash_continuations: Vec<CrashRouteBucket>,
    psi_operation: OperationId,
}

enum TargetCallCallee {
    /// Direct call on a declared machine; NativeCallOrigin distinguishes
    /// authored vs installed-provider completion evidence.
    Direct { origin: NativeCallOrigin, callee: MachineId },
    /// Indirect through a descriptor materialized by StoreDynamicDescriptor.
    StoredDescriptor { dynamic_dispatch: AbstractStoredDynamicDispatch },
    /// Rebound dispatch: initial + rebound argument pair retained.
    Rebound { dynamic_dispatch: AbstractReboundDynamicDispatch },
    /// Indirect through a slot of this function's borrowed descriptor
    /// parameter: requirement row + erased adapter plan + table offset.
    ParameterRequirement {
        dynamic_dispatch: AbstractParameterDynamicDispatch,
        parameter_abi: TargetDynamicDescriptorParameterAbi,
        requirement: TerminalDynamicRequirement,
        dispatch_call_plan: CallPlan,
        table_slot_byte_offset: u32,
    },
    /// Evaluated import leaf: provider execution + normalized binding.
    Foreign {
        boundary: BoundaryMachineId,
        provider_execution: ProviderExecutionBinding,
        binding: NormalizedForeignCallBinding,
    },
}

enum TargetCallArgument {
    Scalar(TargetUnitScalarCallArgument),
    Structural(TargetStructuralArgument),
    /// One authored existential descriptor formal — expands to the
    /// `{data, table}` ABI pair, so one entry carries two placements.
    DynamicDescriptor(TargetDynamicDescriptorArgument),
    /// Normalized-foreign rows keep their evaluated-plan argument types.
    ForeignScalar(NormalizedForeignScalarArgument),
    ForeignStructural(NormalizedForeignStructuralArgument),
}
```

The single `Vec<TargetCallArgument>` in authored formal order is the
"wider coordinate" the NF variant's doc comment asks for: scalar and
structural lanes no longer have to stay empty for the other to be
admitted, and the foreign formal-order mapping is preserved by
construction rather than reconstructed by convention.

`result: TargetCallResult` already carries Unit/Scalar(home)/
Structural(result, callee_result, result_home, reference_results,
returned_claim_transfers) — every `result: AbstractResult` +
`result_home` pair in the dynamic variants folds into `Scalar(home)`
with the semantic result identified through the home's
`defining_operation`/`source_value`, and `Unit` absorbs both `*UnitCall`
variants.

`StoreDynamicDescriptor`, `PortWrite`, `BoundarySettlement`, `Continue`,
`Return` are not call families: `StoreDynamicDescriptor` is a store
(materialization, not an invocation), and the boundary leaves own
settlement/completion evidence the call inventory does not model. They
stay.

## Invariants

1. **Formal order is data, not convention.** `arguments[i]` names the
   authored parameter index (scalar args already carry
   `parameter_index`); receiving checks reconstruct the complete plan and
   custody from the declaration and pre-call state exactly as they do
   today — the unified row must expose the same inputs to that replay.
2. **No family per signature.** A call with any mix of scalar,
   structural, and dynamic-descriptor arguments is the same variant; new
   argument *classes* extend the enum, not the operation set. This is
   the flag's hard rule.
3. **Descriptor custody stays explicit.** `StoredDescriptor`,
   `Rebound`, and `ParameterRequirement` callees keep their
   dispatch/ABI/table-offset rows verbatim — unification names the
   custody, it does not re-derive it.
4. **Foreign calls keep evaluated evidence.** `Foreign` retains
   `boundary`, `provider_execution`, and `binding` — locator and
   calling-plan strings are never re-accepted at lowering, matching the
   existing leaf's contract.
5. **Result custody is one enum.** `Scalar(home)` vs `Unit` vs
   `Structural` replaces the scalar/unit variant duplication; a
   structural-result call keeps `result_home: Option<...>` inside the
   same shape.

## Migration order

1. **Introduce `TargetCallCallee`/`TargetCallArgument` + the unified
   `Call` fields behind a conversion constructor.** Mechanical rewrite
   of the eight variants' constructors/destructors; every current reader
   goes through `Call` accessors. This is the only step that touches
   `target-operations`.
2. **Producers emit the unified row.** `a2t/lowering/unit/*` and
   `lowering/control_flow/operations.rs` switch construction only —
   argument admission rules unchanged.
3. **Validation replays the unified row.** `structural_call_arguments.rs`
   / `structural_argument_sources.rs` gain one entry point per callee
   source; the existing per-family replays compose onto shared argument
   iteration.
4. **Legalization and selection consume `Call`.** The NF structural
   mirror legs and RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY then
   target one operation instead of N variants — the unconsumed
   dynamic families stop needing a new legalization mirror per family.
5. **Delete the retired variants.** `TargetUnitOperation` loses the
   eight rows once no consumer reads them; backend selected-form and
   image-emission rows keep their per-transport families (those are
   encodings, not call families).

Step 1 is independently landable behind accessors; steps 4-5 sequence
with the named sibling legs rather than blocking them.

## Non-goals

- No semantic change to any call class: admission, custody, plan
  reconstruction, and rejection behavior are preserved bit-for-bit at
  each stage.
- No new argument class beyond what today's variants carry; foreign
  scalar/structural rows keep their evaluated types rather than
  normalizing into the domestic ones.
- `StoreDynamicDescriptor`, `BoundarySettlement`, `PortWrite`, and the
  non-call terminators are out of scope.
- Callback transport (CALLBACK-PRIVATE-MATERIALIZATION) and the dynamic
  legalization operand (RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY)
  keep their own rows; this design only removes the need for future
  per-signature variants when they land.
