# slice_0082

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0626 | omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_source.rs:3053 | REACHABLE | `structural: None` is stored without error and structural entry becomes a no-op. |
| A0627 | omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_source.rs:2318 | REACHABLE | Byte/element view read/write projections take no `targeted` row and never compare to the target plan. |
| A0628 | omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_target.rs:810 | UNDECIDED | Selection rejects unreachable CFGs; audit `home_available` uses the dominate quirk but default legalize path does not call that audit. |
| A0629 | omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/optimized.rs:68 | CLOSED | Skipped fixed-input constraints still fail selection at `fixed_input_constraint` lookup. |
| A0630 | omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/selection.rs:1559 | REACHABLE | Unlisted instruction kinds fall through `_ => false` and skip structural parameter transport. |
| A0631 | omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/selection_call_abi.rs:1429 | REACHABLE | `call_plan.result.is_none()` makes the entry predicate true before any placement check. |
| A0632 | omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/selection_inputs.rs:648 | REACHABLE | Hand-maintained `parameter_home_required` lists end in `_ => false` for unlisted readers. |
| A0633 | omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/selection_interrupts.rs:425 | REACHABLE | Task-state and descriptor memory footprints use literals 104 and 10 instead of layout-derived sizes. |

## A0626

Digest (read): area_04.md:211 states `LSG:7376-7409` into `LSrc:3053` accepts `structural_contract` returning `None` (inferred).

Opened `structural_contract` in full at `legalization_scalar_graph.rs:7376-7409` (read). It returns `Some(LegalizedStructuralContract { ... })` when `structural_parameters(target)` is `Some`, or when the `or_else` at `:7382-7387` supplies an empty slice via `matches_producers`; otherwise it returns `None` at `:7408`.

Caller search: `rg '\bstructural_contract\(' omega-rust` → 5 matches (read): one definition at `legalization_scalar_graph.rs:7376`, one legalization call at `legalization_source.rs:3053`, three identity encoders in `legalized-operations` (not on the accept/reject path for this row).

At `legalization_source.rs:3046-3055` (read), `LegalizedScalarFunction` is built with `structural: scalar_graph_input::structural_contract(...)` inside `Ok(...)` with no `?` or `ok_or` on the `Option`.

Upstream: `legalize_target_operations` at `legalization.rs:405-432` (read) calls `derive_source_function_rosters` only; it does not require `structural` to be `Some`.

Downstream acceptance: `selection_structural.rs:134-142` (read) — `entry` returns `Ok(())` immediately when `source.structural` is `None`, skipping structural-parameter transport and `accepts_graph` at `:1444-146`.

Input in plain words: a machine function that still has structural places or structural-parameter uses in the optimized unit, but `structural_contract` returns `None` (for example target graph structural parameters empty and the `or_else` path does not apply), is legalized with `structural: None` and selection does not run structural entry checks.

Verdict **REACHABLE**: such a program should be rejected for missing structural contract data but is stored and later passes structural entry as a no-op.

## A0627

Digest (read): area_04.md:212 — byte and element view read/write projections do not compare against the target row.

Opened `project_byte_sequence_write`, `project_byte_sequence_read`, `project_element_view_write`, and `project_element_view_read` at `legalization_source.rs:1287-1408` (read). Each only destructures the `AbstractOperation` variant and builds a `LegalizedScalarInstructionKind`; none take `target_operations::TargetUnitOperation`.

Contrast (read): `project_byte_sequence_store` at `:1442-1468` requires `TargetUnitOperation::ByteSequenceStore { .. } = targeted` else `Err(Error::custody())`. `StructuralByteViewFieldStore` dispatch at `:2297-2304` pattern-matches `targeted`.

Dispatch at `:2318-2400` (read) routes `ByteSequenceWrite`, `ByteSequenceRead`, `ElementViewRead`, and `ElementViewWrite` to the storage projectors without passing or checking `targeted`.

Input in plain words: a target plan whose row kind or fields for a byte or element view operation disagree with the abstract operation still legalizes if stage-02 pairing reaches these projectors.

