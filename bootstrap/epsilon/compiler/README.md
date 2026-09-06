# Epsilon evaluator source guide

Start at [`epsilon_compiler.delta`](epsilon_compiler.delta). Its
`epsilon_evaluate_entry_slice` accepts source and sealed input, checks the source,
distinguishes rejection from internal failure, and starts the checked entry
invocation. This is a diagnostic
execution entrance, not yet the final evaluator `main` or a closed compiler edge.
The [private execution driver](../../../tests/epsilon/interpreted-omega-experiment/README.md#private-execution-observations)
preserves full `i32` exit codes, trap kinds and stdout prefixes, and rejection
reasons and coordinates in distinct tagged results. Its transport is not a new
normative Epsilon request or observation envelope.

## Follow the program

| Area | Entrance and ownership |
| --- | --- |
| Compiler source | [`source.delta`](source.delta) validates byte windows and owns source-relative reads. [`representations/source.delta`](representations/source.delta) defines their private representation; runtime views and Console bytes are separate. |
| Checking | [`checking/check.delta`](checking/check.delta) sequences parsing and checking. Its subfolders own declarations, types, catalogs, resolution, calls, expressions, and control judgments. |
| Lexical validation | [`lexical/validation.delta`](lexical/validation.delta) validates source bytes and lexical forms; [`tokens.delta`](lexical/tokens.delta) provides syntax-token lookahead. |
| Parsing | [`parsing/machine_declarations.delta`](parsing/machine_declarations.delta) assembles the program from declarations. Sibling files own expressions, transitions, statements, blocks, and data declarations. |
| Execution | [`execution/invocation.delta`](execution/invocation.delta) selects the entry and resumes states. [`control/blocks.delta`](execution/control/blocks.delta) coordinates statements and terminal control. |
| Calls | [`execution/calls.delta`](execution/calls.delta) dispatches checked callable identities. [`calls/receivers.delta`](execution/calls/receivers.delta) selects the receiver place; [`arguments.delta`](execution/arguments.delta) captures value arguments; [`calls/parameters.delta`](execution/calls/parameters.delta) installs and releases callee homes. |
| Expressions | [`execution/expressions.delta`](execution/expressions.delta) dispatches expression forms and carries effects into scalar operations, calls, and storage access. |
| Sums | [`execution/sums/construction.delta`](execution/sums/construction.delta) captures checked constructor payloads; [`defaults.delta`](execution/sums/defaults.delta) normalizes lazy zero sums. [`transitions.delta`](execution/sums/transitions.delta) selects checked cases; [`bindings.delta`](execution/sums/bindings.delta) establishes independent arm-local payload homes. |
| Runtime references | [`execution/references.delta`](execution/references.delta) prepares the shared index and complete linear fallback. [`rows.delta`](execution/references/rows.delta) sequences expression ledgers; [`control_rows.delta`](execution/references/control_rows.delta) selects state, subject, and completed-pattern records. [`construction.delta`](execution/references/construction.delta) owns interval insertion, [`buckets.delta`](execution/references/buckets.delta) preserves typed ledger order, and [`lookup.delta`](execution/references/lookup.delta) delegates exact queries. |
| Projections | [`execution/projections/fields.delta`](execution/projections/fields.delta) selects checked record fields and contextual array/view members; [`indexes.delta`](execution/projections/indexes.delta) evaluates indexes and checks bounds before access. |
| Views | [`execution/views/slices.delta`](execution/views/slices.delta) sequences base and bound evaluation. [`backing.delta`](execution/views/backing.delta) reads ultimate backing and implements place-only `.as_slice`; [`strings.delta`](execution/views/strings.delta) decodes literal bytes. |
| Storage | [`execution/storage/homes.delta`](execution/storage/homes.delta) owns runtime roots, reads, writes, and reclamation. [`places.delta`](execution/storage/places.delta) walks projection paths; [`liveness.delta`](execution/storage/liveness.delta) retains backing roots needed by surviving state views. Sibling files own immutable values, sparse children, and local bindings. |
| Console | [`execution/console.delta`](execution/console.delta) sequences argument effects and selects the operation. [`console/input.delta`](execution/console/input.delta) advances sealed input; [`console/output.delta`](execution/console/output.delta) owns byte/line output and exit. |
| Runtime operations | [`execution/statements.delta`](execution/statements.delta) applies statements. `scalars/` and `control/` own scalar operations and block/state control. |
| Shared representations | [`representations/`](representations/) groups syntax, parsing outcomes, checked facts, diagnostics, and execution values by concept. |

The 84 authoring members have at most 450 lines each; the root entrance has 26.
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

The packed evaluator is 11,809 lines / 601,955 bytes, SHA-256
`6d372762df794f5f4725d050a986036e8bd9c7e1d1e8978d56011808e4206aed`.
When editing a member, update its manifest length and digest; change membership
explicitly when adding or removing source. Update exact test identities only
after reviewing the semantic change and its generated receipt.

Compiler source is a bounded view over existing Delta `Bytes`, not a newly
constructed byte tree. `epsilon_source_view` admits the origin before subtracting
it from the backing length, then checks the window extent. `epsilon_source_byte`
requires a factory-established view and checks its relative index before adding
the origin. Invalid reads retain ordinary Delta `Bytes` failure; they never
substitute a dummy byte or expose the private header or sealed stdin. No new
Delta or Gamma primitive is involved.

Raw-source checking and evaluation entrances wrap the complete byte sequence;
their view-taking counterparts share the same checking and execution judgments.
The framed driver selects only its source window. Syntax, checked facts, and
diagnostics keep source-relative offsets, independent of the backing origin.
Invocation contexts retain that view for name lookup and string decoding, while
Console input, output, decoded literals, and Epsilon runtime view backing remain
ordinary `Bytes`. The driver still constructs its sealed stdin separately.

Runtime reference lookup uses a derivative index, not a replacement checked
program. Entry builds an immutable interval tree over source-start coordinates.
Each branch retains its canonical midpoint, calculated and checked during
construction. Lookup validates that the saved split lies inside the current
interval and descends without recomputing midpoint division.
Each leaf retains separate ordered local, field, callable, state-application,
transition-subject, and completed-pattern ledgers. Original lookup helpers still
own exact kind/span matching and progress precedence. Callable grouping is
normalized before bucket selection; other queries retain their exact targets.
State rows preserve both Complete and Resolved records, while only Complete
pattern records enter the index. Missing completion cannot become executable.
All invocations share the tree, never their local values or roots. Invalid build
premises retain all six original query ledgers, including incomplete pattern
facts, in a linear fallback. Coverage remains in the unchanged checking ledger
and has no runtime query. Construction allocates logarithmic paths in the
existing Gamma pair arena; it neither enlarges that profile nor establishes
Epsilon's final physical storage bounds. A cached split adds one integer payload
to each branch; it does not add a separate tree level or change leaf identity.
Current Delta constructor lowering represents that payload with one additional
Gamma pair per branch reconstruction, including paths superseded by later inserts.

## Validation and completion

From the repository root:

```sh
sh tests/bootstrap/source-closure.sh
sh tools/bootstrap/check-chain-hygiene.sh
sh tests/epsilon/delta-boundary-experiment/run.sh
sh tests/epsilon/checking/run.sh
sh tests/epsilon/checking-invariants/run.sh
sh tests/epsilon/runtime-references/run.sh
sh tests/epsilon/runtime-invariants/run.sh
sh tests/epsilon/source-views/run.sh
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
transitions, and copied payload binders have staging execution paths. The
[immediate payload establishment rule](../LANGUAGE.md#epsilon-constructor-payload-establishment-order)
traps as `ByteRange` at a failing byte payload before later arguments run,
preserving earlier output. This is not full evaluator completion. Remaining
conformance obligations and final composition with D remain open.
The [runtime-invariant controls](../../../tests/epsilon/runtime-invariants/README.md)
exercise internal-failure defenses with synthetic state that bypasses checking;
they do not claim additional admitted Epsilon behavior or final publication.
The [source-window controls](../../../tests/epsilon/source-views/README.md) compare
raw and bounded source routes, source-relative diagnostics, validated extents,
and failure before publication for invalid internal source indexes.
[LANGUAGE.md](../LANGUAGE.md) governs semantics;
[TASKS_BOOTSTRAP.md](../../../TASKS_BOOTSTRAP.md) owns remaining work.
