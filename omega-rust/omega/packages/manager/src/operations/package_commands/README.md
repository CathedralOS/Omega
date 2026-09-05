# Install and update

Enter through [`../package_commands.rs`](../package_commands.rs). The CLI at
`omega/src/command/package.rs` parses arguments; this operation owns resolution,
checking, review, and publication.

```text
package_commands/
├── model.rs       commands and outcomes
├── planning.rs    dependency edits and exact update selections
├── candidate.rs   stage, resolve, check every target, and publish
├── review.rs      compiler findings and per-change decisions
├── source_review.rs  separate bounded source-code diagnostics
├── old_sources.rs    exact old-source recovery through custody issuers
├── state.rs       retained proposal and review files
├── proposal.rs    restart record
└── proposal/      bounded framing and codec tests
```

```text
omega install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>]
omega update [package-or-alias...] [--to <revision>]
omega install --resume
omega update --resume
omega <install|update> --discard-review
```

Both commands accept `--project <dir>` (default current directory). New commands
accept repeated `--target <name>`; they retain every existing lock target and
add requested ones. First acceptance defaults to the host when none is named.
The project must already have a valid `build.omg`.

Install supports Git HTTPS/SSH and local sources through existing adapters.
The fetched `build.omg` supplies the package name and default import alias;
`--as` is an optional local override. Git defaults to `HEAD` when `--rev` is
omitted. `--package <declared-name>` selects one Git workspace member by its own
package declaration. The resolver discovers the member inside the verified
workspace; no member path is accepted. Omission keeps root selection, and local
sources use their package directory directly. The member's declared name still
supplies its default alias; `--as` overrides only that local alias. Unknown or
duplicate declared names reject before publication. For example:

```text
omega install https://example.org/team/libraries.git --package exact-math
omega install https://example.org/team/libraries.git --package exact-math --as math
```

Selection stays in the proposed declaration and source graph during review.
Resume uses it without repeating `--package`. Complex declaration layouts receive a
manual-patch diagnostic rather than an unsafe automatic rewrite.

Update without selections refreshes the graph. Selections resolve a root alias
first, otherwise a unique package name. Selected Git repository members move
together; unrelated repositories keep exact pins. `--to` requires one selection
with a root-authored Git request. A missing lock requires an unselected update
and fresh graph review; unsupported lock formats reject before acquisition.

Blocking findings return exit status 3 and leave accepted project files intact.
Edit each `pending` decision to `accept` or `reject` in the reported review
files, then use the matching command's `--resume`. Only those tokens are
editable. Rejection keeps the proposal unpublished. Findings include complete
compiler-derived policy changes and audit recommendations, not package prose
or proof that somebody audited the source. Nonblocking candidates publish
directly; retained dangerous authority still recommends an audit on upgrades.

Ignored `build/package-manager/` contains:

- `proposal`: candidate pins, proposed build bytes, targets, and original
  project-file/source identities needed to resume.
- `review-<target>.txt`: compiler-rendered findings and editable decisions.
- `source-diff.txt`: separate, escaped source-code patches, regenerated on resume.
  Its contents are hostile source data and cannot supply project decisions.
- `check-<target>/`: disposable compiler/build outputs.
- `transaction.lock` and `pending`: the separate publication mutex and
  commit-intent journal, owned by [publication](../publication/README.md).

Resume fetches only the proposed exact pins if needed, recompiles, and rejects
source, graph, accepted-file, or finding drift. Missing old source does not
prevent comparison with accepted policy. Source diagnostics recover recorded
Git commits and named members without refreshing selectors. Unchanged candidate
custody or an exact live local root can also supply the baseline. Changed local
dependencies have no cache-only old-source recovery entrance and receive
standalone candidate output. The report names missing old source, binary content,
and rendering limits explicitly. Source patches share an 8 MiB output ceiling
and the renderer's independent per-package limits; they never enter capability
review or decision recovery.

`--discard-review` removes the proposal, leaving review files as diagnostics.
It does not roll back accepted files or discard publication recovery. Every
operation recovers pending commit intent before using the accepted pair.

Candidate builds use the existing scoped evaluator: source reads, disposable
output writes, and compiler logging are possible before a later rejection.
Runtime boundary services and resolver credentials are not supplied to builds.
The lock records trusted project decisions, not an audit certificate. Separate
compiler/native admission policy remains outside this transaction.
