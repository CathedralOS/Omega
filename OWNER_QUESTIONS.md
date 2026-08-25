# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Last pruned: 2026-08-25.

## Q1 — Physical ABI for opaque by-value boundary data

### Context

`InterruptAcknowledgement` and `InterruptMaskGuard` are now public opaque
linear boundary data. Their provider-owned settlement fields are correctly no
longer source-visible. Both can nevertheless cross a boundary by value; for
example, `InterruptEntry::enter` receives an `InterruptAcknowledgement` and is
governed by a source-authored `Calling<C>` policy.

### Problem statement

Calling-policy evaluation needs a target-specific byte size and alignment
before it can validate a by-value placement. Opaque boundary data deliberately
has no ordinary Omega layout, and package review currently records its ABI and
mechanism as `Unbound`. The compiler therefore rejects the interrupt entry as
zero-sized. Restoring public structural fields, treating the value as a ZST, or
hardcoding its former five-`u64` shape would each contradict the opacity and
representation-TCB decisions.

### Proposed direction

Keep the source type opaque, but require the selected provider/installation to
supply a compiler-validated, target-specific representation descriptor before
evaluating any `Calling<C>` policy that passes the value by value. The policy
may inspect only the closed shape descriptor, never provider fields. Review and
eventual admission should replace `Unbound` with the exact ABI and mechanism
commitments and reject when no unique descriptor is selected.

### Alternates

- Acceptable if it matches the intended machine contract: make opaque
  obligations cross this boundary through an explicit reference/handle shape,
  so no by-value representation is promised.
- Tempting but wrong: restore public identity fields merely to recover layout.
- Tempting but wrong: assign a compiler-global magic size or accept zero-sized
  placement without selected representation evidence.
