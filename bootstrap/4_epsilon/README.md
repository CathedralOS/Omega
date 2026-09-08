# Epsilon rung

This directory owns the Epsilon language, its Delta-written evaluator, and
adjacent execution validation.

[`LANGUAGE.md`](LANGUAGE.md) is the normative Epsilon v1 contract fixed by D17.
It is self-contained: a compiler, sample corpus, historical implementation, or
Omega document cannot amend Epsilon by acceptance.

D22, D36, and D51 fix deterministic declaration identity beneath that contract:
grammar-selected owner, machine, member, state, and local scopes; one pre-type
scoped duplicate census; no active local shadowing; legal disjoint-state and
member/local spelling reuse; free unqualified machines; mandatory receivers on
owner-qualified data machines; disjoint constructor and receiver-method
namespaces; and no authored machine bodies on boundary owners. D24 completes that census with transition-arm binder scope,
same-phase `DuplicateName`/`InvalidBoundary` ordering, and unique-owner
classification.
D31 completes type formation and D34 completes its realizability report:
positive array lengths, zero-field records, mixed-data rejection, exact
storage-only `u8` and `never`/view/`Console` placement, structural diagnostic
anchors, and profile-owned bounded-witness static-storage refusal.
D37 fixes body/control candidate dependency as a complete premise DAG,
including value/place/resultless/`never` joins and exact relational/projection
anchors. D52 fixes resultless machine/constructor arguments at their authored
argument-expression start and closes the distinct-reason coordinate audit.
D53 fixes five local block-exit effects, checks every state without reachability
analysis, and gives falloff and post-`never` constructs exact delimiter anchors.
D38 fixes `.as_slice` as a once-evaluated, allocation-free full view of a
place-valued fixed array; views and non-place array temporaries are not accepted
receivers.
D56 closes the fixed `Console`/`Main` entry taxonomy inside type formation:
absence of an authored `Main::main` name is the sole `MissingEntry`, every
present malformed or incomplete entry system is `InvalidEntry`, and no entry
candidate is compared with body/control coordinates.
D57 makes wildcard finality grammatical and orders transition-pattern checking
as subject admission, semantic duplicate identity, payload arity, then static
sum coverage. Scalar selectors compare by `i32` value, and missing sum coverage
anchors at the transition subject.

## Canonical edges

```text
Delta-written Epsilon evaluator + exact Epsilon-written Omega D
  └─ selected Gamma execution ─▶ interpreted Omega compiler D

Interpreted D + Omega-written C + alpha_bootstrap
  └─ omega0_compiler_bytecode.tape
```

The first executable composition runs Epsilon. D accepts Omega and produces the
first C tape only when its ordinary `alpha_bootstrap` target is selected.

## Implementation status

- [`FEATURE_LEDGER.md`](FEATURE_LEDGER.md) records Epsilon feature rationale and
  change control.
