---
name: layer-lock
disable-model-invocation: true
description: Use only when the user explicitly requests layer-lock to repair a compiler handoff, assess a proposed change to its contract, or verify a stage for sign-off.
---

# Verify one compiler handoff

Start with the program and decision the user wants verified. Find the governing [Omega spec](../../../wiki/spec/), then follow the production driver from the stage that writes the fact to the stage that consumes it. The [source stage map](../../../source-rust/subsystems/compiler/source-to-psi/README.md) and [native stage map](../../../source-rust/subsystems/compiler/psi-to-native/README.md) identify the drivers, exchanged representations and entry points.

1. **Predict an observable difference.** Choose a program that forces the consumer to use the fact. Name its producer, the field carrying it, the source occurrence it belongs to, and the expected consumer result. Choose a nearby valid case and a plausible substitution. Write down which substitutions a consumer should accept. Two returns can both be well typed even though only one matches the source.
2. **Find where the fact changes.** Run the original program through its ordinary entry point. Inspect what the consumer actually receives. If the right field and identity arrive, fix the consumer. Otherwise walk backward one handoff at a time until you find where the fact is first missing or wrong. For a computation supplied later, inspect the pending request and its arguments before supplying a result. A harness that supplies the answer up front can hide a missing request.
3. **Check ownership from both sides.** Assert the chosen identity on the producer’s public output, alongside an unrelated occurrence when the program has one. Read that output through the production consumer’s intended dependencies. Inspect its manifest and public types for accidental dependencies on an earlier stage. Test direct and forwarded forms when the spec gives them the same meaning. Record any part exercised only by a test consumer.
4. **Exercise the distinction.** Start each substitution from a fresh accepted output. Change one operand, identity, proof source or occurrence. Check the verdict, its reason and its location at the stage responsible. Then rerun the original program and its valid neighbor through the ordinary route. For runtime behavior, use the repository’s native/Psi comparison on the available host.

For example, L5 must emit the scalar return operand named in source. L6 can accept either parameter of the same type. For a byte read, L6 must refuse a length measured on another view even when both lengths equal one. The existing [return and byte-read tests](../../../source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/lowered_facts.rs) exercise both distinctions. Run them from the repository root with the `source-compiler` test target.

```sh
mbx test --locked -p source-compiler --features test-support --test checking_lowered_facts
```

For sign-off, report the revision, contract, deciding field, producer and consumer assertions, commands, host and unverified cases. A repair closes the cases it exercised. A whole-stage sign-off must account for the rest of that stage’s claimed responsibilities and any required owner decision. If an earlier contract must change, show the first lost fact, the spec clause and the consumer behavior that would change. Apply the repository’s [scope and escalation rules](../../../AGENTS.md#scope-and-escalation) to unresolved ownership.
