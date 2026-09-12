# Optimizer source-organization audit

[mod.rs](mod.rs) runs the inventory, entrance, catalog, and
retired-path checks. The [implementation overview](../../../omega-rust/optimization.md)
identifies stage owners; this directory owns the enforceable navigation rules.

## Navigation contract

A stage entrance must show its validated input, eligible exact rules, canonical
order, proposal/independent-validation join, and validated output without a
repository-wide search. A short forwarding or re-export wall is not that join.
One adjacent catalog owns exact enablement and order. Family groups and custody
coordinators must not create proxy schedules.

Descend by semantic responsibility into exact rule or boundary owners. Producer
and validator algorithms remain independent; supporting model, identity, and
persistence code belongs with the responsibility it serves. Shared mechanics
belong below the nearest genuine common owner. Tests mirror the
owning artifact/rule family and then positive, refusal, corruption, or
compatibility behavior. Use the smallest necessary structure, not empty template
files. Descendants import their actual dependencies rather than treating a small
parent entrance as a hidden glob-import namespace.

## Maintenance

The audit does not impose file-length ceilings, directory-depth budgets, or
role-marker comments. Moving a coordinator into another file merely to shorten
an entrance does not improve ownership. Keep the actual coordination readable
where it belongs; split code when responsibilities diverge.

[inventory.rs](inventory.rs) owns explicit governed roots and typed rule-stage
descriptors; [entrance requirements](entrances/requirements/mod.rs) own the
semantic descent. A moved root must be updated explicitly, not silently removed
from audit scope. Keep the guard's own domain inventories navigable, and preserve
producer/replay layering checks when moving source. Do not duplicate the current
path inventory or catalog rows in prose.

From the repository root, run the focused check on Windows or macOS:

```text
mbx nextest run -p omega-architecture-test --all-targets -E 'test(optimizer_source_organization_preserves_semantic_owners)' --no-fail-fast
```

Use Cargo directly if `mbx` is unavailable. This check covers organization;
affected semantic, corruption, and pipeline tests remain separate obligations.
