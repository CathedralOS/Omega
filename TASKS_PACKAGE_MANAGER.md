# Tasks: Package Manager

Remaining work for repository install/update and compiler-derived capability
review. [Scope](wiki/design_briefs/package_manager_first_draft.md) ·
[Build model](wiki/design_briefs/build_and_package_model.md) ·
[Source map](omega-rust/omega/packages/README.md).

The project trusts whoever lands its dependency and lock changes. `omega.lock`
records selected sources and accepted capabilities/assumptions, not proof of an
audit. Use the existing compiler to check code; native artifact verification,
host credentials, and organizational review policy have separate owners.

## Required integration

- [ ] **PACKAGE-MANAGER-RELEASE-AUDIT.** Extend real-command/network coverage to
  transitive helper authority.
  Test HTTPS and SSH independently where credentials permit.
  Cover missing baselines/old source, invalid proofs, spoofed boundaries,
  concurrent edits, and interruption recovery. Run relevant package, resolver,
  compiler-handoff, and architecture checks; report unavailable platforms or
  credentials explicitly.
  Acceptance: successful commands produce usable dependencies; failed stages
  preserve or recover accepted files. Reuse bounded-process tests for helper
  failure/cleanup rather than making OS hardening a package feature.

## Optional conveniences

These do not block the supported online install/update workflow.

- [ ] **OFFLINE-COMMAND-SELECTION.** Expose existing offline exact-pin recovery
  through locked compilation and applicable package command options.
  Acceptance: cached accepted/proposed pins work without network; missing
  content fails clearly without selector refresh or accepted-file mutation.

- [ ] **OPTIONAL-AUDIT-ADVICE.** If connecting the existing `review/advisory/`
  adapter, keep provider configuration with CLI/tooling. It may recommend a
  closer audit, never suppress compiler findings or accept decisions.
  Acceptance: the complete core workflow works with no model configured and
  when advisory invocation fails. No built-in auditing infrastructure is needed.

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
