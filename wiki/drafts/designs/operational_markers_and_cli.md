# Conservative acknowledgements and subsystem CLI migration

Ratified 2026-11-06. Source design is owned by
[call acknowledgements](../../spec/language/effects.md#call-site-acknowledgements);
tooling is owned by the [command-line contract](../../spec/build/command_line.md).
This migration plan can be deleted when the linked implementation tasks close.

A stronger no-suspension guarantee must not require deleting a caller's
acknowledgement. Optional cleanup is a requested source transformation.
The CLI routes that work to source tooling, package provenance to packages,
and portable artifact inspection to Psi. Terminal Psi remains an internal stage.

## Delivery ownership

- **STATIC-MACHINE-MIGRATION** in [TASKS.md](../../../TASKS.md): admit conservative
  call markers across ordinary, nested, terminal, generic and machine-value
  calls. Keep authored syntax restrictions separate from semantic envelopes.
- **RC-RELEASE-RECORD-AND-CLOSURE** in [TASKS.md](../../../TASKS.md): source rewrite
  and CLI migration, independent validator selection, public naming, release
  command coverage and cross-host write/recovery behavior.

The marker change can land independently. Re-root CLI commands with the first
`source rewrite` implementation and migrate every live customer in that change;
do not leave two public spellings. Validator tooling can land separately.

## Implementation route

1. Replace exact marker equality with required-envelope inclusion. Preserve
   combined marker order, reject duplicates and apply suspension placement
   restrictions whenever `suspend` is written. Effects, call admission,
   continuation creation and suspension-related carry use the checked envelope;
   ordinary call fences still apply. Audit consumers that inferred effects from
   syntax bits. Preserve callback upper-bound checking before specialization.
2. Expose source-edit analysis through an owning source-tooling facade, reusing
   checked call contracts and retained source spans. The CLI routes requests;
   it does not own proof logic or call compiler stages directly.
3. Implement `redundant-call-markers` over exact source occurrences. Stage all
   edits, recheck the full edited tree, then use the contract's multi-file
   commit/recovery discipline. Add focused failure-injection controls for failed
   rechecks, changed files and interrupted writes. A check-only run never edits.
4. Migrate argument parsers, help/errors, entry routing, tooling invocations,
   tests and documentation together. Route both package audits under packages;
   keep locator/project modes distinct. Preserve both Psi inspection forms.
   Rename exposed `terminal` engine/work selectors to `psi`; internal engine
   enums, crate paths, wire tags and measured historical records need not change.
5. Register exact validator identities with stage/product/target applicability.
   Execute through the owning compiler driver and publish report rows. Explicit
   requests for unproduced stages reject; enumeration explains inapplicability.
   Existing `--verify` package semantics remain independent. Do not create
   validators merely to populate a registry or present unchecked coverage.
6. Sweep public help, reports, installation/package guidance, learn content and
   artifact descriptions for internal-stage naming. Keep genuine pipeline
   distinctions and source-language terminal transfers intact.

## Acceptance and scope

Use the [required acceptance](../../spec/build/command_line.md#required-acceptance)
controls in existing CLI, checker and corpus suites. Executable marker controls
and transformed programs must agree under `omega run --both`; required-marker,
position, carry and authority failures remain rejected for their intended reason.
Preserve semantic effect envelopes and emitted behavior when only a redundant
acknowledgement changes; source identity and diagnostic/debug provenance may
reflect the edited spelling.

This ratification changes documentation and task acceptance. Parser/runtime
migration is not established by these edits. Source code, snapshots, scripts and
recorded execution evidence still using old CLI spellings belong to the migration
sweep; historical command records remain tied to the compiler that produced them.
No formatter, default cleanup warning, lint framework, convenience aliases or
assurance profiles are added.
