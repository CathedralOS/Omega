# Epsilon evaluator source guide

Start at [`epsilon_compiler.delta`](epsilon_compiler.delta). Its
`epsilon_evaluate_entry_slice` checks the source, distinguishes rejection from
internal failure, and starts the checked entry invocation. This is a diagnostic
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
| Projections | [`execution/projections/fields.delta`](execution/projections/fields.delta) selects checked record fields and array lengths; [`indexes.delta`](execution/projections/indexes.delta) evaluates array indexes and checks bounds before access. |
| Storage | [`execution/storage/homes.delta`](execution/storage/homes.delta) owns runtime roots, reads, writes, and reclamation. [`places.delta`](execution/storage/places.delta) walks projection paths; sibling files own immutable values, sparse children, and local bindings. |
| Runtime operations | [`execution/statements.delta`](execution/statements.delta) applies statements; [`console.delta`](execution/console.delta) handles supported Console calls. `scalars/` and `control/` own scalar operations and block/state control. |
| Shared representations | [`representations/`](representations/) groups syntax, parsing outcomes, checked facts, diagnostics, and execution values by concept. |

The 65 authoring members have at most 450 lines each; the root entrance has 22.
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

The packed evaluator is 10,321 lines / 517,029 bytes, SHA-256
`56e954e09326f53c9ee22fabd2f79823cbd01db0072c6d39a12bf4f51a49e07e`.
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
record and fixed-array value copies, nested receiver places, and separate
invocation-local homes. Sparse typed zero homes avoid eager array allocation;
they do not establish the final application's physical storage profile. Sum and
view execution, full Console behavior, and final composition with D remain open.
[LANGUAGE.md](../LANGUAGE.md) governs semantics;
[TASKS_BOOTSTRAP.md](../../../TASKS_BOOTSTRAP.md) owns remaining work.
