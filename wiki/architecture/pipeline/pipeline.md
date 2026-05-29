# Pipeline Architecture

Omega's compiler pipeline is a sequence of durable representation boundaries.
Each stage should have one primary job, one input representation, and one output
representation.

The same semantic nouns should be recognizable across stages, but their data
shape changes as they become more resolved. Source-shaped IR can only say "this
syntax looks like a place." Checked IR can say "this place overlaps this loan."
Backend IR can say "this place is stack slot plus offset."

## Stage Questions

Every stage document should answer:

- Input representation.
- Output representation.
- Primary responsibility.
- Places, values, facts, loans, moves, drops, calls, transitions, effects, and
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
- Effects: externally visible behavior classes such as allocation, IO, process
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
- If it proves obligations, records facts, creates loans, or validates effects,
  it belongs near checked trees.
- If it schedules already-checked events into graph/control form, it belongs in
  graph or control-flow lowering.
- If it chooses storage, ABI, instruction, relocation, or image form, it belongs
  in the backend lowering stages.

## Semantic Ownership Matrix

This table is intentionally blunt. Each cell says the main relationship between
the stage and the noun: `none`, `syntax`, `identity`, `typed`, `checked`,
`scheduled`, `lowered`, `assigned`, `encoded`, `artifact`, `metadata`, or
`final`.

| Stage | Places | Values | Facts | Loans | Moves | Drops | Calls | Transitions | Effects | Boundaries |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Source Files To Tokens | none | none | none | none | none | none | none | none | none | token |
| Tokens To Syntax Trees | syntax | syntax | syntax | none | none | none | syntax | syntax | syntax | syntax |
| Syntax Trees To Symbol Resolved Trees | identity | identity | identity | none | none | none | identity | identity | identity | identity |
| Symbol Resolved Trees To Typed Trees | typed | typed | typed | type surface | planned | planned | typed | typed | typed | typed |
| Typed Trees To Checked Trees | checked | checked | checked | checked | checked | checked | checked | checked | checked | checked |
| Checked Trees To State Graph | scheduled | scheduled | scheduled | scheduled | scheduled | scheduled | scheduled | graph | scheduled | scheduled |
| State Graph To Control Flow | lowered | lowered | preserved | preserved | lowered | lowered | control flow | control flow | preserved | control flow |
| Control Flow To Abstract Operations | lowered | lowered | metadata | assertion | abstract op | abstract op | abstract op | abstract op | op metadata | metadata |
| Abstract Operations To Target Operations | target | target | metadata | assertion | target op | target op | target op | target op | target op | target metadata |
| Target Operations To Assigned Target Operations | assigned | assigned | metadata | none | assigned | assigned | assigned | assigned | assigned | assigned metadata |
| Assigned Target Operations To Machine Instructions | encoded | encoded | metadata | none | instruction | instruction | instruction | instruction | instruction | instruction metadata |
| Target Operations To Machine Program | artifact | artifact | metadata | none | artifact | artifact | artifact | artifact | artifact | artifact metadata |

Current deliberate gaps:

- Moves and drops now have durable checked/control-flow event plumbing, but
  event production still needs type-aware precision plus transition and nested
  call coverage.
- Checked values now preserve through state graph, control flow, abstract
  operations, target operations, assigned target operations, symbolic machine
  instructions, and the current machine-program artifact, but still need
  type-aware ownership kind, drop policy, storage consequences, and backend
  lowering beyond metadata.
- Source-level boundary trait calls now preserve as checked, graph, and
  control-flow boundary edges. Backend boundary summaries still preserve
  lowered host-operation edges as separate metadata, and those two layers still
  need an explicit linkage to target policy.

## Stages

- [Source Files To Tokens](stages/source_files_to_tokens.md)
- [Tokens To Syntax Trees](stages/tokens_to_syntax_trees.md)
- [Syntax Trees To Symbol Resolved Trees](stages/syntax_trees_to_symbol_resolved_trees.md)
- [Symbol Resolved Trees To Typed Trees](stages/symbol_resolved_trees_to_typed_trees.md)
- [Typed Trees To Checked Trees](stages/typed_trees_to_checked_trees.md)
- [Checked Trees To State Graph](stages/checked_trees_to_state_graph.md)
- [State Graph To Control Flow](stages/state_graph_to_control_flow.md)
- [Control Flow To Abstract Operations](stages/control_flow_to_abstract_operations.md)
- [Abstract Operations To Target Operations](stages/abstract_operations_to_target_operations.md)
- [Target Operations To Assigned Target Operations](stages/target_operations_to_assigned_target_operations.md)
- [Assigned Target Operations To Machine Instructions](stages/assigned_target_operations_to_machine_instructions.md)
- [Target Operations To Machine Program](stages/target_operations_to_machine_program.md)
