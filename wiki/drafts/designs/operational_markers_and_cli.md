# Conservative acknowledgements and subsystem CLI migration

The rules live in
[call-site acknowledgements](../../spec/language/effects.md#call-site-acknowledgements)
and the [command-line contract](../../spec/build/command_line.md), including its
[source rewriting](../../spec/build/command_line.md#source-rewriting) and
[required acceptance](../../spec/build/command_line.md#required-acceptance)
sections; this note keeps only the implementation route. The CLI recognizes
`omega source rewrite` but reports it as not implemented
(`omega-rust/omega/application/cli/src/arguments/mod.rs`). The marker work
belongs to **STATIC-MACHINE-MIGRATION** and the source rewrite and CLI work to
**RC-RELEASE-RECORD-AND-CLOSURE** in [TASKS.md](../../../TASKS.md). Delete this
note when both close.

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
