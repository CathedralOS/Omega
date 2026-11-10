# Area 08 open accepts — trace

Read-only. Citations match the checkout, which is unchanged from atlas commit `457adf4a11f` for every file named here (`git diff --stat 457adf4a11f HEAD` on the stage 04 and stage 05 pipeline directories was empty). Each bullet under "Classed open" in `.agents/atlas/compiler-2026-11-10/digests/area_08.md` is one entry, in that order. One verdict per bullet.

| # | file:line | verdict | reason |
| --- | --- | --- | --- |
| 1 | `preparation.rs:47-61` | UNDECIDED | A non-usize array extent stays an expression; the later unit planner turns that into a gap, and I did not read whether a gap rejects the program. |
| 2 | `preparation.rs:138` | CLOSED | `None` and `ArrayLength` recipes never reach the arm: their expression handle is invalid. |
| 3 | `preparation.rs:1452` | CLOSED | Checking rejects a package-authored selection that is still late-bound, except the exemptions that function names. |
| 4 | `selection_and_reflection.rs:1087`, `:1103` | UNDECIDED | A same-spelling binder is recorded with no symbol check; I did not read the substitution that applies the binding. |
| 5 | `borrow.rs:4765-4775`, `:4891-4893`, `:7764`, `:7801-7803` | UNDECIDED | A template-dependent stored-slot relation yields no loans; I did not read whether that relation can still be present in a checked body. |
| 6 | `borrow.rs:1475-1479` | UNDECIDED | Missing call frames skip custody, and the diagnostics that cause missing frames are thrown away in that builder. |
| 7 | `borrow.rs:1058`, `:1174` | UNDECIDED | An unrecognized call target records a plain read; I did not read a prior reject of that target. |
| 8 | `borrow.rs:2452-2519` | CLOSED | The only caller turns `None` into an error. |
| 9 | `checking.rs:329-334`, `:47-54` | UNDECIDED | A call with no operational row is not compared to its acknowledgement; I did not read how the plan is filled. |
| 10 | `checking.rs:995-997` | REACHABLE | With no package authority, a non-toolchain file can select another package's private declaration and get no diagnostic. |
| 11 | `checking.rs:408-412` | CLOSED | The product check uses the settled request, which requires the Copy receipt. |
| 12 | `checks.rs:2602-2603` | UNDECIDED | Nesting past 16 makes a region comparison succeed; the comment gives that job to the borrow checker, which I did not read. |
| 13 | `checks.rs:4167-4169` | UNDECIDED | An expression with an empty static-world list skips the index check; I did not read who writes an empty list. |
| 14 | `checks.rs:4205-4210`, `:4294-4296`, `:4297-4305` | UNDECIDED | A missing machine, parameter roster, or call site skips the runtime-index check; I did not read whether that absence is already an error. |
| 15 | `checks.rs:4269` | UNDECIDED | An assignment whose target has no `actual` skips the value check; I did not read `actual`. |
| 16 | `checks.rs:3328-3330`, `:1902`, `:1926-1935` | UNDECIDED | An invalid symbol counts as a stable index and as const; I did not read whether such a symbol can sit in these expressions. |
| 17 | `checks.rs:5750-5753` | UNDECIDED | A missing statement is treated as "not used after"; the caller ORs another test I did not finish. |
| 18 | `checks.rs:6972-6977` | UNDECIDED | A loan that has a transport supplier adds no resource row; I did not read the consumer of a successful return. |
| 19 | `checks.rs:6299-6301` | UNDECIDED | An empty borrow-path list is view-free; I did not read `borrow_carrying_owner_paths`. |
| 20–84 | remaining open bullets | not reached | Not opened. |

## 1. `preparation.rs:47-61` — UNDECIDED

