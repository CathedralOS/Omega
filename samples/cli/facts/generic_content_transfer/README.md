# Generic content conservation

Fact: a generic transfer conserves the complete counted content of its owned input.
Every place sends a linear region containing 29 through `Transfer<Payload>`,
observes 29 and consumes the returned region once. Both selection paths run.

This is a BUMP-ALLOCATOR-CANARY prerequisite: applied conservation contracts
must retain their own proof context and formal parameters. This counted model
does not grant access to storage, establish vacancy or implement initialization.

Run `omega run --both <place>/main.omg` for `local`, `field`, `parameter`,
`call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, and
`target_body`. Each declares exit 0 and four hosted target bindings. Its
`invalid/main.omg` changes only the transfer body: it drops the original region
and returns a region containing 28. That must fail the conservation postcondition.
The nine place sources are pinned as
`tests/omega/pass/facts/generic_content_transfer_through_<place>_exit` with
the loss twins as
`tests/omega/fail/facts/generic_content_transfer_through_<place>_rejected`.

## Requirements exercised

- Establishment: the selected implementation proves the applied trait's complete
  conservation contract. The caller's final comparison observes the payload;
  it does not establish the callee's proof obligation.
- Identity and lifetime: `old(&region)` denotes the entry revision, not a copied
  resource. The positive moves the owned region and consumes it exactly once.
- Composition: the contract survives all nine places, including four checked
  target bodies. The const place names the payload, not a reusable linear value.
- Soundness boundaries: each loss twin rejects. Focused core fixtures also reject
  executable proof projections, inherited-but-unproved conservation, loss in an
  unselected target body and a generic adapter claiming unissued residency.
- Psi 05–07: copied proof contracts are checked in their own formal scope and
  erased before Terminal execution. A static-callback fixture additionally
  exercises substituted callable parameters. This row claims no borrow
  restoration, initialization or dynamic dispatch coverage.
- Omega 00–02: hosted Build entry settlement, record layout and ordinary
  argument/result calling plans preserve the linear payload.
- Omega 03–06: calls and state transitions preserve the payload across register
  homes and activation frames without duplicating the owned region.
- Omega 07–09 and emission: native image execution agrees with Terminal
  execution on both paths and exit 0. Execution evidence is macOS ARM64;
  authored bindings alone do not establish execution on the other hosts.

## Current frontier

The original generic transfer rejected its copied `old` and `content` contracts
as executable expressions. All nine positive places now run with native and
Terminal exit 0; all nine loss twins reject. Callable checking contexts remain
separate from callable theorem sources and cannot confer provider authority.

Pinned for this host at base `c1c6bc8cb8ec`: 9/9 positives `checked` with
`check-target:macos_arm64` and 9/9 loss twins `rejected` with
`cannot prove ensures contract` in `corpus_outcomes.txt`, 9/9 native
`built ... exit:0` rows in `corpus_native_macos_arm64.txt` and 9/9
`exit:0 interpreter-exit:0 interpreter:agrees` rows in
`corpus_both_macos_arm64.txt`.

The parent still requires actual-backed vacancy, initialization, placed read,
retirement and exact backing return, then allocator coexistence, failure
conservation and reset. This row does not establish those storage operations.
