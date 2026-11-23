---
name: layer-loop
disable-model-invocation: true
description: Use only when the user explicitly requests layer-loop to carry a compiler stage or range through successive repairs and verification to an agreed outcome.
---

# Work through a compiler stage or range

Take the stage range, outcome and stopping point from the request. A request for one pass ends after that pass. For a repair through completion, retain the original program and expected result across commits. Follow the repository’s [completion and worktree rules](../../../AGENTS.md) and identify the active crates from the [source](../../../source-rust/subsystems/compiler/source-to-psi/README.md) or [native](../../../source-rust/subsystems/compiler/psi-to-native/README.md) stage map.

1. Record the starting revision, host, focused commands, outcomes and first failing stage for each original case. Run the registered handoff or CLI target when the stage crate’s own tests do not exercise the production route. On resume, reconcile the last recorded cases and commits with the current checkout.
2. Group failures by the deciding fact and its consumer. Vary one input to test whether similar diagnostics share a cause. Pick a group with a reproducible witness. Record the expected result, nearest lawful control, and public field that should carry the decision. If the observed refusal follows the spec, verify its location and run the lawful control.
3. Follow the witness through the ordinary driver to the first wrong decision. Repair it at its owner. Check the producer’s selected value or identity and the consumer’s result after a plausible substitution. With authorized workers, give each independent group one owner and coordinate shared representation changes. A single session can work the groups in sequence.
4. Before merge, check the original cases against the candidate with the same inputs and configuration. Account for each case and replay any new blocker on the baseline. Distinguish a case that reaches a later stage from one that meets its expected result. Use the owning stage fixture and any consumer or CLI control needed for that result. Carry those results through a clean merge.
5. Continue with the next reproducible case within scope. If the same evidence recurs, vary the input or inspect the preceding handoff before repeating the run. When ownership is unclear, bring the smallest example and the governing [spec clause](../../../wiki/spec/) to the repository’s owner-question process. Continue independent work while that decision is pending.

At a checkpoint, record the revision, commands or log paths, remaining case IDs and next action in the existing task or PR. At completion, give the user the verified stage range, host, original outcomes and any unexercised paths. A suite passing on one host supports that host’s result.
