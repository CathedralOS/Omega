# Atlas packet unit_010: stage 04 target lowering and its audit

Packet complete. Every correctness claim in this slice now has a disposition.
None of them produces a wrong result or accepts an invalid program on current
`main`. Each fail-open lead either cannot be reached, because an earlier stage
already refuses the input, or a second audit check catches it. The open items
are diagnostic quality and a few unsupported shapes. These shapes are refused
rather than miscompiled.

This packet covers [record unit_010](../../records/unit_010.md) and the
lines of the [area 10 digest](../../digests/10-native-stages-04-10.md) that
cite it. All evidence comes from `main` at
`24ad2700685e6cc6062026c87ed5fd24ed415265`. No range in
`second_pass_ranges.tsv` belongs to unit 10, so no gap-packet range was skipped.
The [Psi and native ledger](../07-13-psi-and-native.md) had no earlier
disposition for these claims. Its stage 04 rows (mixed ABI detection and the
local-candidate boundary) belong to other units and are unchanged.

Paths shorten
`source-rust/subsystems/compiler/psi-to-native/pipeline/04_abstract-operations-to-target-operations/src/`
to `S04/`. "The audit" is the independent target-row checker in
`S04/preparation/legalization_target.rs`. It runs on an explicit validator
request and in tests, and never during ordinary compilation
(`S04/preparation/legalization.rs:793-803`). Its product entry is
`verify_execution_plan`, which `verify_retained_target_operations` calls
(`psi-to-native/verification/published-objects/src/lib.rs:344-372`). Line
numbers below are current; the record's line numbers are from `4f3c255d`.

## Fail-open leads