Verdict **REACHABLE**: mismatching target rows for those four operations are not compared and projection succeeds.

## A0628

Digest (read): area_04.md:213 — `LT:810-812` with `:446-448` treats an unreached block as dominated by everything; selection rejection traced at `selection.rs:166-168`; standalone audit open.

Opened `SourceDominators::dominates` at `legalization_target.rs:806-818` (read). When `block` is missing from `self.order`, `:810-812` returns `true` for any `candidate`.

Opened `InputScope::admits` at `:436-454` (read). For `SourceScope::Block`, interval filtering uses `dominates` indirectly via dominator tree construction (`:800-804`).

Opened `structural_cases::home_available` at `:887-934` (read). At `:929-931`, for operation-result homes it requires `producer_block == block || sources::dominates(optimized, producer_block, block)`; with `:810-812`, an unreachable `block` makes `dominates` true for every producer.

Selection closing check (read): `selection.rs:144-167` — `ordered_blocks` DFS; if `postorder.len() != outgoing.len()` at `:166-167`, returns `None`; `block_order::derive` maps that to `SelectedInstructionError::custody()` at `:139`. Input: legalized function containing a CFG block not reachable from entry fails selection here.

Legalize path (read): `legalization.rs:405-432` does not call `audit_target_operation_plan` or `match_input`; audit is only under `verify_target_operation_plan` at `:466-480`.

Verdict **UNDECIDED** for compiler acceptance on the default legalize→select path: selection appears to reject unreachable CFGs, but whether a bad target plan whose unreachable block references values from non-dominating reachable blocks is caught only in the optional audit (`home_available` + `:810-812`) was not fully traced to a single accept/reject outcome on the main pipeline.

## A0629

Digest (read): area_04.md:214 — required register parameter with no `fixed_register_view` gets no constraint and no error.

Opened `push_fixed_input` at `optimized.rs:52-78` (read). At `:68-70`, `environment.fixed_register_view(register)` `None` causes `return` without pushing a constraint.

Caller search: `rg 'fixed_input_constraint\(' omega-rust` → 2 matches (read): definition `selection.rs:594-609`, sole use `selection_scalar_graph.rs:7169-7176`.

Opened `select_function` parameter loop at `selection_scalar_graph.rs:7139-7178` (read). For required register incoming parameters (`required_values` contains `parameter.value`, register placement at `:7146-7154`), `:7169-7176` requires `fixed_input_constraint(...).ok_or_else(invalid)?` and matching `environment.fixed_register_view`.

Input in plain words: a function whose required incoming scalar is assigned to a register the environment does not model — constraint building silently omits it, but selection still requires that constraint record.

Verdict **CLOSED** on program acceptance: every required register parameter path that would have received a fixed-input constraint goes through `selection_scalar_graph.rs:7169-7176` and fails without a matching constraint.

## A0630

Digest (read): area_04.md:215 — instruction kinds not in `referent_used` count as not using the referent.

Opened `parameter_use::referent_used` at `selection.rs:1469-1561` (read). The `source.instructions()` match ends `_ => false` at `:1559`. `ByteSequenceWrite` and `ElementViewWrite` are not named in any arm (read: arms include `ByteSequenceRead` and `ElementViewRead` at `:1546-1548`, not the Write variants).

Caller search: `rg '\breferent_used\(' omega-rust` → 2 matches (read): definition `selection.rs:1469`, one production call `selection_structural.rs:227`.

Opened caller at `selection_structural.rs:227-229` (read): `if owned_pointer.is_none() && !used { continue; }` skips further transport for that structural parameter.

Input in plain words: a structural parameter place only referenced by a legalized instruction kind omitted from the `referent_used` list is treated as unused and gets no pointer/home setup.

Verdict **REACHABLE**: e.g. a parameter only used by `ElementViewWrite` or `ByteSequenceWrite` should require referent transport but is skipped.

## A0631

Digest (read): area_04.md:216 — void-return functions pass `accepts_stack_parameter_entry` without parameter scrutiny.

