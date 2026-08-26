# Pipeline Architecture

Omega's compiler pipeline is a sequence of durable representation boundaries.
Each stage should have one primary job, one input representation, and one output
representation.

The target architecture has one named language/realization boundary. Psi owns
Omega-file parsing and every target-neutral stage through immutable terminal
Psi. Omega consumes terminal Psi and owns installation, optimization, ABI and
storage realization, target operations, and native artifacts. See
[Terminal Psi Architecture](terminal_psi.md). The stage list and matrix below
describe both the terminal lane and the bootstrap paths that remain while
unsupported slices migrate. They are not a commitment to preserve
`StateGraph` and `ControlFlowPlan` as public representations.

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
- If it schedules already-checked events into graph/control form, it belongs in
  graph or control-flow lowering.
- If it chooses storage, ABI, instruction, relocation, or image form, it belongs
  in the backend lowering stages.

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

Production entry points retain full artifacts by default. Corpus schedulers may
select output-only mode for independent pass/fail compiles whose assertions are
the diagnostics, checked result, or installed primary output. Tests that inspect
a report continue to use full emission. This keeps observability selectable at
the orchestration boundary without turning report generation into language
semantics or duplicating policy through every representation stage.

## Representation Root Shape

Durable representations should make their semantic spine visible at the root.
When a representation has both executable/data shape and preserved semantic
evidence, those should be separate named roots rather than a flat bag of arenas.

Current preferred shapes:

- Source-shaped representations use `roots` plus `tables` when identity and
  contiguous storage are the main concerns, for example `TypedTrees`.
- Checked representations use a source/program root plus a facts root, for
  example `CheckedTrees { typed, facts }`.
- Graph/control/backend representations use a code/shape root plus a semantic
  evidence root, for example `StateGraph { code, semantics }`,
  `ControlFlowPlan { code, semantics }`, and backend operation plans.
- Aggregate backend artifacts use their own artifact root, for example
  `BackendArtifactRoots`, and orchestration should construct empty artifact
  spines or explicit artifact bundles through that root instead of
  hand-assembling machine, object, or relocation internals. Artifact roots
  should expose semantic-summary accessors when the evidence lives beside
  physical artifacts rather than inside each physical artifact.

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
| Checked Trees To State Graph | scheduled | scheduled | scheduled | scheduled | scheduled | scheduled | scheduled | graph | scheduled | scheduled |
| State Graph To Control Flow | lowered | lowered | preserved | preserved | lowered | lowered | control flow | control flow | preserved | control flow |
| Control Flow To Abstract Operations | lowered | lowered | metadata | assertion | abstract op | abstract op | abstract op | abstract op | op metadata | metadata |
| Abstract Operations To Target Operations | target | target | metadata | assertion | target op | target op | target op | target op | target op | target metadata |
| Target Operations To Assigned Target Operations | assigned | assigned | metadata | none | assigned | assigned | assigned | assigned | assigned | assigned metadata |
| Target Operations To Machine Program | artifact | artifact | metadata | none | artifact | artifact | artifact | artifact | artifact | artifact metadata |
| Assigned Target Operations To Machine Instructions | encoded | encoded | metadata | none | instruction | instruction | instruction | instruction | instruction | instruction metadata |
| Machine Instructions To Machine Bytes | encoded | encoded | metadata | none | metadata | metadata | encoded call bytes | encoded branch bytes | encoded bytes | encoded metadata |
| Machine Bytes To Object Plan | artifact | artifact | metadata | none | metadata | metadata | symbol metadata | section metadata | artifact | sibling metadata |
| Object Plan To Relocations | artifact | artifact | metadata | none | metadata | metadata | relocation records | relocation records | artifact | sibling metadata |
| Object Relocations To Final Image | final | final | final layout | none | metadata | metadata | final import/fixup | final branch fixup | final artifact | final metadata |

Current deliberate gaps:

- Terminal Psi is the expression-lowering boundary for the migrated scalar,
  control, call, crash, contract, and content-conservation slices. Unsupported
  aggregate, cleanup, transfer, boundary, loop, suspension, and ordering slices
  still use the bootstrap `StateGraph`/`ControlFlowPlan` path, whose typed
  expression references prevent it from becoming a portable boundary. Each
  completed terminal slice retires its corresponding tree consumer.

- Moves and drops now have durable checked/control-flow event plumbing, but
  event production still needs type-aware precision plus transition and nested
  call coverage.
- Checked values now preserve through state graph, control flow, abstract
  operations, target operations, assigned target operations, symbolic machine
  instructions, encoded machine bytes, and the current machine-program
  artifact, but still need
  type-aware ownership kind, drop policy, storage consequences, and backend
  lowering beyond metadata.
- Source-level boundary trait calls now preserve as checked, graph,
  control-flow, and abstract source boundary edges. Abstract/backend boundary
  summaries also preserve lowered host-operation edges, source-to-lowered
  links, and target policy-check records. Exact source policy path matching is
  still pending because target `boundary ...` declarations are not yet carried
  in the semantic spine.

## Stages

- [Terminal Psi target architecture and migration](terminal_psi.md)

- [Source Files To Tokens](stages/source_files_to_tokens.md)
- [Tokens To Syntax Trees](stages/tokens_to_syntax_trees.md)
- [Syntax Trees To Symbol Resolved Trees](stages/syntax_trees_to_symbol_resolved_trees.md)
- [Symbol Resolved Trees To Typed Trees](stages/symbol_resolved_trees_to_typed_trees.md)
- [Typed Trees To Checked Trees](stages/typed_trees_to_checked_trees.md)
- [Checked Trees To State Graph](stages/checked_trees_to_state_graph.md)
- [State Graph To Control Flow](stages/state_graph_to_control_flow.md)
- [Control Flow To Abstract Operations](stages/control_flow_to_abstract_operations.md)
- [Abstract Operations To Target Operations](stages/abstract_operations_to_target_operations.md)
- [Terminal Target Operations To Selected Instructions](stages/terminal_target_operations_to_selected_instructions.md)
- [Selected Instructions To Liveness](stages/selected_instructions_to_liveness.md)
- [Target Operations To Assigned Target Operations](stages/target_operations_to_assigned_target_operations.md)
- [Target Operations To Machine Program](stages/target_operations_to_machine_program.md)
- [Assigned Target Operations To Machine Instructions](stages/assigned_target_operations_to_machine_instructions.md)
- [Machine Instructions To Machine Bytes](stages/machine_instructions_to_machine_bytes.md)
- [Machine Bytes To Object Plan](stages/machine_bytes_to_object_plan.md)
- [Object Plan To Relocations](stages/object_plan_to_relocations.md)
- [Object Relocations To Final Image](stages/object_relocations_to_final_image.md)
