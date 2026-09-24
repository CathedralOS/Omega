# Carrying an erased-argument lane through `&dyn` descriptors

Status: exploratory draft, not an accepted design or implementation task.
The current check-time refusal stays in force until every leg below lands
together. These notes describe the smallest sound channel for a proof-actual
lane on dynamic scalar calls; replace or delete them when the design question
is settled elsewhere.

Re-audited at `2dbccb9bd6` (NEW-PRM-DYNAMIC-ERASED-LANE-DESIGN): every
anchor below still holds — static `Call`/`CallUnit`/`CallStructural*` carry
`erased_arguments: Vec<ScalarTerm>` (`terminal-psi`
`terminal_module/control_flow/operations.rs`), emission still enforces the
roster ("erased lane disagrees with its target roster",
`checked-trees-to-lowered-psi` internal-calls emission),
`checks/contracts/dynamic_erased_lane.rs` still refuses by name with
`fail/relevance/dynamic_erased_formal_lane` pinning the refusal, and the
served board clause (TASKS.md "Remaining work" — the `&dyn` erased-lane
refusal) is unchanged. The three-leg channel stands as designed; the item
resolves only when the canary flips, per the evidence clause.

Affected subjects: [relevance and erased parameters](../../spec/terminal-psi/structural_access.md),
[dynamic dispatch custody](../../spec/terminal-psi/dynamic_dispatch.md).

## Current state

A requirement may declare proof-only `[erased]` formals. Every static call
operation already carries them:

- `Call`, `CallUnit`, `CallStructuralScalar`, and
  `CallStructuralWithScalarArguments` each retain
  `erased_arguments: Vec<ScalarTerm>` — proof-only actuals in the callee's
  `contract.erased_scalar_formals` roster order, evaluated against the
  caller's namespace at verification only, with no runtime operand
  (`terminal_module/control_flow/operations.rs`). The same lane exists on
  call terminators.
- Emission enforces roster arity
  (`attached_unit/ordinary_calls.rs::prepare`, which every Unit call emits
  through: "Unit call erased formal roster drifted from its checked target")
  and requires each actual to lower as a pure checked expression.

The dynamic lane deliberately carries none of this. `CallDynamicScalar`,
`CallDynamicParameterScalar`, `CallDynamicUnit`, and
`CallDynamicParameterUnit` keep only `descriptor_ordinal` /
`parameter_ordinal` + `requirement_slot` — "no static callee or raw source
argument" — so a requirement declaring an `[erased]` formal can never receive
its proof-only actuals through a descriptor. The checker refuses such calls
by name (`checks/contracts/dynamic_erased_lane.rs`, pinned by
`fail/relevance/dynamic_erased_formal_lane`): the descriptor keeps `self`
arity only, and carrying the lane needs a proof-actual channel on
`CheckedDynamicScalarCallPlan` plus emitted vtable rows — a larger slice
than the refusal.

## Why the naive extension is insufficient

On a static call the callee's `erased_scalar_formals` roster is known at
emission. On a dynamic call the callee is selected by a descriptor row at
each dispatch; the emitted channel must stay sound across every candidate
realization of the requirement, including descriptor rebound, stored, and
parameter-transferred versions. Adding `erased_arguments` to the call
operations without pinning the roster at descriptor establishment would make
roster agreement a per-dispatch assumption instead of reconstructed evidence.

## Proposed channel

Three legs, landed together:

1. **Checked representation.** `CheckedDynamicScalarCallPlan` (and the
   stored/parameter analogs) gains
   `erased_arguments: Vec<CheckedCallScalarArgument>` in authored order.
   `check_dynamic_erased_formal_lane` flips from blanket refusal to
   validation: the lane is allowed only when the requirement's erased roster
   is non-empty and authored actuals are present, pure, and arity-matched;
   arity mismatches and impure actuals keep refusing by name. The stored-
   descriptor wrapper keeps the actuals as caller custody alongside the
   aggregate lineage it already retains.

2. **Terminal emission and vtable rows.** Each `CallDynamic*` operation
   gains `erased_arguments: Vec<ScalarTerm>` — the same proof-only,
   caller-namespace lane static calls emit. "Emitted vtable rows" means the
   descriptor interface publishes the roster, not just the dispatch: each
   `TerminalDynamicRequirement` slot declares the requirement's erased-
   formal arity (or its roster identity), so descriptor establishment —
   `&x as &dyn` materialization, rebound, and stored-descriptor joins —
   rejects a conformance whose realization callable does not publish a
   matching `erased_scalar_formals` roster. Roster divergence is caught at
   materialization, where the descriptor's closed application is already
   reconstructed, rather than deferred to each dispatch row.

3. **Verifier reconstruction.** For direct, indirect, stored, and parameter
   dispatches, the verifier resolves the selected row to its
   `realization_callable_identity`, joins the operation's emitted actuals
   against that callable's published `erased_scalar_formals`, requires exact
   arity, checks each actual is a closed `ScalarTerm` in the caller's
   namespace, and substitutes actuals into the selected callable's
   `requires` at the call site — the same reconstruction the static
   `erased_arguments` lane already performs. The lane has no runtime operand,
   so descriptor layout, ABI placement, and execution are unchanged.

## Invariants the channel must preserve

- Erased actuals remain proof-only: no runtime operand, no fuel, no
  structural place; they instantiate proof, not storage.
- Roster equality is a property of the requirement signature: every
  realization of a requirement declaring erased formals shares the declared
  roster arity, pinned at conformance/descriptor establishment.
- Rebound and stored descriptors cannot widen or lose the lane: a
  descriptor established with the roster publication may only dispatch to
  callables that published the same roster.
- Forged or missing actuals reject in source-free verification; the
  verifier replays, never trusts, the emitted lane.

## Explicitly out of scope

- Erased *structural* or `mut`/`const`/self formals through descriptors —
  no checked form exists for those on static calls either.
- Descriptor runtime layout — the channel carries no operand bytes.
- Relaxing which requirements may declare erased formals.

## Evidence to move when landing

- `fail/relevance/dynamic_erased_formal_lane` flips to a pass canary once
  the three legs land; new fail canaries pin arity mismatch, impure actual,
  and roster-diverged conformance rejections.
- The board clause this draft serves ("a `&dyn` call to a requirement
  declaring an erased formal now explicitly refuses its erased lane … a
  larger slice than the refusal") resolves only when the canary flips, not
  when this draft lands.

## Open questions

- Whether `TerminalDynamicRequirement` carries the full roster identities or
  only the arity commitment; arity alone is weaker at establishment but
  keeps the wire surface smaller.
- Whether the erased-proof lane (`erased_proof_parameters`) needs a parallel
  channel in the same change or a follow-up — today both static lanes exist,
  so admitting one without the other creates an asymmetric refusal.
- Codec/wire rows for the new lanes (`terminal-codec`
  `block_wire.rs` already encodes static erased lanes; the dynamic rows need
  their own sections).
