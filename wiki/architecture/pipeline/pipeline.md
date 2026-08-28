# Pipeline Architecture

Omega's compiler pipeline is a sequence of durable representation boundaries.
Each stage should have one primary job, one input representation, and one output
representation.

The target architecture has one named language/realization boundary. Psi owns
Omega-file parsing and every target-neutral stage through immutable terminal
Psi. Omega consumes terminal Psi and owns installation, optimization, ABI and
storage realization, target operations, and native artifacts. See
[Terminal Psi Architecture](terminal_psi.md). The stage list and matrix below
describe the one production path. Unsupported Terminal-Psi vocabulary rejects;
the compiler does not retain a second source-shaped backend as a fallback.

The same semantic nouns should be recognizable across stages, but their data
shape changes as they become more resolved. Source-shaped IR can only say "this
syntax looks like a place." Checked IR can say "this place overlaps this loan."
Backend IR can say "this place is stack slot plus offset."

## Stage Questions

Every stage document should answer:

- Input representation.
- Output representation.
- Primary responsibility.
- Places, values, facts, loans, moves, drops, calls, transitions, reach, and
  boundary edges.
- What this stage must not own.
- Known gaps.

## Semantic Spine

- Places: location-like expressions that can be read, written, borrowed, moved,
  or invalidated.
- Values: produced runtime or compile-time objects with type, initialization,
  ownership, and storage/lowering consequences.
- Facts: proven or accepted assertions at a program point.
- Loans: active borrows over places or views.
- Moves: ownership transfers that may make a source unusable.
- Drops: lifetime-ending cleanup events.
- Calls: invocations of machines, states, operators, helpers, or imported
  boundary entries.
- Transitions: control and argument transfers between states or exits.
- Reach: externally visible service classes such as allocation, IO, process
  exit, or host interaction.
- Boundary edges: points where Omega accepts a declared contract from
  compiler/runtime/host/toolchain code.

## Ownership Rule

The stage that first creates durable, queryable data for a noun owns that noun's
semantic meaning. Later stages preserve, refine, schedule, lower, encode, or
report that data. Earlier stages may parse or carry syntax for the noun, but
they should not make semantic decisions about it.

Use this rule when a pass starts to sprawl:

- If it discovers identity, it belongs near symbol resolution.
- If it decides type/signature compatibility, it belongs near typing.
- If it proves obligations, records facts, creates loans, or validates reach,
  it belongs near checked trees.
- If it chooses storage, ABI, instruction, relocation, or image form, it belongs
  in the backend lowering stages.

## Cross-Stage Compiler Projections

Some compiler-owned consumers are total observations assembled after successful
checking rather than transformation stages. Package admission is the principal
example. Its projector reads each fact from the earliest existing representation
where that fact is semantically complete, then joins structural identity to any
later checked acceptance, effect, proof, or realization evidence it requires.
"Earliest" does not mean unchecked or merely convenient: unresolved syntax and
diagnostic renderings are never admission evidence.

No single IR must contain the complete package report. The versioned canonical
projection is the compiler/package boundary; the representations and handles
used to derive it stay compiler-private and may evolve with the compiler. A
downstream Psi stage may repeat an invariant as a backstop without becoming the
mandatory place from which the projector reconstructs an already-settled fact.

Do not introduce a nominal `Chi` stage merely to collect these queries or give
them an internally stable interface. Add a named representation only when work
discovers a reusable semantic invariant boundary with its own consumers or
transformations. Prefer an existing coherent representation, including `Exact`,
when it already carries the required meaning.

## Operational Artifact Emission

Pipeline viewers and diagnostic reports are observations of a compilation, not
semantic gates. `ArtifactEmissionPolicy::OutputOnly` therefore suppresses the
HTML, JSON, text, Markdown, timing, and disassembly bundle used for interactive
inspection while preserving every fail-closed validation that normally feeds
those reports. Wire compatibility demands, capability-ledger checks, trust
consistency checks, trust-lock enforcement, and final executable-footprint
certification still run. If output installation is requested, the primary
object or executable and any installation records required by its semantics
still exist; otherwise an output-only check need not create a build directory.
Boundary reporting captures its ordered source target, contract, and policy
rows once, writes that initial observation at the source boundary, and later
consumes the same carrier when checked capability facts become available. It
does not rebuild those rows from a retained syntax-tree clone, and checked
capability settlement remains validating under report suppression.
Backend reporting likewise captures one checked-surface observation only for a
full native compilation and consumes it once the corresponding backend plan is
available. Suppressed and non-native products retain canonical absence; the
pipeline driver neither couriers a raw optional report surface nor owns its
conditional publication policy.

Production entry points retain full artifacts by default. Corpus schedulers may
select output-only mode for independent pass/fail compiles whose assertions are
the diagnostics, checked result, or installed primary output. Tests that inspect
a report continue to use full emission. This keeps observability selectable at
the orchestration boundary without turning report generation into language
semantics or duplicating policy through every representation stage.

`RequestedCompileProduct::NativeArtifact` is a distinct stopping boundary. It
runs the Psi-owned checked frontend and canonical Terminal producer, then gives
that complete artifact to the source-free native realization path
shared with component staging. It returns exactly one non-clonable payload
owning the canonical Terminal identity, checked target, selected-provider
projection, object and relocation evidence, encoded text, and independently
replayed final executable image. Its report has no executable path,
publication, installation, terminal deployment, or runtime authority and
records that no primary output was written. Unsupported Terminal vocabulary
rejects at this boundary without legacy fallback, and pending component
progress rejects rather than being discarded. Auxiliary observations remain
controlled independently by `ArtifactEmissionPolicy`; with `OutputOnly`,
retained-artifact compilation creates no build directory.

