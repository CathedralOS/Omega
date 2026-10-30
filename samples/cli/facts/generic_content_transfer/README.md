# Generic content conservation

Fact: a generic transfer conserves the complete counted content of its owned input.
Every place sends a linear region containing 29 through the explicitly selected
`ConservingTransfer` implementation of `Transfer<Payload>`, observes 29 and
consumes the returned region once. The match join executes both arms and checks
an independent arm tag after consuming the original counted payload, so a wrong
arm cannot hide behind the identical conserved quantities.

This row discharges the `generic_content_transfer` source migration under
**BUILD-OWNED-TARGET-SELECTION**. It also retains the applied conservation-contract
prerequisite of **BUMP-ALLOCATOR-CANARY**: copied contracts keep their own proof
context and formal parameters. This counted model does not grant access to
storage, establish vacancy or implement initialization.

Run `omega run --both <place>/main.omg` for `local`, `field`, `parameter`,
`call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, and
`target_body`. Each declares exit 0 and four hosted `ProgramEntry` bindings. Its
`invalid/main.omg` changes only the transfer body: it drops the original region
and returns a region containing 28, failing the conservation postcondition.
The sources and their loss twins are pinned under
`tests/omega/{pass,fail}/facts/generic_content_transfer_through_<place>_*`.

The former four target-prefixed bodies were identical. Their replacement is one
ordinary named implementation with a real inline body and explicit member calls.
`Transfer` is an ordinary checked trait, so Build provider selection cannot choose
its evidence. The `target_body` path remains the migration witness; this shared
computation needs no invented boundary facade or OS split. The independent
`cleanup_region` operation is selected by the data default's `dropped by` clause.
Its behavior remains a no-op, so this row claims no observable cleanup side effect.

## Requirements exercised

- Establishment: the selected implementation proves the applied trait's complete
  conservation contract. The caller's final comparison observes the payload;
  it does not establish the callee's proof obligation.
- Identity and lifetime: `old(&region)` denotes the entry revision, not a copied
  resource. The positive moves the owned region and consumes it exactly once.
  Count equality grants no storage, identity or access authority.
- Composition: the contract survives all nine places through exact explicit
  implementation selection. The const place names the payload, not a reusable
  linear value; the match join preserves the selected arm and the original region.
- Soundness boundaries: each loss twin rejects the body replacing 29 with 28.
  Public trait evidence does not inherit an unproved stronger guarantee or
  authorize a resource introduction.
- Psi 05–07: copied proof contracts are checked in their own formal scope and
  erased before Terminal execution. Owned moves and selected nominal cleanup
  survive lowering. This row claims no borrow restoration, initialization or
  dynamic dispatch coverage.
- Omega 00–02: hosted Build entry settlement, record layout and ordinary
  argument/result calling plans preserve the linear payload.
- Omega 03–06: calls and state transitions preserve the payload across register
  homes and activation frames without duplicating the owned region.
- Omega 07–09 and emission: native execution agrees with Terminal execution on
  the authored result. Actual observations, producer identities and host limits
  are recorded with the migration qualification; bindings alone establish no
  other-host execution.

## Recorded migration evidence

The optimized producer at fixed base `e3d308817192` first ran the untouched row:
all nine positives exited 0 in both engines, and all nine loss twins rejected
for the conservation postcondition. After migration, the same verified producer
ran all nine positives and loss twins again with those outcomes; its compiler,
library and Cargo input closure is unchanged. The pinned Source comparison has
18 observations and zero losses; nine pinned ARM64 native/Terminal executions
agree at 0. A Terminal-only counterfactual keeps the original owned transfer
but swaps the selected arm, and another corrupts its later scalar observation;
both trap, while the unmodified join exits 0.

Execution acceptance is macOS ARM64. An optional cross-target capture failed at
an immutable artifact-path collision and supplies no execution evidence. The
row does not claim other-host native or cross-target installation acceptance.

## Remaining parent acceptance

The allocator still requires actual-backed vacancy, initialization, placed read,
retirement and exact backing return, then allocator coexistence, failure
conservation and reset. This row does not establish those storage operations.
The wider target-selection/native-binding migration also remains open.
