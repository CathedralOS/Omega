# Epsilon evaluator source guide

Start at [`epsilon_compiler.delta`](epsilon_compiler.delta). Its
`epsilon_evaluate_entry_slice` accepts source and sealed input, checks the source,
distinguishes rejection from internal failure, and starts the checked entry
invocation. This is a diagnostic
execution entrance, not yet the final evaluator `main` or a closed compiler edge.

## Follow the program

| Area | Entrance and ownership |
| --- | --- |
| Checking | [`checking/check.delta`](checking/check.delta) sequences parsing and checking. Its subfolders own declarations, types, catalogs, resolution, calls, expressions, and control judgments. |
| Lexical validation | [`lexical/validation.delta`](lexical/validation.delta) validates source bytes and lexical forms; [`tokens.delta`](lexical/tokens.delta) provides syntax-token lookahead. |
| Parsing | [`parsing/machine_declarations.delta`](parsing/machine_declarations.delta) assembles the program from declarations. Sibling files own expressions, transitions, statements, blocks, and data declarations. |
| Execution | [`execution/invocation.delta`](execution/invocation.delta) selects the entry and resumes states. [`control/blocks.delta`](execution/control/blocks.delta) coordinates statements and terminal control. |
| Calls | [`execution/calls.delta`](execution/calls.delta) dispatches checked callable identities. [`calls/receivers.delta`](execution/calls/receivers.delta) selects the receiver place; [`arguments.delta`](execution/arguments.delta) captures value arguments; [`calls/parameters.delta`](execution/calls/parameters.delta) installs and releases callee homes. |
| Expressions | [`execution/expressions.delta`](execution/expressions.delta) dispatches expression forms and carries effects into scalar operations, calls, and storage access. |
| Sums | [`execution/sums/construction.delta`](execution/sums/construction.delta) captures checked constructor payloads; [`defaults.delta`](execution/sums/defaults.delta) normalizes lazy zero sums. [`transitions.delta`](execution/sums/transitions.delta) selects checked cases; [`bindings.delta`](execution/sums/bindings.delta) establishes independent arm-local payload homes. |
| Projections | [`execution/projections/fields.delta`](execution/projections/fields.delta) selects checked record fields and contextual array/view members; [`indexes.delta`](execution/projections/indexes.delta) evaluates indexes and checks bounds before access. |
| Views | [`execution/views/slices.delta`](execution/views/slices.delta) sequences base and bound evaluation. [`backing.delta`](execution/views/backing.delta) reads ultimate backing and implements place-only `.as_slice`; [`strings.delta`](execution/views/strings.delta) decodes literal bytes. |
| Storage | [`execution/storage/homes.delta`](execution/storage/homes.delta) owns runtime roots, reads, writes, and reclamation. [`places.delta`](execution/storage/places.delta) walks projection paths; [`liveness.delta`](execution/storage/liveness.delta) retains backing roots needed by surviving state views. Sibling files own immutable values, sparse children, and local bindings. |
| Console | [`execution/console.delta`](execution/console.delta) sequences argument effects and selects the operation. [`console/input.delta`](execution/console/input.delta) advances sealed input; [`console/output.delta`](execution/console/output.delta) owns byte/line output and exit. |
| Runtime operations | [`execution/statements.delta`](execution/statements.delta) applies statements. `scalars/` and `control/` own scalar operations and block/state control. |
| Shared representations | [`representations/`](representations/) groups syntax, parsing outcomes, checked facts, diagnostics, and execution values by concept. |

The 75 authoring members have at most 450 lines each; the root entrance has 22.
Files end at complete top-level Delta forms. They are not independent Delta
modules: they share one translation unit and the language gains no imports.

## Exact source closure

[`epsilon_compiler.delta.sources`](epsilon_compiler.delta.sources) is the
ordered source authority. Its `DeltaSourceClosureV1` rows bind a stable member
identity, byte length, SHA-256 digest, and relative path. Declaration order is
explicit. Directory enumeration never selects order; forward definitions share
the same translation unit.

[`source_closure.py`](../../../tools/bootstrap/source_closure.py) checks those
rows, closed ASCII source bytes, canonical nonsymlink member paths, and exact
source inventory, then concatenates bytes without separators. It does not parse
or lower Delta. Bootstrap callers use `OMEGA_PATH_EPSILON_COMPILER_SOURCES`
from the shared role registry rather than reading the entrance as the full source.

The packed evaluator is 11,188 lines / 564,044 bytes, SHA-256
`444aa8c6eb7392fa07c84fda68e9deecb875aa5356b2a6ac284f2036589a0b38`.
When editing a member, update its manifest length and digest; change membership
explicitly when adding or removing source. Update exact test identities only
after reviewing the semantic change and its generated receipt.

## Validation and completion

From the repository root:

```sh
sh tests/bootstrap/source-closure.sh
sh tools/bootstrap/check-chain-hygiene.sh
sh tests/epsilon/delta-boundary-experiment/run.sh
sh tests/epsilon/interpreted-omega-experiment/run.sh
sh tests/delta/staged-compiler/run.sh
```

[Implementation notes](implementation_notes.md) describe the supported checking
and execution slices and their conformance controls. The staging executor carries
record, fixed-array, and sum value copies, nested receiver places, and separate
invocation-local homes. Views retain literal, live-place, or existing snapshot
backing; strings, range slices, `.as_slice`, indexing, lengths, and all four
Console operations execute in this staging path. Sparse typed zero homes avoid
eager array allocation; they do not establish the final application's physical
storage profile. Sum constructors, first-case zero defaults, checked case
transitions, and copied payload binders have staging execution paths. A failing
`u8` payload argument followed by another argument remains `Unsupported` until
[Epsilon constructor payload establishment order](../../../OWNER_QUESTIONS.md#epsilon-constructor-payload-establishment-order)
is settled; final-argument `ByteRange` already has an execution path. This is
not full sum or evaluator completion. Remaining conformance obligations and
final composition with D remain open.
[LANGUAGE.md](../LANGUAGE.md) governs semantics;
[TASKS_BOOTSTRAP.md](../../../TASKS_BOOTSTRAP.md) owns remaining work.
