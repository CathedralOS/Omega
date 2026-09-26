# bump_allocator_canary `omega run` wall — contract-entailment open obligations

Lane: `leaf/bump-allocator-canary` @ leg HEAD `7a1af0074e`.
Binary: `target/release/omega` (pre-restructure build; crate logic identical).

## Advancement this leg

`omega --check tests/omega/pass/memory/bump_allocator_canary/main.omg`
previously declined at `provider selection operand does not resolve to one
visible product declaration` — `build.omg` selected
`ExtentRootProvider → BumpRootProvider` but the type was declared nowhere.
The leg adds the one-block declaration
(`machine BumpRootProvider::grant_root(root: Extent) -> Extent satisfies
ExtentRootProvider::grant { root }`, the `extent_root_provider_adapter`
pattern). The fixture now checks clean: **16 source files, exit 0**.

`omega run` then advances through compile and package-evidence capture and
stops at evidence admission:

```
error: cannot accept fresh package review evidence: accepted ordinary
  evidence policy check failed: fresh ContractEntailmentOpenObligation
  obligation requires a concrete later discharge and cannot be admitted
  by root policy
```

`omega update --project <fixture>` declines the same obligations earlier:

```
package "alloc" has 9 unresolved contract obligations; project decisions
cannot discharge proofs
```

## The 9 obligations — all `alloc`, all `UnrecognizedInductiveBody`

From `omega audit packages --project <fixture> --details`:

- `allocate` (3): `result.strategy.remaining == strategy.remaining - length`;
  `result.allocation.length == length`;
  `embed(result.strategy.tail.length) + embed(result.allocation.length)
   == embed(strategy.tail.length)` — the counted-residual + tail
  conservation laws the leg is asked to prove.
- `open` (1): `result.buffer.length == capacity`.
- `release` (2): `embed(result.tail.length) ==
  embed(returned.length) + embed(strategy.tail.length)`; same for
  `result.remaining`.
- `reset` (1): `embed(result.remaining) == embed(returned.length) +
  embed(released.length) + embed(strategy.tail.length)`.
- `shrink` (2): `result.buffer.length` geometry; tail/kept conservation.

The fixture package itself has **zero** obligations — its boundary
requirements publish contracts but are `TopLevelRequirement` supply, which
the stand-down accounting skips.

## Owning stage + mechanism

Producer: `omega-rust/psi/pipeline/04_typed-trees-to-checked-trees/src/
validation/proof_contracts/contract_entailment.rs` +
`contract_entailment/inductive_judgment.rs`.

For `CheckedBody` machines, `account_stand_downs` is on. A non-empty body
routes every expression `ensures` into `inductive_transition_entailment`,
which only judges bodies shaped as pure `guard -> value` / tail-self-call
transition arms. Bodies containing let-bindings or calls take
`UnrecognizedInductiveBody` stand-downs on **all** expression ensures —
before any goal-specific judgment.

Projection: `omega/src/package_evidence/capture/contracts/stand_downs.rs`
emits `ContractEntailmentOpenObligation` review rows.

Admission: `omega/src/package_manager/review/reconstruction/
root_policy.rs` (`UnresolvedLaterDischarge`) for `run`, and
`operations/package_change/error.rs` (`UndischargedContract`) for `update`.

## Why they cannot discharge today

The only discharge tier is the assumption certificate
(`proof/contract_entailment.rs::build_contract_entailment_assumption_discharges`),
which fires only for `UnrecognizedInductiveBody` and then only when
`selected_assumption_position` finds the goal's lowered `Proposition`
**verbatim** among the machine's `requires` facts. Two structural bounds
make the bump ensures unreachable:

1. `lower_proposition`/`lower_scalar_term` accept only boolean literals,
   integers, and **single-member** `Name` atoms bound to scalar entry
   parameters. Every goal above mentions `result.*` member paths or
   `embed(...) + embed(...)` arithmetic — lowering returns `None`.
2. `requires` cannot express the goal even where `result` parses
   (untyped in pre-state): a counted-residual postcondition is not a
   precondition.

So for any imperative `CheckedBody` machine carrying `result`-mentioning
expression ensures, the obligation can neither be proven by requires →
ensures entailment nor discharged by citation. `--check` already proves
these facts (the body-exits verifier — they pass), but the package-evidence
ledger records them as undischarged obligations; there is no
"verified by body exits" discharge row.

Net: `omega run` is dead for any package depending on a library whose
checked machines publish result-geometry ensures — the "bump open
obligations" front-door wall. A discharge tier keyed to the body-exit
proof (or a broader proposition lowering covering member paths and
linear arithmetic) is the missing piece.

## Walls queued behind this one (not yet reached)

- Hosted entry visible-parameter policy is `None`
  (`psi/target/src/target_profile.rs::program_entry_slot`), so the
  exercise machines' `whole: Extent in Granted & Vacant` parameter can
  never be supplied at `Main::main(&mut self)` — the two provisioned
  roots are bridge-internal. A run-visible entry storage surface
  (`ProgramStorageEntry` satisfaction or equivalent) is required before
  main can own an extent.
- `ExtentPartition` and `ResidentStorage` have no selectable
  provider implementations; `select_provider` settlement for
  contract-bearing boundary requirements on the run path is untested
  behind the obligation wall.
