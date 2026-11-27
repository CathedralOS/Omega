# Command-line tools

The `omega` application routes requests to compiler, execution, package, Psi
inspection and source-editing subsystems. Argument parsing and presentation
belong to the dispatcher; semantic analysis, validation and edits belong to
their owning subsystem. Subcommands do not assemble private compiler stages.

Named decision: `subsystem-command-routing`, ratified 2026-11-06.

## Command vocabulary

Compilation and execution remain the flat product commands. Other tools use
`omega <subsystem> <action>`. A subsystem need not have multiple actions to
justify its name. The source subsystem owns operations on authored source files,
including parsing and rewriting. The CLI routes each implemented action to its
owner.

```text
omega [--verify] [--validator <ExactName>]... [--every-validator] <root.omg>
omega --emit-psi <file.psi> [--max-psi-output-bytes <bytes>] <root.omg>
omega --trusted-artifact <manifest-id> [--max-psi-input-bytes <bytes>] <program.psi>
omega --check [--floor-only] [--verify] [--validator <ExactName>]... [--every-validator] [<root.omg>]
omega build [--allow Network] [--allow ProgramLaunch] [--program <logical-path> <executable> [--program-read <path>]... [--program-write <path>]... [--program-timeout <milliseconds>]]... [--compare-tests] <root.omg>
omega run [--engine native|psi] [--target <name>] <root.omg>
omega run [--engine native|psi] --trusted-artifact <manifest-id> [--max-psi-input-bytes <bytes>] <program.psi>
omega source parse [--] <file>
omega source rewrite <ExactRule>... [--check] <root.omg>
omega packages audit --kind <local|git> <locator> [--rev <rev>]
omega packages audit [--project <dir>] [--target <name>]... [--details] ...
omega packages install <source> ...
omega packages update [package-or-alias...] ...
omega psi inspect --machine <qualified> [--target <name>] [--output <file>] <root.omg>
omega psi inspect --artifact <file> [--max-psi-input-bytes <bytes>]
```