Read `array_extents::normalize` in full (`omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/preparation.rs:15-66`). On every failure it `continue`s and leaves `FixedArrayLength::Expression` in place. Search `array_extents::normalize`: 3 call sites, all in this file (`:2033` in `prepare_typed_trees`, `:2095` in `prepare_execution_applications`, `:2119` in `prepare_constant_execution`). The function returns `()`; none of the callers inspect a failure.

Read `validate_symbolic_array_length` (`validation/src/type_references_and_wire.rs:1564-1701`). An expression length whose result type accepts an integer literal is not diagnosed (`:1621-1660`), including a negative signed value. Read `declared_constant_extent` (`typed-trees/src/access_views_and_commitments.rs:2005-2024`): a const expression that is not a `u64` yields `None`. Read `const_length_extent` and the `?` in `collect_type` (`execution_terminal_unit_types.rs:3007-3008`, `:3551-3566`), then `add_type` (`:3347-3369`). Read the statement-sequence `?` (`execution_terminal_unit_control_statement_sequence.rs:9308`) and `plan_checked_body` turning `None` into `Err(gap)` (`execution_terminal_unit.rs:8096-8116`). Read `gap_diagnostic`: an ordinary gap is `Diagnostic::incomplete`, not an error (`checked-trees/src/flow_terminal_units.rs:1655-1664`).

Not read: the function that decides whether a nonempty gap list fails the compile, and whether every array type is visited by `validate_symbolic_array_length`. What would decide it: that consumer of `EffectPlans.gaps`, plus one local whose type is a fixed array of a negative integer expression, followed until success or a diagnostic.

Digest: "later rejection not traced" matches what I found; it is not a wrong description of the site.

## 2. `preparation.rs:138` — CLOSED

Read `constant_entries::materialize` in full (`preparation.rs:75-171`). Search `constant_entries::materialize`: 1 caller, `prepare_typed_trees` at `:2009`.

The `_ => false` arm sets `live` false, and `:140-146` then skips the context. Read `RetainedConstantPosition` and `expression` (`typed-trees/src/access_views_and_commitments.rs:1847-1868`). `None` and `ArrayLength` return an invalid expression handle. `materialize` continues at `:111-113` before the match whenever that handle is invalid, so those two variants do not reach `:138`. The two variants that return a valid expression are the two arms above the `_`. Read the `ArrayLength` constructor (`build-time-evaluation/src/const_evaluation.rs:3256-3266`): `contexts` is the default empty span, so there is no context entry to install on that path either.

Digest is wrong about the arm being the reason those positions get no entry. They are excluded earlier because they have no expression.

## 3. `preparation.rs:1452` — CLOSED

Read the loop in `bind_pre_specialization_authored_selections` (`preparation.rs:1365-1479`, function starts at `:1365`). When the late-bound target does not resolve, `:1452-1454` `continue`s and the selection stays late-bound. Search `finalize_checked_authored_selections`: defined at `05.../shared.rs:973`, called once at `checking.rs:1214-1218` on the success path before checked trees are returned.

Read the gate (`shared.rs:1383-1422`) and its header comment (`:919-925`). A remaining late-bound selection whose source is not toolchain, not a descriptor occurrence, and not a derived wire codec is `Diagnostic::error`. Read `descriptor_selection_occurrences` (`validation/src/frozen_program_and_queries.rs:2079-2092`), `checks_authored_selection` (`typed-trees/src/lib.rs:1561-1576`), and `selection_is_derived_wire_codec` (`shared.rs:1446-1469`). With no product-declaration frontier, `checks_authored_selection` is false only for the excluded-occurrence set. The frontier is installed by build-time evaluation (`build-time-evaluation/src/lib.rs:172`), which the comment names as staying late-bound for the build that binds it. Toolchain origin is the other named skip (`shared.rs:1394-1397`).

Not read: who fills `excluded_authored_selection_occurrences` (it is preserved across `rebuild_fresh_typed_trees` at `03.../lowerer.rs:2147` and `:2192`). Every path I read either errors or is one of those named skips.

