# Optimizer source-organization audit

[mod.rs](mod.rs) runs the inventory, bounds, role, entrance, catalog, and
retired-path checks. The [implementation overview](../../../omega-rust/optimization.md)
identifies stage owners; this directory owns the enforceable navigation rules.

## Navigation contract

A stage entrance must show its validated input, eligible exact rules, canonical
order, proposal/independent-validation join, and validated output without a
repository-wide search. A short forwarding or re-export wall is not that join.
One adjacent catalog owns exact enablement and order. Family groups and custody
coordinators must not create proxy schedules.

Each governed `lib.rs` and `mod.rs` declares exactly one source-local role:
`Optimizer module role: crate map.`, `Optimizer module role: stage group.`, or
`Optimizer module role: executable entrance.` Crate maps name responsibilities;
stage groups map neighboring boundaries without execution; executable entrances
own the registered coordination seam. The audit checks the declared role against
its expected role and required seam, not merely the presence of a comment.

Descend by semantic responsibility into exact rule or boundary owners. Keep
proposal, independent replay, model, identity, persistence, and broad fixtures
out of the same file. Shared mechanics belong below the nearest genuine common
owner; producer and validator algorithms remain independent. Tests mirror the
owning artifact/rule family and then positive, refusal, corruption, or
compatibility behavior. Use the smallest necessary structure, not empty template
files. Descendants import their actual dependencies rather than treating a small
parent entrance as a hidden glob-import namespace.

## Bounds and maintenance

[bounds.rs](bounds.rs) sets 600 lines for governed production Rust files,
800 for tests/fixtures, and a preferred 100-line limit for governed `lib.rs` and
`mod.rs` files. An entrance exception requires an exact path, semantic reason,
and non-growing ceiling no higher than 200. The source-file and entrance exception
lists are currently empty. Missing, duplicate, invalid, or stale exceptions
reject; line counts are a review aid, not a substitute for cohesion.

[inventory.rs](inventory.rs) owns explicit governed roots and typed rule-stage
descriptors; [entrance requirements](entrances/requirements/mod.rs) own the
semantic descent. A moved root must be updated explicitly, not silently removed
from audit scope. Keep the guard's own domain inventories navigable, and preserve
producer/replay layering checks when moving source. Do not duplicate the current
path inventory or catalog rows in prose.

From the repository root, run the focused check on Windows or macOS:

```text
mbx nextest run -p omega-architecture-test --all-targets -E 'test(optimizer_source_organization_is_bounded_and_navigable)' --no-fail-fast
```

Use Cargo directly if `mbx` is unavailable. This check covers organization;
affected semantic, corruption, and pipeline tests remain separate obligations.