These forms show routing, not an exhaustive restatement of existing options.
Build and run share the same [initial entry selection](#build-first-entry-selection).
`--allow Network` permits the root Build to use an admitted live network provider.
Omitting it leaves the root offline. Each dependency Build requires an explicit
[delegation at registration](scoped_execution.md#live-networking-in-a-build).
`--allow ProgramLaunch` permits the root Build to use the [specific program bindings](program_launch.md#grant-and-delegation) supplied by the invoker.
`omega build --compare-tests` requests [native/Psi comparison of registered
tests](testing.md#comparing-native-and-psi-tests). This replaces `omega run --both`.

`--emit-psi` writes the compiled Psi artifact subject to the destination's
capacity. Pass `--max-psi-output-bytes` to set an inclusive output quota.
The receiving host can pass `--max-psi-input-bytes` to set an inclusive
encoded input limit when it [loads the artifact](embedding.md#loading-and-preparation).
Both options take a nonnegative decimal byte count. Omitting an option leaves
that byte count uncapped.

`omega source parse` checks the syntax of the whole file. Success
exits silently with status 0. File-read, lexer and parser errors go to stderr
with status 1. Source refusals include the path, one-based line and byte
column, and byte range. Invalid usage exits with status 2. Use `--` before
a file name beginning with a dash.

In the [source-based workflow](../packages/staged_inputs.md#installing-and-updating),
`omega packages`, `omega packages help add` and `omega packages help update`
provide instructions for editing the consuming stage. The version 4 package manager
uses the following acquisition, review, resume and discard forms during migration:

```text
omega packages install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>]
omega packages update [package-or-alias...] [--to <revision>]
omega packages install --resume
omega packages update --resume
omega packages <install|update> --discard-review
```

`--as` selects the requester's alias for the dependency; the default is its
declared name unchanged. `--resume` continues the same operation after its
review document's `pending` decisions were edited to `accept` or `reject`, and
`--discard-review` abandons it with accepted files unchanged. `--offline`
restricts acquisition, resume and historical-source diagnostics to local
sources and cached recorded Git pins; it skips no check and does not authorize
a cached branch for a newly requested selection. Package audit retains project
targets, details, offline and Build-input options. Inspection retains both source-machine and artifact forms.
With no explicit root, `omega --check` looks for a conventional `build.omg`
in the current directory and its ancestors. One candidate selects that entry.
If none exists, the command reports a missing enclosing entry. Multiple candidates
require an explicit path, before any candidate runs. Name the workspace
catalog to check every declared member as a [validation subject](validation_subjects.md),
or name the member's entry to check that member alone. `--verify` withholds trust
acceptance and re-verifies; it does not widen the subject set.

The locator audit examines a package source, not editable program text.
`--kind` selects locator inspection. A locator requires that selector;
`--rev` belongs to that form. Reject combinations with project-audit-only
arguments. Without a locator form, audit inspects the named project or its
ordinary current-directory default. Inspection does not accept package changes.

There is one public spelling for each operation. Former flat install/update,
action-first audit and hyphenated inspection commands are not compatibility
aliases. The implementation migration must update callers, diagnostics, tests,
guides and command examples together.

## Local Build inputs

The project selects additional packages in Build source. See
[packages available to the initial Build](../packages/staged_inputs.md#packages-available-to-the-initial-build).

By default, a local Build can read its entry and the package-local source files
admitted to compile it. To capture a package asset alongside the source, include it in the invocation inventory.
For example, a package with a Build entry, one helper, and a lookup table can use the following invocation.

```text
omega build --build-input build.omg --build-input helper.omg --build-input table.txt build.omg
```

Each `--build-input` names a required file or subtree. An
`--optional-build-input` records whether that path exists. An explicit inventory
must include all required source files. Without that list, the compiler retains
the local package snapshot for resolution and its package capture limits apply.
A dependency receives its sealed package snapshot unless its caller supplies
another filesystem view. [Build source capture](scoped_execution.md#snapshot-and-staging-protocol)
and [local package capture](../packages/sources.md#local-source-capture).

## Initial Build directory mappings

Named decision: `initial-build-directory-mappings`.

On both `omega build` and `omega run`, the repeatable options
`--build-directories-readonly` and `--build-directories-readwrite` grant extra
live directories to the initial Build's filesystem provider. Each takes one
`host-directory::/guest-directory` mapping with the same path, confinement,
on-demand access and read/write semantics as the [runtime directory mappings](#runtime-directory-mappings).
The caller supplies the backing before the Build runs, within its admitted
reach. With no extra mappings, the Build has its read-only captured package
and private, bounded writable scratch.

```sh
omega run --engine psi \
  --build-directories-readonly ./grammar::/grammar \
  --run-directories-readonly ./documents::/input \
  build.omg
```

Here `use helper.omg` in `build.omg` resolves from the captured package before
entry. The running Build can then open `/grammar/sql.txt` as data. A Build
mapping leaves source capture, import lookup and package registrations unchanged.
The final parser can read `/input` through its separate Product grant.
Dependencies and successors receive extra directories only through explicit
[delegation](scoped_execution.md#supplying-a-filesystem-to-a-child-build);
a successor can also use a deliberately selected [package root](declarations.md#successor-package-and-directories).

A writable mapping lets Build code write ordinary files. The driver publishes
the selected Product to the caller-selected output directory, defaulting to
`build/` beside the input root. Publishing the binary requires no writable
directory mapping for the Build. A mapping at a guest path such as `/out`
does not redirect artifact publication.

## Runtime directory mappings

Named decision: `psi-run-directory-mappings`.

A caller can give an interpreted Product access to live host directories.
Each repeatable option takes one `host-directory::/guest-directory` mapping.
The host path can be absolute or relative to the caller's working directory.
The guest sees the right-hand path.

```sh
omega run --engine psi \
  --run-directories-readonly ./documents::/input \
  --run-directories-readwrite ./results::/out \
  parser.omg
```

In this example the parser opens `/input/query.txt` by a path written in its
source. The first mapping lets it read that file and list `/input`. The second also allows creation,
modification, renaming and deletion within `/out`, subject to host permissions
and the program's admitted operations. A run may mix mappings with different
access modes. Program launch requires its own authority.

The driver builds the Product up to Psi and retains these grants for its
execution. A preparing Build, its dependencies and its successors gain no access
from a run mapping. Before entering the Product the driver supplies the view to
the admitted filesystem provider. Reads and writes stay within the granted
roots, including during link traversal and concurrent path changes. A missing
file in a mapped directory cannot trigger a lookup on a broader host filesystem.

Mapped files are opened on demand. A fresh open can observe a host-side change
even in a read-only mapping, and a permitted write updates the mapped
directory. Startup requires no preload into RAM. Compilation uses the captured
source bytes even if a later runtime read sees different contents.
[Local Build inputs](#local-build-inputs)

These options describe interpreted Product execution. Keep the existing
[engine selection](embedding.md#standalone-execution-selection), including its
native default. A mode that cannot enforce a requested mapping must report
that before running the Product. See the separate
[memory filesystem proposal](../../drafts/designs/run_memory_filesystem.md).

## Build-first entry selection

`omega build <file>` and `omega run <file>` start from the file the caller names. Both use the same first-stage rule in the core package.
A build requests the product artifact. A run requests execution under the
[engine and target rules](embedding.md#standalone-execution-selection).
Building a package checks its source, runs its registered tests and publishes
its declared Build outputs. Only an executable Product requires a program entry.

1. Look for a free `machine build` in the named file. If present, bind it as the
   [build entry](declarations.md#project-role), including when the file also
   declares `machine main`. Run the build after the stage checks. An invalid or
   ambiguous build entry fails the invocation.
2. If the file has no build entry, use core's default Build configuration and
   bind `machine main` in that file as the product entry. Check it under
   the ordinary [target entry contract](entry_roots.md). A missing or invalid
   `main` fails the invocation.

A neighboring file has no effect on this selection. After a failed invocation,
the CLI may suggest explicitly naming a neighboring `build.omg`. Giving a
build file any other name has the same semantics.

A build stage starts with its own service backings already selected. Its plan
selects the inputs, providers and exact entry for the next stage within the
grants the build received. The driver follows the entry's reflection identity
directly. For example, a build in `release.omg`
can select `main` in `release.omg` as its product. The initial build-first
lookup occurs only once. Selecting the same file with a different entry is
valid. A plan that depends on its own active stage request, directly or
indirectly, forms a cycle and is refused.

The compiler checks the whole source file with the inputs available to each
stage before running its selected entry. If `build` generates a type named
`Schema` and a same-file `main` uses it, checking the build stage fails at the
use of `Schema`. A handwritten Product entry in a separate source file can
import the generated type normally once both files belong to the
[Product source view](execution.md#generated-source-boundary).

## Public Psi naming

**Psi** is the public name of the portable IR, artifact and interpreted execution
product. Use it in command names, help, reports and user-facing documentation.
`psi inspect` does not expose a compiler stage name.

`Terminal Psi` names an internal compiler stage or its representation. It is
appropriate when distinguishing that stage from checked or lowered trees, and
in internal contracts, crate paths and representation identifiers. It is not
the public artifact name. Source-language terminal transfers/outcomes are
unrelated terminology and keep their ordinary meaning.

The interpreted engine spelling is `--engine psi`. The interpreted work limit
is `--psi-work-budget <positive-u64>` wherever that budget is applicable.
See [standalone execution selection](embedding.md#standalone-execution-selection)
for the effect of an explicit target on a single-engine run.

## Source rewriting

`omega source rewrite <ExactRule>... <root.omg>` requests only the exact named
transformations, with at least one rule. Unknown names reject. There are no
implicit style tiers, automatic rule bundles or rewrites during ordinary
compilation. The rule registry specifies applicability and the preservation
argument for each transformation. Rechecking alone proves validity, not
behavioral equivalence.

The initial rule is `redundant-call-markers`, covering both `suspend` and `block`.
Resolve calls under the [acknowledgement contract](../language/effects.md#call-site-acknowledgements).
Remove a marker only when its axis is absent from the source occurrence's
checked envelope. A generic body uses its callback upper bounds; a currently
synchronous instantiation does not authorize editing away a marker the generic
needs. Unknown selections retain their conservative requirements. Preserve the
other marker, call syntax, ordinary obligations and all source behavior.

Analysis uses the source's declared checking context and required selections,
not an optimizer's predicted target or one convenient execution. Limit edits
to the requested writable source set; dependency stores are not editing targets.
Retain exact source versions and token spans. Preserve comments and unaffected
text, reject ambiguous or conflicting edits, and detect intervening file changes
before committing rather than overwriting another edit.

Construct the complete edited source set in staging and recheck it before any
original is changed. Failed analysis or rechecking writes no source files.
Commit the validated edits as one logical transaction: success means every
planned file was updated; a reported write failure restores the originals.
Interrupted writes require durable recovery, not a sequence of per-file renames
described as atomic. Recovery must complete or restore the edit set before
another rewrite can succeed. Unrelated filesystem readers may observe intermediate file replacements.

With `--check`, report the proposed edits without writing source files. No
required edit yields success; available edits yield a nonzero CI result.
Analysis/recheck failures remain distinguishable from an edit-needed result.
Without `--check`, report the applied edits after successful rechecking and
commit. The rule produces no default-path compiler warning and does not make
permitted redundant markers invalid.

## Checker conformance

`omega --check --floor-only <root.omg>` restricts automatic discharge to the
[required automatic discharge](../proofs/contracts.md#required-automatic-discharge)
and the [first synchronization floor](../language/concurrency.md#first-required-automatic-floor).
Cited lemmas, branches and contracts work as usual. Each obligation that the
ordinary run proves only with automation beyond the floor is reported with its
location, so a project can confirm that every conforming checker accepts its
sources. The option changes no accepted program's meaning and is not a Build
setting.

<a id="independent-stage-validators"></a>

## Independent stage validators and determinism checks

`--validator <ExactName>` requests a named stage-output check with the coverage
described below. Independent correspondence validators and producer replay
checks have different assurance; a replay is not independent validation.
The option runs the selected check inside this compilation. Repeat the option
to select multiple exact validators.
`--every-validator` enumerates the deterministic registered set for this compiler
and runs every applicable validator. It is a quantifier, not a curated assurance
profile. Combining it with named requests does not run the same validator twice;
each explicit request still requires applicability.

Selection adds checking work, not new source semantics or compiler authority.
It does not select extra compilation stages, emit a native artifact for
`--check`, relax admission, or replace required source/boundary checks.
Unknown names reject. A named validator whose stage is not produced or whose
target/product requirements are unavailable is an invocation error, even when
`--every-validator` was also requested.

For the enumerated set, report which validators ran and which were inapplicable,
with reasons. Report exact names, relevant target/stage and passed/failed
results in the existing compile report; `--report-file` retains those rows.
A skipped or inapplicable validator is never reported as passed. Registry changes
may change the enumerated set, so retain compiler identity and the actual roster.
A validator failure makes the command fail; continue to distinguish compiler
invariant failures from invalid authored source and unavailable checks.

`--verify` keeps its separate meaning: with `--check`, withhold retained package
trust acceptance and re-verify under retained source pins. It is not a spelling
for stage validation. Default compilation retains the existing
[verification placement](../../../source-rust/subsystems/compiler/README.md#verification-placement).
Full source-to-executable correspondence requires separate justification.

`FixedFrameCorrespondence` compares current fixed-frame output with its original
allocation, instruction encoding and baseline layout. The request retains those
inputs for that audit. Default compilation enforces instruction encoding, frame,
exit and machine-state constraints during realization. Independent checks of the final executable
image belong to [backend tests](machine_state_evidence.md#final-artifact-validation-placement)
outside the build's validator registry.

`SelectedInstructionRewriteReplay` rebuilds each selected-instruction rewrite
run step by step with the producer from that run's own input and requires
the allocated program's optimization record to name exactly that run. The request
retains the run for that audit; default compilation trusts the run and does not keep
it. This establishes producer repeatability and record custody only. It cannot
detect a deterministic incorrect rewrite and does not satisfy the independent
rewrite-validation requirement in
[optimizations](optimizations.md#atomic-candidates-and-independent-validation).
Its report names the determinism scope, including when it passes. With no
selected-instruction rewrite rule selected it is inapplicable.

## Required acceptance

Exercise redundant and required markers independently on both axes, generic
upper bounds, known synchronous selections, narrowing refinements, legal
positions, invariant windows and context floors. Missing markers and invalid
positions still reject; permitted extras create no effect or continuation.

Exercise rewrite preview, idempotence, exact-rule selection, retained comments,
generic templates, concurrent source changes, failed rechecks, failed multi-file
commits and interrupted-write recovery. A successful rewrite preserves execution
and ordinary contract acceptance.

Exercise both package audit forms, install/update resume/discard, both Psi
inspection forms and old-spelling rejection. Validator controls cover unknown
names, unsupported stages/targets, `--check` applicability, enumerated coverage,
report rows and failures. Source cleanup, marker admission and validator tooling
have separate implementations; none substitutes for the others.