Opened `accepts_stack_parameter_entry` at `selection_call_abi.rs:1428-1444` (read). First disjunct `source.call_plan.result.is_none()` at `:1429` returns `true` for the whole predicate via short-circuit.

Caller search: `rg '\baccepts_stack_parameter_entry\(' omega-rust` → 2 matches (read): definition and `selection_scalar_graph.rs:6228-6229`.

Opened caller `entry` at `selection_scalar_graph.rs:6224-6243` (read). When `accepts_stack_parameters` is true, stack parameters still require matching shape and `call_plan.parameters` at `:6239-6242`, but only for parameters in `required_values` (`:6231-6232`).

Structural functions: `selection_structural.rs:140-146` (read) still runs `accepts_graph` when `source.structural` is `Some`.

Input in plain words: any void-return function makes the stack-entry predicate true immediately, including void-return functions with structural signatures and placement disagreements that never touch the later per-parameter checks (for example parameters not in `required_values`).

Verdict **REACHABLE**: the entry predicate accepts void-return programs before placement validation; later checks do not re-derive the full predicate for all parameters.

## A0632

Digest (read): area_04.md:217 — new instruction kinds not added to hand-maintained place-reader lists silently get no home.

Primary site: opened `parameter_home_required` at `selection_inputs.rs:476-649` (read). Instruction scan ends `_ => false` at `:648-649`. No arms for `ByteSequenceWrite` or `ElementViewWrite` (grep over file: 0 matches).

Related lists (read): `requires_descriptor` at `:2492-2541` ends `_ => false` at `:2540`. `view_access` inner scan at `:547-649` (same function body as cited). `activation_storage_input::local_shape` at `:30-32` returns `Some(ordinary)` for non-structural slots without extra checks.

Opened `unknown_storage` branch at `:4856-4858` (read) inside scalar storage origin walking: returns `Ok(None)` when `inventory.unknown_storage` contains a node. Opened `read_storage_fragments` default at `:3276` (read): non-matching instruction kinds fall through to `runtime_abi::ScalarStorageCodec::from_scalar(scalar)`.

Caller search: `rg '\bparameter_home_required\(' omega-rust` → 3 matches (read): definition and uses in `selection_inputs.rs:472`, `selection_structural.rs:334`.

Input in plain words: a new `LegalizedScalarInstructionKind` that reads an owned structural parameter place but is not copied into the three hand-maintained enumerations is treated as not needing a home or descriptor.

Verdict **REACHABLE**: omission from the lists at `:648-649` (and parallel lists) silently disables home/descriptor requirements for that instruction kind.

## A0633

Digest (read): area_04.md:218 — interrupt installation memory uses hard-coded 104 and 10.

Opened emit path at `selection_interrupts.rs:404-475` (read). After layout-checked GDT accesses using `checked.gdt_bytes` at `:409-418`, task backing read uses byte count `104` at `:425-426`; descriptor write uses `10` at `:432-434`. Parallel guard path at `:1584-1605` (read) uses `layout.gdt_bytes` then literals `104` and `10` again at `:1597` and `:1605`.

Opened `const STACK_TOP: u32 = 104` at `:1445` (read). GDT/table sizes still come from `layout` (`:1589`, `:1614`).

Input in plain words: admitted static interrupt installation always emits fixed 104-byte task-state and 10-byte descriptor snapshot accesses regardless of per-argument layout fields at those call sites.

Verdict **REACHABLE**: a program whose installed task-state or descriptor backing size differs from 104 or 10 should be rejected or sized from layout but still receives hard-coded memory footprints at selection.

## What I did not read

- Full bodies of `match_input`, `audit_target_operation_plan` beyond signatures, and all `home_available` callers in `legalization_target.rs` outside `:887-934`.
- `place_declarations::matches_producers` and every condition that makes `structural_contract` return `None` vs `Some`.
- Provider-admission rules that might force 104-byte task state before stage-03 selection.
- Corpus programs or runtime behavior beyond the cited Rust paths.
- `omega-rust` outside stage-03 except caller grep results noted above.

SLICE COMPLETE
