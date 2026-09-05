# Tasks: Package Manager

Remaining work for repository install/update and compiler-derived capability
review. [Scope](wiki/design_briefs/package_manager_first_draft.md) ·
[Build model](wiki/design_briefs/build_and_package_model.md) ·
[Source map](omega-rust/omega/packages/README.md).

The project trusts whoever lands its dependency and lock changes. `omega.lock`
records selected sources and accepted capabilities/assumptions, not proof of an
audit. Use the existing compiler to check code; native artifact verification,
host credentials, and organizational review policy have separate owners.

## Remaining work

No implementation items are currently scheduled. Optional model integration
is not an install/update prerequisite; its existing adapter and future
integration constraints belong in the
[advisory documentation](omega-rust/omega/packages/review/advisory/README.md),
not an open-ended execution task.

## Ownership and limits

`TASKS.md` owns generic/representation composition, opaque ABI/lifecycle,
native publication, and std migration. `TASKS_OPTIMIZER.md` owns optimization
and physical replay. Their results enter review when relevant; completing all
of them is not a source-package prerequisite. Process resource/lifecycle
mechanisms belong to `omega-rust/omega/tooling/bounded-process/`; stronger host isolation,
SSH credentials, and audit seriousness belong to the operator.

Changing both a dependency's source and alias may appear as removal plus
addition, with the new package checked in full. A special paired-replacement
command is not required. Never infer common identity from names or row order.

Any further security task must name a concrete invariant Omega can enforce.
Escalate an ambiguous boundary before adding machinery; do not invent lock
certification, proof of review, or protection from the project author.
