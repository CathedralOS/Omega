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
| Runtime operations | [`execution/statements.delta`](execution/statements.delta) applies statements; [`console.delta`](execution/console.delta) handles supported Console calls. `scalars/`, `storage/`, and `control/` own their respective operations. |
| Shared representations | [`representations/`](representations/) groups syntax, parsing outcomes, checked facts, diagnostics, and execution values by concept. |

The 56 authoring members have at most 450 lines each; the root entrance has 22.
Files end at complete top-level Delta forms. They are not independent Delta
modules: they share one translation unit and the language gains no imports.

## Exact source closure

[`epsilon_compiler.delta.sources`](epsilon_compiler.delta.sources) is the
ordered source authority. Its `DeltaSourceClosureV1` rows bind a stable member
identity, byte length, SHA-256 digest, and relative path. Declaration order is
explicit; the root entrance is last. Directory enumeration never selects order.

[`source_closure.py`](../../../tools/bootstrap/source_closure.py) checks those
rows, closed ASCII source bytes, canonical nonsymlink member paths, and exact
source inventory, then concatenates bytes without separators. It does not parse
or lower Delta. Bootstrap callers use `OMEGA_PATH_EPSILON_COMPILER_SOURCES`
from the shared role registry rather than reading the entrance as the full source.

The packed evaluator is 9,927 lines / 497,563 bytes, SHA-256
`fedd2c1ad0934bac9970d8bbc02959d7cc926af215734889d6621c8377ba93a0`.
The partition preserves the preceding single-file source byte for byte.
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
and execution slices and their conformance controls. General machine calls,
aggregate execution, views, full Console behavior, and final composition with D
remain open. [LANGUAGE.md](../LANGUAGE.md) governs semantics;
[TASKS_BOOTSTRAP.md](../../../TASKS_BOOTSTRAP.md) owns remaining work.
