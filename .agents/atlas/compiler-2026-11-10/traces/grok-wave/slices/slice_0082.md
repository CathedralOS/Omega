slice_0082

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0626 | LSG:7376-7409; LSrc:3053 | REACHABLE | `structural_contract` can yield `None` while abstracted structural parameters are non-empty and target `graph.parameters` is empty; legalization stores `None` with no error. |
| A0627 | LSrc:1287-1469 | REACHABLE | Byte/element view projections ignore the paired `TargetUnitOperation` row; ordinary `legalize_target_operations` does not run the standalone target audit. |
| A0628 | LT:810-812; S03/selection.rs:166-168 | REACHABLE | Unreachable blocks make `dominates` return true for any candidate (audit); selection rejects CFGs that omit blocks from DFS postorder. |
| A0629 | S03/optimized.rs:68-70 | CLOSED | Missing `fixed_view` skips constraint insertion; entry register selection fails closed at `selection_scalar_graph.rs:7169-7176`. |
| A0630 | S03/selection.rs:1559 | REACHABLE | `_ => false` omits unlisted instruction kinds from `referent_used`, so structural parameter setup can be skipped. |
| A0631 | S03/selection_call_abi.rs:1428-1430 | CLOSED | `result.is_none()` widens the predicate, but `entry` revalidates placements at `selection_scalar_graph.rs:6239-6243`. |
| A0632 | SIN:647-649;2540;4856-4858;3276;30-32 | REACHABLE | Hand-maintained place-use lists and `read_storage_codec` fallbacks treat unknown instruction kinds as non-users / generic scalars. |
| A0633 | S03/selection_interrupts.rs:404-475 | CLOSED | `104` and `10` are fixed layout offsets (`STACK_TOP`, descriptor size) after `Layout::prepare` validates installation geometry. |

### A0626

1. Read `structural_contract` in full at `legalization_scalar_graph.rs:7376-7409` (read): returns `Some` when `structural_parameters(target)` or the `or_else` empty-slice/`place_declarations::matches_producers` branch applies; otherwise `None`.
2. Read `derive` at `legalization_source.rs:2966-3064` (read): assigns `structural: scalar_graph_input::structural_contract(...)` at `3053-3055` without treating `None` as error.
3. Search `\bstructural_contract\(` in `omega-rust`: 2 production hits (`legalization_scalar_graph.rs` definition, `legalization_source.rs:3053` call); identity encoding uses separate `encode_structural_contract` symbols.
4. Read `derive_source_function_rosters` / `validate_source_custody` at `legalization_source.rs:17-32` and `3072-3087` (read): custody checks plan lengths and ids only, not structural contract presence.
5. Read `legalize_target_operations` at `legalization.rs:405-437` (read): does not call `verify_target_operation_plan` / `audit_target_operation_plan`.
6. Input reaching accepting branch: target function with empty `graph.parameters`, no `mixed_structural_scalar_abi`, abstracted with non-empty `structural_parameters`, `or_else` guard false (`legalization_scalar_graph.rs:7382-7387`).
7. **Verdict: REACHABLE.** A target plan whose abstract side lists owned structural parameters but the target graph carries none can legalize with `structural: None`; homes and type-driven selection paths that require `source.structural` may be skipped or fail later, but legalization accepts the program.
8. Unsure: whether any later mandatory gate always errors before object emission for every such mismatch (did not trace full compiler driver beyond stage 03).

### A0627

1. Read `project_byte_sequence_write/read` and `project_element_view_write/read` at `legalization_source.rs:1287-1408` (read): project from `OptimizationNode` / abstract op only; no `targeted` argument.
2. Read dispatch at `legalization_source.rs:2318-2323` (read): `ByteSequenceWrite`/`Read` call those projectors without checking `targeted` variant (contrast `ByteSequenceStore` at `2287-2290` and `StructuralByteViewFieldStore` at `2297-2304`).
3. Read `derive` pairing loop `legalization_source.rs:3023-3034` (read): zips optimized nodes with target rows by position but does not require per-row variant equality before projection.
4. Read `audit_target_operation_plan` / unit-op matches at `legalization_target.rs:6287-6354` and expression checks at `2527-2557` (read): standalone verifier compares target vs abstract for these ops.
5. Read `legalize_target_operations` at `legalization.rs:405-421` (read) and `construct_optimized_instruction_selection` at `optimized.rs:98-107` (read): ordinary compile path legalizes only; `verify_target_operation_plan` is separate (`lib.rs:19-20`, read).
6. Search `project_byte_sequence_write` in `omega-rust`: 3 hits (definition + dispatch in `legalization_source.rs`).
7. Input reaching branch: positionally aligned node/row pair where abstract node is `ByteSequenceWrite` but target row differs (or target fields disagree); projection follows abstract only.
8. **Verdict: REACHABLE.** A corrupt or foreign target plan with mismatched unit-operation variants at a byte/element access site can legalize from abstract alone when the standalone audit is not run.
9. Entry description matches read code for LSrc projections; LT audit is the closed path when invoked.

### A0628

1. Read `SourceDominators::dominates` at `legalization_target.rs:806-817` (read): if `block` absent from `order`, returns `true` at `810-811`.
2. Read dominator construction `legalization_target.rs:718-797` (read): `order` includes only blocks reachable from `function.entry` via DFS.
3. Read consumer at `legalization_target.rs:927-932` (read): failed dominate check rejects; unreachable consumer block makes `dominates` vacuously true.
4. Read selection `ordered_blocks` at `selection.rs:144-167` (read): if DFS postorder length differs from block count, returns `None`; caller at `139` maps to custody error.
5. Input reaching LT branch: structural-home audit for a place used in a CFG block not reachable from entry; producer in a reachable block still passes dominate guard.
6. **Verdict: REACHABLE** for standalone target audit (`verify_target_operation_plan` → `match_input` dominator checks). Selection path is **CLOSED** at `selection.rs:166-168`, but that is a different mechanism than `LT:810-811`.
7. Unsure: none for dominate semantics; selection rejection not re-traced through all selection callers.