## Representation Root Shape

Durable representations should make their semantic spine visible at the root.
When a representation has both executable/data shape and preserved semantic
evidence, those should be separate named roots rather than a flat bag of arenas.

Current preferred shapes:

- Source-shaped representations use `roots` plus `tables` when identity and
  contiguous storage are the main concerns, for example `TypedTrees`.
- Checked representations use a source/program root plus a facts root, for
  example `CheckedTrees { typed, facts }`.
- Omega operation and artifact representations use a code/shape root plus the
  retained evidence needed to replay the next boundary. Orchestration passes
  complete typed artifacts between stages rather than rebuilding semantic facts
  from source-shaped trees.

This is not ceremony. It makes it obvious whether a pass is changing executable
shape, preserving semantic evidence, or doing both. If a stage starts reaching
through unrelated roots to answer a question, that is a sign the query belongs
behind a unified view or helper instead of being reconstructed ad hoc.

Concrete field layout for backend-ABI carriers is a separate concern from these
representation roots. Fat-descriptor field layout (for slices and text windows,
the shared `{ ptr, len }` carrier) is a backend-ABI concern owned at the
runtime-abi boundary, not redefined by later lowering. Owned and borrowed
carriers share that layout and differ only by an ownership tag in the semantic
spine. Layout and instruction-selection stages consume the descriptor shape
through its owner rather than re-deriving offsets and sizes.

## Semantic Ownership Matrix

This table is intentionally blunt. Each cell says the main relationship between
the stage and the noun: `none`, `syntax`, `identity`, `typed`, `checked`,
`scheduled`, `lowered`, `assigned`, `encoded`, `artifact`, `metadata`, or
`final`.

| Stage | Places | Values | Facts | Loans | Moves | Drops | Calls | Transitions | Reach | Boundaries |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Source Files To Tokens | none | none | none | none | none | none | none | none | none | token |
| Tokens To Syntax Trees | syntax | syntax | syntax | none | none | none | syntax | syntax | syntax | syntax |
| Syntax Trees To Symbol Resolved Trees | identity | identity | identity | none | none | none | identity | identity | identity | identity |
| Symbol Resolved Trees To Typed Trees | typed | typed | typed | type surface | planned | planned | typed | typed | typed | typed |
| Typed Trees To Checked Trees | checked | checked | checked | checked | checked | checked | checked | checked | checked | checked |
| Checked Trees To Terminal Psi | lowered | lowered | preserved | lowered | lowered | lowered | lowered | lowered | preserved | lowered |
| Terminal Psi To Abstract Operations | lowered | lowered | metadata | assertion | abstract op | abstract op | abstract op | abstract op | op metadata | metadata |
| Optimization Run To Abstract Operations | projected | projected | validated metadata | assertion | projected op | projected op | projected op | projected op | projected metadata | validated metadata |
| Abstract Operations To Target Operations | target | target | metadata | assertion | target op | target op | target op | target op | target op | target metadata |
| Target Operations To Selected Instructions | selected | selected | metadata | none | selected | selected | selected | selected | selected | selected metadata |
| Selected Instructions Through Allocation | assigned | assigned | metadata | none | assigned | assigned | assigned | assigned | assigned | assigned metadata |
| Target Operations To Assigned Target Operations | assigned | assigned | metadata | none | assigned | assigned | assigned | assigned | assigned | assigned metadata |
| Assigned Operations To Machine Code | encoded | encoded | metadata | none | metadata | metadata | encoded call bytes | encoded branch bytes | encoded bytes | encoded metadata |
| Machine Code To Native Artifact | final | final | final layout | none | metadata | metadata | final import/fixup | final branch fixup | final artifact | final metadata |

Current deliberate gaps:

- Terminal Psi is the sole Psi/Omega boundary. Unsupported aggregate, cleanup,
  transfer, boundary, loop, suspension, and ordering slices reject until their
  canonical lowering exists; they do not revive a tree-consuming backend.

- Moves and drops now have durable checked/control-flow event plumbing, but
  event production still needs type-aware precision plus transition and nested
  call coverage.
- Checked values preserve through Terminal Psi, abstract operations, target
  operations, physical assignment, machine code, and native artifacts. Missing
  semantic slices fail at their owning stage.

## Stages

- [Terminal Psi target architecture and migration](terminal_psi.md)

- [Source Files To Tokens](stages/source_files_to_tokens.md)
- [Tokens To Syntax Trees](stages/tokens_to_syntax_trees.md)
- [Syntax Trees To Symbol Resolved Trees](stages/syntax_trees_to_symbol_resolved_trees.md)
- [Symbol Resolved Trees To Typed Trees](stages/symbol_resolved_trees_to_typed_trees.md)
- [Typed Trees To Checked Trees](stages/typed_trees_to_checked_trees.md)
- [Optimization Run To Abstract Operations](stages/optimization_run_to_abstract_operations.md)
- [Abstract Operations To Target Operations](stages/abstract_operations_to_target_operations.md)
- [Target Operations To Selected Instructions](stages/target_operations_to_selected_instructions.md)
- [Selected Instructions To Liveness](stages/selected_instructions_to_liveness.md)
- [Liveness To Live Ranges](stages/liveness_to_live_ranges.md)
- [Live Ranges To Allocation Legality](stages/live_ranges_to_allocation_legality.md)
- [Allocation Legality To Fixed-View Copies](stages/allocation_legality_to_fixed_view_copies.md)
- [Fixed-View Copies To Reanalyzed Legality](stages/fixed_view_copies_to_reanalyzed_legality.md)
- [Allocation Legality To Register Homes](stages/allocation_legality_to_register_homes.md)
- [Target Operations To Assigned Target Operations](stages/target_operations_to_assigned_target_operations.md)