## 4. `selection_and_reflection.rs:1087` and `:1103` — UNDECIDED

Read `infer_static_bindings` from `:1066` through the name match and the structural recursion (`:1066-1174`). Both the type-parameter test (`:1082-1087`) and the const-parameter test (`:1097-1103`) succeed when `parameter_name == name.as_str()` even if both symbols are valid and different. The middle clause (both symbols invalid) is redundant with that last clause. Search `infer_static_bindings(`: the definition, recursive calls in the same function, and calls at `selection_and_reflection.rs:261`, `:263`, `:1451`, `:1484`, `contextual_and_requirement_calls.rs:985`, `:1571`, `:3595`, and `constructed_results.rs:1035`. I read the call at `:253-268` (zip of a requirement signature against an actual state's parameters). I did not read the other three files' call sites.

Read `proposal_bindings` (`:716-849`). A proposal is stored by parameter index. Disagreement on the same slot sets `conflicted`. Nothing there checks that the formal's symbol is the parameter's symbol. Read `CallSelection::has_bound_arguments` (`monomorphization.rs:3728-3734`): a non-conflicted full binding is complete.

Not read: the substitution that writes `type_bindings` into the callee, and any later comparison of each argument's type to that signature by symbol. What would decide it: that substitution. If it replaces by symbol, a concrete type that only shares the parameter's spelling is not rewritten and a later type comparison can still reject the call. If the binding is trusted with no second comparison, a generic whose formal type is a different declaration with the same spelling would be instantiated from the argument and accepted.

The digest's description of the disjunct is right. I read it.

## 5. `borrow.rs:4765-4775`, `:4891-4893`, `:7764`, `:7801-7803` — UNDECIDED

Read `stored_calls::roster_at_expression` (`borrow.rs:4748-4776`) and `independent_owned_formation` (`:4821-4872`). `TemplateDependent` returns an empty roster only when that predicate is true: the expression's type is a reference or slice view and the geometry's places are owned. Read `stored_calls::loans` (`:4875-4896`): any `TemplateDependent` returns `Ok(Vec::new())`, with no owned-formation test. Search `stored_calls::loans(`: one caller, `statement_borrow_loans` at `:7669`, which I read through `:7682`. Read the two `roster_at_expression` arms in `binding_borrow_loans` (`:7754-7806`): `:7764` skips one initializer; `:7801-7802` returns no loans for the local. Search `roster_at_expression(`: the definition plus `:7391`, `:7754`, `:7791`, all in this file.

Not read: `stored_reference_fallthrough_effects` / `stored_reference_effects_at_expression`, and whether a checked body can still carry `TemplateDependent` after stage 04 specialization. What would decide it: the producer of `StoredReferenceRelationRefusal::TemplateDependent` and whether it is reachable once a call target has been specialized.

## 6. `borrow.rs:1475-1479` — UNDECIDED

Read `append_reference_custody` from its comment through the `None` return (`:1458-1479`). Search `append_reference_custody(`: one caller, `:8685`. Read `:8685-8708`: `None` adds no custody rows and the statement continues. Read `build_borrow_facts` (`:9434-9440`) and its only caller `build_check_facts` (`published_facts.rs:10982-10983`), which passes `CallFrameResolver::new(...).as_ref()` even when that is `None`. Read `CallFrameResolver::new` (`validation/src/calls.rs:15337-15348`) and `CallFrameTables::build` (`:14075-14093`). `build` returns `None` when `TopLevelSymbols::build` pushes any diagnostic, and that local `diagnostics` vector is dropped.

Not read: the other callers of `TopLevelSymbols::build` and whether the same diagnostic fails checking. What would decide it: if those diagnostics are also returned as errors before or after `build_check_facts`, the skip is harmless; if this is the only place they are produced, a program with those symbol errors is checked with no call frames and no custody rows.

## 7. `borrow.rs:1058`, `:1174` — UNDECIDED

Read `collect_target_argument_accesses` (`:1027-1066`) and `collect_formal_argument_evaluation` through `collect_single_expression_argument_access` (`:1069-1194`). An unknown formal uses `unwrap_or_default()` (`:1058`), so `formal` is invalid, the implicit-loan closure returns `None` at `:1151-1152`, and a non-borrow argument is recorded as a read (`:1174`). Read `call_target_parameters` (`validation/src/calls.rs:40802-40826`): `None` unless the target is a dynamic slot, a state, a machine-parameter signature, an operator, or a selected requirement signature. Search `collect_target_argument_accesses(`: two callers, `borrow.rs:718` and `:961`. I read both call sites (`:707-719` and `:931-959`). Neither requires the target to be one of those five kinds.

Not read: a diagnostic that rejects a call whose target is none of those five, and whether it runs before `published_facts.rs:10983`. What would decide it: that diagnostic's place in the checking order.

The digest's sentence about an unresolved `&mut` formal being recorded as a read matches the branch I read.

## 8. `borrow.rs:2452-2519` — CLOSED

Read `lifetime_contract::source_type` (`borrow.rs:2446-2519`; the function continues past `:2519`, and I read through the mismatch returns in that range). Every mismatch returns `None`. Search `lifetime_contract::source_type(`: 1 caller, `checks_borrows_resources.rs:6679`. Read `plan` there (`:6656-6687`): `None` becomes `Err("completed output lifetime input does not match its captured call argument")` via `?`. No other caller. The digest said consumers were not read; the function does return `None` on mismatch, and that one consumer rejects it.

## 9. `checking.rs:329-334` and `:47-54` — UNDECIDED

Read `validate_call` (`checking.rs:318-358`) and the loop in `validate_call_acknowledgements` (`:39-70`). If no operational row has the same statement index and call ordinal, `validate_call` returns without comparing the acknowledgement to the envelope. Search `validate_call_acknowledgements(`: 1 caller, `published_facts.rs:11230`. Read `:47-54`: a state with no operational summary is `continue`d, so its calls are not visited.

Not read: the builder of `OperationalPlan` and whether every source call gets a row. What would decide it: that builder. A call that is absent from the plan is not checked here; if the plan is complete by construction, the return is harmless.

## 10. `checking.rs:995-997` — REACHABLE

Read `admit_request` (`checking.rs:991-999`) and `CheckingRequest` (`:755-775`). `validate_authored_declaration_selections` runs only when `declaration_authority` is `Some`. `new()` sets it to `None`. Search `with_declaration_authority(`: one call, `psi-compiler/src/source_checking.rs:702-704`, passing `SourceAdmission.declarations`. That field is `Option` and `Default` (`source_checking.rs:599-609`). Search `SourceAdmission {`: two constructors. The product check sets `declarations` from `package_inputs` (`omega/.../checked/checking/source_checking.rs:74-75`), so `None` package inputs leave authority unset. The bundled-core defaults build passes `package_inputs: None` (`omega/.../checked/checking/source_build.rs:70-72`). Build-time evaluation uses `CheckingRequest::new()` with no authority (search `CheckingRequest::new(`: `const_initializers.rs`, `machine_execution.rs`, `const_evaluation.rs`, and `source_checking.rs:702` before `with_declaration_authority`).

Read `validate_authored_declaration_selections` and `normalize_checked_selection` (`validation/src/declarations_core.rs:905-950` and `:988-1053`). A non-toolchain late-bound selection is an error string (`:1009-1013`), and a private declaration selected from another package is an error string (`:1045-1052`). Those strings become diagnostics only inside `validate_authored_declaration_selections` (`:945-947`). Search `checked_selection_requirements(`: stored into facts at `published_facts.rs:4579` and not otherwise diagnosed. `finalize_checked_authored_selections` (entry 3) rejects a still-late-bound package selection, so this hole is the resolved selection: private, other package, not toolchain.

Smallest program: a source file that is not compiler toolchain source, selecting a private declaration that belongs to a different package, compiled with no package authority. The selection is kept and no diagnostic is emitted. That is the default checking request, including a product check whose package inputs are absent.

Digest: "open for the rest" is right for that path. The toolchain comment at `checking.rs` was not what I used; the skip is the `if let Some(authority)` itself.

## 11. `checking.rs:408-412` — CLOSED

Read `prepare_typed_program` (`checking.rs:399-412`). `allow_pending_opaque_copy` selects `PendingBuildSelection`, which passes `allow_pending_opaque_copy: true` into `validate_data_properties` (`program_validation.rs:546-557`). Read `validate_opaque_data_properties` (`proof_contracts_core.rs:8182-8209`): the missing-Copy-receipt diagnostic is skipped only when that flag is true (`:8188-8190`).

The flag is `admits_pending_inputs` (`checking.rs:1047-1051`). Read `PreparationRequest` (`preparation.rs:1836-1851`): `settled()` is false, `preliminary()` is true. Search `PreparationRequest::settled`: the product admission uses it (`psi-compiler/src/source_checking.rs:695`). Search `PreparationRequest::preliminary`: `machine_execution.rs:640` and `:719` (a generated constant-invocation probe) and `const_evaluation.rs` test expects. The product judgment does not take the pending branch, so the receipt check runs. A probe that skips it is not the product result; the same data is still under the settled check.

## 12. `checks.rs:2602-2603` — UNDECIDED

Read `region_operand` (`checks.rs:2565-2606`) and the recursive call at `:2627` (`depth + 1`). `depth > 16` returns `Region::Inferred`. Read `compare` (`:2846-2849`): `(Some(_), Some(_)) => true`, so an `Inferred` region matches without comparing identities. The comment on `Region::Inferred` (`:1858-1861`) and on `region_operand` (`:2565-2576`) says a rowless call's regions are left to the borrow checker. The depth cutoff is inside the rowless-call arm (`:2585-2592` returns earlier when the call has lifetime rows).

Not read: a borrow check that still compares those regions after this function has stopped walking. What would decide it: that check, on a rowless call whose region nesting is deeper than 16 and whose two sides name different regions.

## 13. `checks.rs:4167-4169` — UNDECIDED

Read `check_completed_value` (`checks.rs:4161-4183`). If `static_type_context_is_unreachable` is true, the index-subject check returns. Read that method (`typed-trees/src/type_system.rs:1682-1683`) and `is_unreachable` (`:760-766`): true when the expression has a row whose world list is empty.

Not read: the writer of those rows. What would decide it: whether an empty world list is stored only for an expression that does not execute.

## 14. `checks.rs:4205-4210`, `:4294-4296`, `:4297-4305` — UNDECIDED

Read the runtime-index walk (`checks.rs:4192-4305`). No data type parameter of const or value kind returns immediately (`:4200-4201`). A flow row whose machine or state is missing is `continue` (`:4205-4210`). A call with no parameter roster (`:4294-4296`) or no call site (`:4297-4305`) is `continue`, so that call gets no index check.

Not read: the producer of flow states and call rows, and whether a missing machine, roster, or site is already a diagnostic. What would decide it: those three lookups' failure modes on a call that passes a runtime index.

## 15. `checks.rs:4269` — UNDECIDED

Read the assignment arm (`checks.rs:4263-4271`). `scope.check` on the value runs only when `scope.actual(assignment.target)` is `Some`.

Not read: `actual`. What would decide it: when `actual` returns `None` for a target that still carries a runtime index.

## 16. `checks.rs:3328-3330`, `:1902`, `:1926-1935` — UNDECIDED

Read `stable_index_symbol` (`checks.rs:3327-3332`): an invalid symbol returns `true` before any const or field test. Read `expression_is_const` (`:1896-1934`): `is_const_symbol` is true for an invalid symbol (`:1902-1903`), and a call argument with an invalid symbol is const (`:1926-1928`).

Not read: the callers of these two functions and whether an invalid symbol can appear in a proof-index or const-index expression that has already passed name resolution. What would decide it: one caller and the earlier reject, if any, of an unresolved name in that position.

## 17. `checks.rs:5750-5753` — UNDECIDED

Read `place_is_used_after_call` (`checks.rs:5742-5753`): a missing statement returns `false` ("not used after"). Search `place_is_used_after_call(`: one caller, `:7804`, inside `append_live_value_carry_demands`. Read `:7793-7814`: the symbol is live if `place_is_used_after_statement` is true or this function is true. A missing statement makes only the second test false.

Not read: `place_is_used_after_statement` on an out-of-range index, and whether `statement_at` can fail for a call that is in the borrow facts. What would decide it: `statement_at` and that other predicate.

## 18. `checks.rs:6972-6977` — UNDECIDED

Read the match (`checks.rs:6961-6984`). The arm `_ if transport_suppliers(...).is_some() => {}` accepts the loan without pushing a resource. The next arm returns `Err` when none of the kinds match. I did not read `transport_suppliers` or the function that consumes a successful `Ok` from this walk.

What would decide it: whether a loan that takes this arm still has a resource row from an earlier arm, or the success path requires one of the three kinds.

## 19. `checks.rs:6299-6301` — UNDECIDED

Read the return (`checks.rs:6294-6301`). `borrow_carrying_owner_paths` yielding `Some` of an empty list returns `SuspensionValueBorrowFrontier::ViewFree`. `None` is `Unresolved` (`:6296-6297`). I did not read `borrow_carrying_owner_paths` or the consumer of `ViewFree`.

What would decide it: whether that function returns an empty list for a type that still holds a borrow path, and whether `ViewFree` skips a suspension check.

## Counts

- REACHABLE: 1 (entry 10)
- CLOSED: 4 (entries 2, 3, 8, 11)
- UNDECIDED: 14 (entries 1, 4, 5, 6, 7, 9, 12, 13, 14, 15, 16, 17, 18, 19)
- Not reached: 65 bullets, entries 20 through 84

## Entries not reached

Not opened. Listed in digest order, starting at the next bullet after entry 19:

- `checks.rs:7448-7457`, `:7460-7469`, `:7708-7714`
- `checks.rs:7289-7291`
- `checks.rs:8633-8639`
- `checks.rs:8959-8961`, `:8940-8946`
- `checks.rs:10374-10381`, `:10798-10800`
- `checks_borrows.rs:439`, `:446`, `:555`
- `checks_borrows.rs:1447-1449`
- `checks_borrows.rs:5393-5408`
- `checks_borrows.rs:5502-5509`
- `checks_borrows.rs:11434-11435` and `checks_borrows_calls.rs:1497`
- `checks_borrows_calls.rs:1079-1085`
- `checks_borrows_overlap.rs:2828`
- `checks_borrows_overlap.rs:2989-2992`
- `checks_borrows_resources.rs:100`, `:129-131`
- `checks_borrows_resources.rs:2940-2953`
- `checks_borrows_resources.rs:5773`
- `checks_borrows_resources.rs:4001-4013`
- `checks_content.rs:2283-2285`
- `checks_content.rs:2286-2288`
- `checks_content.rs:2307-2317`
- `checks_content.rs:1258`
- `checks_content.rs:5395-5409`
- `checks_content.rs:6270`
- `checks_content.rs:6005`, `:6011-6019`
- `checks_content.rs:3834`, `:3897-3910`
- `checks_contracts.rs:1201-1209`
- `checks_contracts.rs:244-251`, `:255-261`
- `checks_contracts.rs:1566-1587`, `:690-696`
- `checks_contracts.rs:2873`, `:2876`
- `checks_contracts_calls.rs:3407-3474`
- `checks_contracts_calls.rs:4542-4560`, `:4617-4621`, `:4638-4652`
- `checks_contracts_calls.rs:4175-4178`
- `checks_contracts_exits.rs:9696`, `:9713`
- `checks_contracts_exits.rs:9655-9659`
- `checks_contracts_exits.rs:4037`, `:4042`
- `checks_contracts_exits.rs:2079-2088`, `:2096`
- `checks_contracts_writes.rs:1590-1594`, `:205-214`
- `checks_contracts_prover.rs:2836-2845`
- `checks_contracts_prover.rs:4667-4676`
- `checks_contracts_prover.rs:5557`
- `checks_contracts_prover.rs:7703`
- `checks_contracts_prover.rs:7615-7623`
- `checks_contracts_prover.rs:7384-7386`
- `checks_contracts_prover.rs:7863-7886`
- `checks_current_scalar_values.rs:5073`, `:4820-4822`
- `checks_current_scalar_values.rs:604-610`, `:1994`
- `checks_multiplicity.rs:7218`, `:7518`
- `checks_multiplicity.rs:9802-9937`
- `checks_multiplicity.rs:10872`
- `checks_multiplicity_borrowed_windows.rs:1498-1513`
- `checks_multiplicity_borrowed_windows.rs:565-567`, `:1368-1370`, `:1433-1435`, `:1480-1482`, `:960-962`
- `checks_multiplicity_claim_outcomes.rs:2167-2187`, `:1678-1686`, `:1704-1706`, `:1839`, `:1851-1855`
- `checks_multiplicity_cleanup_dispositions.rs:822-833`, `:722-727`
- `checks_multiplicity_linear_validation.rs:489-494`, `:574-579`, `:679-684`, `:3654`, `:4472-4525`
- `checks_multiplicity_permission_events.rs:1961`, `:1972`
- `checks_operators.rs:1856`, `:190-192`, `:251-255`, `:332`
- `checks_ranges.rs:6074-6079`, `:6091`
- `checks_ranges_facts.rs:4830-4833`
- `checks_ranges.rs:11163-11230`
- `checks_ranges_facts.rs:3717`
- `checks_termination.rs:6729-6731`, `:7903-7918`
- `execution.rs:817-821`
- `execution.rs:547-550`
- `execution.rs:2565-2580`
- `execution.rs:292-301`

## What I did not read

- Any function in `checks_borrows.rs`, `checks_borrows_calls.rs`, `checks_borrows_overlap.rs`, `checks_content.rs`, `checks_contracts.rs`, `checks_contracts_calls.rs`, `checks_contracts_exits.rs`, `checks_contracts_writes.rs`, `checks_contracts_prover.rs`, `checks_current_scalar_values.rs`, `checks_multiplicity.rs`, `checks_multiplicity_borrowed_windows.rs`, `checks_multiplicity_claim_outcomes.rs`, `checks_multiplicity_cleanup_dispositions.rs`, `checks_multiplicity_linear_validation.rs`, `checks_multiplicity_permission_events.rs`, `checks_operators.rs`, `checks_ranges.rs`, `checks_ranges_facts.rs`, `checks_termination.rs`, or stage 05 `execution.rs`, except the two `checks_borrows_resources.rs` sites named under entries 6 and 8 (`CallFrame` was not in that file; `source_type`'s caller was).
- The rest of `source_type` after `borrow.rs:2519`.
- `independent_owned_formation` after `:4872`.
- Who writes `excluded_authored_selection_occurrences`, who consumes `EffectPlans.gaps`, who builds `OperationalPlan`, and who substitutes `CallSelection.type_bindings`.
- The other `infer_static_bindings` call sites outside `selection_and_reflection.rs`.
- Unit records, except the fail-open list in `records/unit_046.md` used to confirm the digest's wording. Verdicts are from the source lines above, not from that record.
