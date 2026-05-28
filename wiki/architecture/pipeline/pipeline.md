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
- Effects: externally visible capability behavior such as allocation, IO,
  process exit, or host interaction.
- Boundary edges: points where Omega accepts a declared contract from
  compiler/runtime/host/toolchain code.

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
- [Machine Instructions To Object File](stages/machine_instructions_to_object_file.md)
- [Object File To Final Image](stages/object_file_to_final_image.md)
