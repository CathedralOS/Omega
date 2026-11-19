---
name: v2-layer-lock
description: Use when explicitly asked to apply the V2 layer workflow. Settle one open layer’s responsibility, test its handoff, lock it, and evaluate evidence for reopening an earlier layer.
---

# Settle and lock one V2 layer

Work through the V2 junction in the assigned worktree. Start with the layer’s lock status in [PLAN.md](../../../PLAN.md), the relevant [V2 spec](../../../wiki/spec/), and the layer’s own contract. Treat a design question as a discussion. Begin editing when the user asks for implementation.

1. **Define the open layer’s job.** For a concrete program, identify what the layer receives, what decision must be made, and what the next layer needs. Ask “What information is required to make this decision?” and “Which layer owns that information?” Name the field or other part of the output that carries the answer. Read the producer and its consumers to check that account.
2. **Make the architectural changes before testing.** Change the representation, APIs, and ownership together where the design requires it. Remove the consumer code that reconstructs a decision the producer now supplies. During this phase, read existing tests for context and defer test runs. Move to testing when you can trace the decision from its inputs through one owner to its consumers, with the responsibilities of adjacent locked layers intact.
3. **Test the handoff.** Use the [parallel handoff review](../../../PLAN.md#handoff-review) after settling ownership. Have three GPT-6.2 Sol agents examine the producer, consumer and equivalent inputs independently. Reproduce their cases. Run the focused regressions and the open layer’s relevant suite. If a failure exposes an ownership mistake, return to design. If it exposes an implementation bug, fix it and rerun the affected checks.
4. **Lock, commit, then advance.** Before calling a layer locked, check that the owner’s sign-off covers the current handoff. Report what each boundary test proves and any limits on that evidence. Record the signed-off responsibility in `PLAN.md` and align the layer contract with it. State which changes would require reopening the lock. Commit the agreed implementation, contract, and tests, then apply the same process to the next open layer.

## When a later layer reaches back

Pause the proposed change to the locked layer. Name the missing information and trace a concrete case across its existing handoff.

| What the case shows | Who acts |
| --- | --- |
| The handoff already supplies the information. | Fix the consumer to use it. |
| The locked contract promises the information, but the producer loses or misstates it. | Fix the producer under its existing lock and test the handoff. |
| The requested information falls outside the locked contract. | Decide whether the open layer can derive it from facts it owns. If the evidence points to the earlier layer, bring the case forward for an explicit decision to reopen. |

One valid program can demonstrate information loss. Repeated workarounds can reveal a pattern, but repetition alone cannot establish ownership. To propose reopening, show what breaks, where the deciding facts first exist, which contract would change, and what each consumer would do with the revised output. After an explicit reopening decision, apply the same design and test sequence to the revised boundary before locking it again.