### A0629

1. Read `push_fixed_input` in full at `optimized.rs:52-78` (read): `environment.fixed_register_view(register)` `None` causes early `return` at `68-70`.
2. Search `\bpush_fixed_input\b` in `omega-rust`: 2 hits (definition + call in `optimized.rs:35`).
3. Read caller `selection_constraints` `optimized.rs:16-49` (read): builds `fixed_inputs` for required register parameters.
4. Read consumer `selection_scalar_graph.rs:7169-7178` (read): `fixed_input_constraint(...).ok_or_else(invalid)?` errors when constraint missing; also compares `fixed_register_view` to stored view.
5. Input: required entry parameter in a fixed register without a catalogued fixed view in the validated register environment.
6. **Verdict: CLOSED.** Absence of constraint leads to selection custody failure, not silent acceptance.
7. Did not read every register-parameter emission site beyond `7169`.

### A0630

1. Read `referent_used` in full at `selection.rs:1469-1561` (read): large explicit match; `_ => false` at `1559`.
2. Search `\breferent_used\b` in `omega-rust`: 2 hits (definition + `selection_structural.rs:227`).
3. Read caller `selection_structural.rs:227-229` (read): `if owned_pointer.is_none() && !used { continue; }` skips parameter referent transport.
4. Input: owned structural parameter place only referenced by a legalized instruction kind not listed in the match (e.g. `ByteSequenceWrite`, `ElementViewWrite` absent from read list).
5. **Verdict: REACHABLE.** Adding a new instruction that reads a parameter place without updating `referent_used` makes the compiler treat the referent as unused and omit setup.
6. Unsure: whether all current production instruction kinds that read referents are already listed (not exhaustively compared to enum).

### A0631

1. Read `accepts_stack_parameter_entry` at `selection_call_abi.rs:1428-1444` (read): first disjunct `source.call_plan.result.is_none()` is sufficient for `true`.
2. Search `\baccepts_stack_parameter_entry\b` in `omega-rust`: 2 hits (definition + `selection_scalar_graph.rs:6229`).
3. Read `entry` at `selection_scalar_graph.rs:6224-6243` (read): even when predicate true, each required stack parameter must match `call_plan`, shape, and placement or `SelectedInstructionError::custody()`.
4. Input: void-return function with deliberately wrong parameter placements still evaluated under full checks in `entry`.
5. **Verdict: CLOSED.** Permissive predicate is narrowed by mandatory checks in the sole production caller.
6. Did not read tests or non-production call sites (none found).

### A0632

1. Read `parameter_home_required` match `_ => false` at `selection_inputs.rs:547-649` (read).
2. Read `requires_descriptor` `_ => false` at `selection_inputs.rs:2503-2540` (read).
3. Read `read_storage_codec` fallback `_ => runtime_abi::ScalarStorageCodec::from_scalar(scalar)` at `selection_inputs.rs:3250-3277` (read).
4. Read `scalar_storage_origins` `unknown_storage` branch at `selection_inputs.rs:4856-4858` (read); caller `selection.rs:977-982` returns `Ok(false)` on `None` (read).
5. Read `local_shape` early accepts at `selection_inputs.rs:30-32` and `49-51` (read): non-structural slots and non-`EstablishActivationClaim` rows return `Some(ordinary)` without structural footprint refinement.
6. Input: new `LegalizedScalarInstructionKind` variant that reads an owned parameter place but is omitted from the hand-maintained lists.
7. **Verdict: REACHABLE.** Maintenance gap: unlisted kinds are treated as non-users; codec fallback can accept generic scalar encoding without path validation.
8. `scalar_storage_origins` `None` path looks conservative for foreign-backing query (not silent accept of bad geometry).

### A0633

1. Read emission block `selection_interrupts.rs:404-475` (read): uses `checked.gdt_bytes` for GDT backing accesses and literal `104` at `425` plus `10` at `433-434` for descriptor field stores.
2. Read constants `STACK_TOP: u32 = 104` and layout module at `selection_interrupts.rs:1437-1445` and `layout::Layout::prepare` at `913-933` (read): installation requires `source.structural` and derived layout types.
3. Read `with_validated` gate at `1470-1512` (read): rejects inconsistent entry/gate associations before success closure runs.
4. Input: admitted static interrupt installation after layout preparation; hard-coded offsets match internal `Layout` schema, not arbitrary caller sizes.
5. **Verdict: CLOSED.** Constants are schema positions, not permissive acceptance of mismatched footprint; dynamic sizes use `checked.gdt_bytes` / `checked.table_bytes` where appropriate.
6. Did not read full `Layout::derive` body beyond `prepare`.

### What I did not read

- Full bodies of `match_input` and all `legalization_target.rs` audit matchers beyond cited slices.
- Stage 04+ pipeline gates after instruction selection.
- Complete `LegalizedScalarInstructionKind` enum for exhaustive diff against `referent_used` / `parameter_home_required` lists.
- `Layout::derive` implementation details in `selection_interrupts.rs`.
- Test-only modules in stage 03.

SLICE COMPLETE