| # | Claim (original location) | Current source | Contract, test or trace | Outcome |
| --- | --- | --- | --- | --- |
| 1 | `SourceDominators::dominates` returns `true` for a block missing from the order (record §1 fail-open 1; digest line 146, §10 item 3 at line 264) | `S04/preparation/legalization_target.rs:818-830`. `InputScope::admits` (`:451-469`) uses the same convention when `self.interval.is_none()` | A block missing from the reverse-postorder is unreachable from the entry, and every block vacuously dominates an unreachable one. The audit's optimization unit comes from stage 03. Stage 03 validates the unit (`03_abstract-operations-to-abstract-operations/src/abstract_optimization/pass_manager.rs:1355`) through `validate_total_cfg`, which refuses any unreachable block (`representations/abstract-operations/optimization-unit-semantics/src/unit_validation.rs:6062-6085`, called at `:9917`). Lowering also refuses one (`S04/target_lowering/control_flow.rs:84-86`, `:3061-3062`). The audit pairs each target block with a source block by position and id, so every block it judges is in the order. | Intended; earlier stages refuse unreachable input |
| 2 | `Boolean::Immediate` accepts when `available` is `None` (record §1 fail-open 2; digest line 148) | `legalization_target.rs:2980`; the same `is_none_or` pattern in `scalar_definitions::observation` (`:3762`, `:3837`, `:3854`, `:3879`, `:3902`) | The crate builds `Checker` in exactly three places: `:1796`, `:1928` and `:5630`. All three set `available: Some(..)`; a grep of `Checker {` over `S04/` finds no other construction. | Intended; unreachable |
| 3 | `arithmetic_carrier_admitted` admits every policy/operator pair except three, and an inline copy repeats it (record §1 fail-open 3 and "same job"; digest lines 68 and 147) | `legalization_target.rs:3180-3195` and the inline copy at `:3746-3759` | Both are pre-filters, and the audit checks the same row twice more. `match_input` admits integer arithmetic only through `integer_arithmetic_admitted` (`S04/preparation/legalization_scalar_graph.rs:7790-7799`, `:8600-8612`, rule at `:7856-7870`). That rule needs a saturating carrier for every saturating operator and a native scalar shape for the others. The source projection also needs a saturating carrier for saturating add, subtract and multiply (`S04/preparation/legalization_source.rs:365-390`), saturating division (`:455-475`) and wrapping division (`:557`). The record says both copies have "the same two match arms", but they differ: the inline copy at `:3752` omits saturating `Multiply`. The other checks cover that pair, so the difference changes no verdict. | Intended; not fail-open. The record's "same arms" detail is wrong. The two copies are a cleanup candidate only. |
| 4 | Four `validate_operation` arms only compare fields with the abstract operation (record §1 fail-open 4; digest line 149) | `ReleaseActivationClaim` `legalization_target.rs:4053-4067`, `PortWrite` `:6185-6199`, `EstablishByteSequenceConcatenation` `:6595-6611`, `GlobalDescriptorTable` `:6812-6827` | `PortWrite` carries only a `u16` port and a `u8` value as immediates (`representations/target-operations/src/operations.rs:475-480`), so equality is its whole contract. `ReleaseActivationClaim` carries an argument and a claim id, both compared. The concatenation row's capacity proof is looked up as an accepted fact when the target row is projected (`S04/preparation/legalization_instruction.rs:582-594`). `GlobalDescriptorTable` skips its `indices` with `..` in this arm. The source projection derives those indices again (`legalization_source.rs:1708-1738`) and the retained projection reads them from the row (`legalization_instruction.rs:468-479`). `audit_target_operation_plan_with_module` then requires the derived and retained rosters to be equal (`legalization.rs:776-789`). | Intended; the roster comparison checks the remaining fields |
| 5 | The `byte_view` and `element_view` parameter arms depend on `structural_parameters` (record §1 fail-open 5) | `legalization_target.rs:111-130`, `:2082`, `:2159`; `structural_parameters` at `legalization_scalar_graph.rs:7422-7430` | A `None` from `structural_parameters` makes `is_some_and` false, so the audit refuses. A match also needs the place, the arrival parameter's type, access, multiplicity, qualifications and placement to agree. | Intended; closed, as the record found |
| 6 | Only one missing lookup returns `true` (record §1 fail-open 6) | `legalization_target.rs:822-824` and `:462` | These are the vacuous-dominance cases from receipt 1. No other missing-lookup `true` was found in the arms reviewed. | Duplicate of 1 |
| 7 | `ModuleTypes::integer` and `scalar` turn a catalog error into `None` (record §2) | `S04/preparation/mod.rs:41-55`, `:71-78` | Callers treat `None` as a custody refusal. `accepted_types` also needs the Psi identity to match and the module owner to be the same object (`std::ptr::eq`); the in-file test covers a module with equal bytes but another owner. | Intended |
| 8 | Publication capture and verification may disagree for an operation that has both a cleanup roster and an edge, for example `Crash` (record "unsure") | `S04/preparation/publication.rs:29-46` and `:129-163`; `cleanup_rosters` at `representations/abstract-operations/src/operations.rs:800-838` | `cleanup_rosters` yields one edge for each `Jump`, `Return`, `ReturnUnit` and `ReturnStructural`, both edges of a `Conditional`, and every `StructuralCase` edge, including those with no cleanup actions. `Crash`, `InvokeNoNormal`, `ExternalCompletion` and `EliminateEmpty` yield none, so capture takes their `psi_edge` from the fallback match. `Crash` has no cleanup field (`operations.rs:695-700`). Verification lists the same edges for each variant. The two differ only for a `StructuralCase` with no cases: capture refuses it ("has no semantic occurrence") on the production path, which runs first. | Intended; the two lists agree |
| 9 | `call_result_origins` turns a `resolve` error into `None` with `.ok()`; does the graph then fail? (record §5 fail-open, "unsure") | `S04/target_lowering/call_result_origins.rs:64-99`; `representations/abstract-operations/optimization-unit-semantics/src/descriptor_results.rs:1167-1290`, `adapt` at `:1364` | A `None` from the callback makes `adapt` return `None` (`descriptor_results.rs:1580`, `operation_projection(function, node)?`). `catalog_graph` collects `Option`s, so the whole graph is `None`, and `retain` reports "descriptor backing graph is unresolved" (`call_result_origins.rs:93-99`). Operation lowering also resolves the same selected call and returns its own error (`control_flow_operations.rs:321-332`). | Intended; the refusal is kept |
| 10 | `reference_arrival` returns the current state when a predecessor recorded no per-edge exits (record §6 fail-open; digest line 141) | `S04/target_lowering/control_flow.rs:2764-2797` | Only `Conditional` (`:2050-2053`) and `StructuralCase` (`:1161`, `:1214`) call `finish_reference_edges`. Each records one exit per successor edge, after that edge's own cleanup and bindings. The remaining terminator with an in-function successor is `Jump`. It applies `re_home_bound_references` and its cleanup to `live` itself (`:1981-1988`), so the block's exit state is its single edge's state. `Return`, `ReturnStructural`, `ExternalCompletion`, `InvokeNoNormal`, `EliminateEmpty` and `Crash` have no in-function successor. Exits are cleared when each block is lowered again (`:3188`). | Intended |
| 11 | The fixed-point loop turns per-block errors into "deferred" (record §6 fail-open; record "five things" item 3) | `control_flow.rs:3126-3343`, check after the loop at `:3345-3371` | `deferred` is reset on each pass (`:3128`). The loop refuses if the last unchanged pass still deferred a block (`:3339-3341`). A block never lowered fails at `:3372-3375`. After the loop, every arrival is computed again from all predecessors and must equal the stored entry, otherwise lowering refuses. | Intended, as the record traced |
| 12 | `plain_home_cleanup` `RetireHome` removes reference entries (record §6 fail-open) | `control_flow.rs:1369-1412` | Lowering refuses retirement with `InvalidStructuralValueInput` unless the place has an owned root, a retired reference root, a disposable view carrier or a retirable arrival. It then drops only the place's non-`Referent` entries. | Intended |
| 13 | `IeeeFloatCompare` inserts its scalar home without a duplicate check, unlike `FloatResultPolicy` (record §8 fail-open; digest line 68) | `S04/target_lowering/control_flow_operations.rs:644-649`, `:681`, `:712` | `control_flow::lower` refuses a duplicate defining value for every function before any operation is lowered, and `IeeeFloatCompare` is in that list (`control_flow.rs:2877-2919`, `:2886`). The three insertion paths still differ in style. | Intended; a cleanup candidate only |
| 14 | `unreachable!` in `ieee_float_arithmetic::lower` (record §8; digest line 241) | `control_flow_operations.rs:134-145` | Its only caller is the `NearestIeeeFloatArithmetic` arm inside `AbstractOperation::BoundaryCall` dispatch (`:1005`, `:1154-1162`). | Intended; unreachable |
| 15 | `OMEGA_COMPILER_CHECK_SITES` debug prints (record §1, §6, §7) | for example `control_flow.rs:1384`, `:1415` | The prints are guarded by `var_os(..).is_some()` and sit on paths that already refuse or report; they do not change verdicts (digest line 243). | Intended |