- [`epsilon_compiler.delta`](epsilon_compiler.delta) is the in-progress
  Delta-written evaluator's entry; `epsilon_compiler.delta.sources` orders the
  concept-owned authoring members into one exact Delta translation unit.
  The source currently
  contains final compiler material through complete parsing, the D22/D24
  source-shaped identity census including D51's receiver-only qualified-machine
  syntax and removal of the superseded case/machine collision registry, D31
  structural type formation including D56's final reserved entry-shape
  judgment, the source-backed resolution catalog, ordered local-value
  resolution, exact scalar and aggregate value/place facts, one generalized
  callable ledger, direct
  qualified applications, grouped/unqualified machine applications and
  discarded postfix-statement category admission in settled non-continuation
  contexts, ordinary named receiver binding for `self`, named-data
  receiver applications, exact sealed-`Console` receiver applications, and
  explicit transition state applications with state/machine collision
  rejection and separate state completion custody, transition subject,
  resolved-case, complete payload-binder, and retained sum-coverage facts,
  settled field/index/slice projection failures, D37 scalar and argument-
  `never` category joins, let/assignment/assert and explicit-return relations,
  first-following-statement terminal flow.
  D50 fixes bare-state-transfer spelling, D51 retires static qualified
  machines plus special `self` resolution, D52 fixes resultless-argument
  anchoring, D53 fixes local block exits, D56 fixes entry diagnostics, and D57
  fixes transition-pattern and coverage diagnostics. D50, D51, D52, D56, and
  D57 are implemented. The remaining rulings, final body/control checking,
  the remaining D37 control/terminal premise DAG, full resource conformance,
  storage realization, execution, `main`, and composition remain incomplete,
  so it exposes no final evaluator artifact yet. The
  current fact pass does enforce D38's contextual receiver/result relation and
  separate array-view extra-call rejection.
  The [execution status](#validation-and-completion) below describes the
  staging executor and its remaining conformance obligations.

Superseded bridge and native-publication experiments remain only in Git
history. No compatibility owner replaces them. A compact positive/negative
suite will be derived from D17 and owned by the real compiler edge.

## Boundaries

- Epsilon is independent of Omega even when spelling overlaps.
- The Epsilon evaluator is written in Delta and executes exact Epsilon source
  under the selected Gamma evaluator.
- Epsilon-written `D` implements full Omega and may slowly compile Omega C for
  the ordinary `alpha_bootstrap` target.
- All fixed capacities are source-visible bounds, explicit profile parameters,
  or private budgets whose exhaustion is `Incomplete` and publishes no tape.
- Shell and Python may invoke tests or stamp tapes. They may not parse, lower,
  manufacture semantic evidence, or become compiler stages.
- The Rust compiler remains a comparator, not a producer in the canonical
  sequence.

Active work is tracked in
[`../../TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).

## Retention inventory

| Retained child | Canonical role | Deletion condition |
| --- | --- | --- |
| `LANGUAGE.md` and `FEATURE_LEDGER.md` | Normative contract and feature/change rationale. | Replace only with synchronized contracts and customer gates. |
| `epsilon_compiler.delta` and `epsilon_compiler.delta.sources` | Evaluator entry and exact ordered Delta source closure. | Replace only atomically with an admitted immediate-predecessor evaluator edge. |
| Concept-owned source files and folders below | Checking, execution, and shared representations within that closure. | Replace while preserving the exact compiler customer and required conformance evidence. |

Proposed programs without a compiler or a contract-derived test role are not
retained as tests.

## Epsilon evaluator source guide

Start at [`epsilon_compiler.delta`](epsilon_compiler.delta). Its
`epsilon_evaluate_entry_slice` accepts source and sealed input, checks the source,
distinguishes rejection from internal failure, and starts the checked entry
invocation. This is a diagnostic
execution entrance, not yet the final evaluator `main` or a closed compiler edge.
The [private execution driver](../../tests/epsilon/interpreted-omega-experiment/README.md#private-execution-observations)
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
| Storage | [`execution/storage/homes.delta`](execution/storage/homes.delta) owns runtime roots, reads, writes, and reclamation. [`places.delta`](execution/storage/places.delta) walks projection paths; [`children.delta`](execution/storage/children.delta) selects record-field or indexed-array access. [`arrays.delta`](execution/storage/arrays.delta) validates array extents and delegates to [`arrays/lookup.delta`](execution/storage/arrays/lookup.delta) and [`arrays/updates.delta`](execution/storage/arrays/updates.delta). [`liveness.delta`](execution/storage/liveness.delta) retains backing roots needed by surviving state views. |
| Console | [`execution/console.delta`](execution/console.delta) sequences argument effects and selects the operation. [`console/input.delta`](execution/console/input.delta) advances sealed input; [`console/output.delta`](execution/console/output.delta) owns byte/line output and exit. |
| Runtime operations | [`execution/statements.delta`](execution/statements.delta) applies statements. `scalars/` and `control/` own scalar operations and block/state control. |
| Shared representations | [`representations/`](representations) groups syntax, parsing outcomes, checked facts, diagnostics, and execution values by concept. |

The 87 authoring members have at most 450 lines each; the root entrance has 26.
Files end at complete top-level Delta forms. They are not independent Delta
modules: they share one translation unit and the language gains no imports.

## Exact source closure

[`epsilon_compiler.delta.sources`](epsilon_compiler.delta.sources) is the
ordered source authority. Its `DeltaSourceClosureV1` rows bind a stable member
identity, byte length, SHA-256 digest, and relative path. Declaration order is
explicit. Directory enumeration never selects order; forward definitions share
the same translation unit.

[`source_closure.py`](../../tools/bootstrap/source_closure.py) checks those
rows, closed ASCII source bytes, canonical nonsymlink member paths, and exact
source inventory, then concatenates bytes without separators. It does not parse
or lower Delta. Bootstrap callers use `OMEGA_PATH_EPSILON_COMPILER_SOURCES`
from the shared role registry rather than reading the entrance as the full source.

The packed evaluator is 11,998 lines / 611,266 bytes, SHA-256
`566139b6c2e97d06d1c18297432ebe4453801be45d35c8c76385bda3ecde0ad8`.
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

Call checking reuses the newest resolved-callable fact only when its kind and
both span endpoints identify the current application. Children are checked
first; receiver and unqualified callable resolution then prepend that exact
fact. A resolved callable head has no value result to find, so this case avoids
searching the accumulated whole-source expression ledger. Other heads retain
the original value-category lookup and diagnostic ordering. This adds no index,
cache, representation, or language/profile change. The
[checking controls](../../tests/epsilon/checking/README.md) cover nested
same-start calls, grouped/local value heads, and independent argument errors.

From the repository root:

```sh
sh tests/bootstrap/source-closure.sh
sh tools/bootstrap/check-chain-hygiene.sh
sh tests/epsilon/checking/run.sh
sh tests/epsilon/checking-invariants/run.sh
sh tests/epsilon/runtime-references/run.sh
sh tests/epsilon/runtime-invariants/run.sh
sh tests/epsilon/array-storage/run.sh
sh tests/epsilon/source-views/run.sh
sh tests/epsilon/interpreted-omega-experiment/run.sh
sh tests/delta/staged-compiler/run.sh
```

[Implementation notes](implementation_notes.md) describe the supported checking
and execution slices and their conformance controls.
The staging executor supports unqualified calls, value/resultless returns,
effect-threaded expressions, record/fixed-array/sum value copies, nested receiver
places, and independent recursive invocation homes. Runtime places distinguish
instances by root identifier and checked field/index path. Scope exit releases
local roots without reusing their identifiers, retaining backing roots still
referenced by views passed to the next state. Views retain literal, live-place,
or existing snapshot backing and expose no assignable place; strings, range
slices, `.as_slice`, indexing, lengths, and all four
Console operations execute in this staging path. Sparse typed zero homes avoid
eager array allocation; they do not establish the final application's physical
storage profile. Record fields retain exact checked identities in a sparse list;
array children use an immutable interval tree keyed by element index. Reads and
updates traverse at most 31 partitions, independent of the number of populated
elements, and updates share untouched branches. Parent reconstruction has its
own stack, separate from either child representation. The
[array-storage gate](../../tests/epsilon/array-storage/README.md) covers an
ordinary fill/read workload and private malformed-path controls.
Sum constructors, first-case zero defaults, checked case
transitions, and copied payload binders have staging execution paths. The
[immediate payload establishment rule](LANGUAGE.md#epsilon-constructor-payload-establishment-order)
traps as `ByteRange` at a failing byte payload before later arguments run,
preserving earlier output. This is not full evaluator completion. Remaining
conformance obligations and final composition with D remain open.
The [runtime-invariant controls](../../tests/epsilon/runtime-invariants/README.md)
exercise internal-failure defenses with synthetic state that bypasses checking;
they do not claim additional admitted Epsilon behavior or final publication.
The [source-window controls](../../tests/epsilon/source-views/README.md) compare
raw and bounded source routes, source-relative diagnostics, validated extents,
and failure before publication for invalid internal source indexes.
[LANGUAGE.md](LANGUAGE.md) governs semantics;
[TASKS_BOOTSTRAP.md](../../TASKS_BOOTSTRAP.md) owns remaining work.