## Recount and caller corrections

| # | Claim (original location) | Current source | Check | Outcome |
| --- | --- | --- | --- | --- |
| 16 | `sum_layout_selection.rs` has 29 tests (record §4, "unsure") | `S04/preparation/sum_layout_selection.rs` | `grep -c '#\[test\]'` gives 25, as the digest (line 247, §10 item 11) found | Duplicate of the digest correction |
| 17 | Who calls `validate_target` and `validate_call_descriptor_origins` (record "unsure") | `legalization_scalar_graph.rs:9121`; `legalization.rs:990` and `:1015` | `match_input` calls `validate_target`; `derive_expected_functions` calls `match_input` (`legalization.rs:1015`) and `validate_call_descriptor_origins` (`:990`). The other callers are tests (`sum_layout_selection.rs:1005`, `:1043`, `:1072`). | Duplicate of digest line 248, with current lines |
| 18 | Error counts: 186 `custody()` and 152 `unsupported_control_flow` (record "five things" item 2; digest lines 35, 36, 243) | `legalization_target.rs`; `control_flow.rs`, `control_flow_calls.rs`, `control_flow_operations.rs` | The current counts are unchanged: 186, and 79 + 49 + 24 = 152 | Still open: refusals carry no cause. This is diagnostic quality, not a wrong result. |

## Unsupported shapes and duplicate rules

| # | Claim (original location) | Current source | Contract or trace | Outcome |
| --- | --- | --- | --- | --- |
| 19 | Fixed shapes refused with `custody()` or `unsupported_control_flow` (record §1 and §6–8 "cases by shape" and "stated limits"; digest lines 81-85, 93, 235, 237). Examples: `ProgramImageAddress` only on `UefiX64`; `scalar_definitions` only for `IntegerWiden` into 16, 32 or 64 bits; hosted scalar calls only `[i32]`; float views only Binary32 and Binary64; view arrays need a constant extent; `call_result_origins::project` supports three of five origin shapes; structural cases refuse scalar-descriptor pointer slots | the cited ranges in `legalization_target.rs`, `control_flow.rs`, `control_flow_operations.rs`, `call_result_origins.rs` | Each is a closed match whose fallback refuses the program, so an unsupported shape cannot compile into wrong code. Whether each should be supported is a question for its own spec page and release gate. | Feature gaps (refused, not miscompiled) |
| 20 | The lowering rules appear a second time in the audit, and the crate has two dominator implementations (record "same job elsewhere" and "five things" items 1 and 4; digest lines 62 and 64) | `control_flow.rs:75-352` and `legalization_target.rs:696-831`; the twin pairs listed in the record | The audit is meant to re-derive each row independently (`S04/preparation/mod.rs:13-18`). Both dominator implementations treat an unreachable block in their own way, but by receipt 1 neither ever sees one. This is architecture duplication, not a correctness claim. | Intended by design; no correctness action |

## Validation and limits

- The review read the current source at the baseline revision; it built and ran
  nothing. The repository rules forbid running tests outside `mbx`, and the
  task excluded a compiler rebuild. No receipt depends on a new run: each one
  cites the code path or check that decides it.
- No ordinary language defect was found, so this packet adds no reproduction
  and no code change. The difference in receipt 3 and the insertion paths in
  receipt 13 are cleanup candidates. Neither changes a verdict.
- The trace in receipt 1 assumes the unit stage 04 receives is the one stage 03
  validated. `OptimizedAbstractProgram` has public fields
  (`representations/abstract-operations/optimization-unit/src/lib.rs:312-322`),
  so a hand-built unit in a test could skip that validation. Production builds
  the unit only in `03_.../abstract_optimization/publication.rs:294`, after
  validation.
