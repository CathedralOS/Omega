# slice_0142

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A1089 | symbols/src/table.rs:1645-1651, 2878-2883, 2588-2600 | REACHABLE | Source-free and empty-span lookups can bind the first of ambiguous same-named declarations without a uniqueness failure. |
| A1090 | CI/interpreter.rs:772-779, 1102-1106; CI/evaluation.rs:323-337 | REACHABLE | Trap, unsupported, resource, setup error, worker panic, and `error_outcome` all surface as `InterpretOutcome` with `exit_code` 0 while `error` is set. |
| A1091 | CI/build_machine.rs:15-35, 758-821 | REACHABLE | `accept_component_assumption` evaluates the digest and returns unit without recording or comparing it; post-completion mutation tracking only follows rooted-path operands, not arbitrary descriptor writes. |
| A1092 | CI/execution.rs:594-693, 1013-1025, 1211-1254, 1391-1471 | REACHABLE | Typed evaluator (`checked == None`) skips checked cleanup plans; `ReadLine` can omit a failed destination write; non-`CheckedShape` build-time arguments skip shape binding. |
| A1093 | CI/execution_expressions.rs:912-1000 | REACHABLE | After failed static resolution, listed unqualified callee spellings return synthetic values instead of halting. |
| A1094 | CI/execution_scalars.rs:143, 708-774, 1615-1621; CI/execution.rs:324-329 | REACHABLE | Non-boolean casts to `Bool` become false; statement-position operator calls pass an invalid application handle and skip checked-selection cross-check; enum equality compares variant names only. |
| A1095 | CI/filesystem_real.rs:106-118; CI/build_machine.rs:985-991 | REACHABLE | When `descriptor_root` is `None` (unscoped real run or unknown fd), the facet-root trap is skipped and the descriptor operation proceeds. |
| A1096 | CI/filesystem_real.rs:1268-1290; CI/filesystem.rs:3880-3885 | REACHABLE | `real_seek` maps any `whence` other than 1 or 2 to start-relative seek; the virtual provider rejects other `whence` values with `None`. |

## A1089

Digest cited `table.rs:2547` for `find_child_by_name`; re-located at `omega-rust/psi/foundation/symbols/src/table.rs:2588` (read).

**`same_product_package_instance` (1645-1651, read):** When `self.sources` is `None`, the function returns `true` for every `(reference, declaration)` pair without consulting package or scope. It is used from `select_product_declaration_candidate` (1550-1553, read) and from validation (`frozen_program_and_queries.rs:3559`, `validation/src/lib.rs:635`, grep count 2 production sites besides the definition, read). `source_reference_can_describe_product_symbol` (1609-1614, read) likewise treats missing sources as visible when `!has_source_metadata()` (1716-1718, read). Input: symbol table built without a `SourceMap` (comment 1624-1625 documents source-free lowering for representation tests).

**Empty-span `Ordinary` top-level lookup (2878-2883, read):** In `lookup_top_level_candidates`, `reference_is_source_backed` is false when `reference.span.start == reference.span.end` (2843, read). For that case with `TopLevelSelection::Ordinary`, the candidate list is truncated to one entry before `lookup_top_level` calls `SymbolLookup::from_candidates` (2829-2831, 1991-1999, read), so two distinct same-named declarations never become `Ambiguous`. Callers: `lookup_top_level_by_name_and_kinds_from_source_matching` at 2663-2676 (grep `\blookup_top_level_by_name_and_kinds_from_source_matching\b` in `omega-rust`: 17 matches across 9 files, read). Input: internal resolution with an empty `SourceSpan` and two eligible top-level declarations of the same spelling.

**`find_child_by_name` (2588-2600, read):** Returns the first name-matching selected child; ambiguity is enforced only when the parent kind is `Module` (2597-2598, read). Used from `find_descendant_by_path` / `resolve_child_path` (3149-3151, 3322-3324, read) and from stage-02 symbol assignment (`find_child_by_name_and_kind` grep: 6 production call sites in `omega-rust`, read). Input: a machine or data scope with two same-named child symbols where the parent is not a module; path resolution binds the first.

Verdict **REACHABLE**: each mechanism can complete resolution as `Unique` or pick a first child where the model expects rejection or ambiguity.

## A1090

Read `interpret_entry` (`interpreter.rs:765-779`): worker panic is caught and turned into `InterpretOutcome::error` with no non-zero exit code.

Read `interpret_on_current_thread` result match (`interpreter.rs:1093-1106`): `Halt::Trap`, `Unsupported`, and `Resource` map to `InterpretOutcome::error`; `error_outcome` paths at 1013, 1028, 1044, 1060, 1072, 1078 use the same constructor.

Read `InterpretOutcome::error` (`evaluation.rs:323-336`): sets `exit_code: 0` and `error: Some(message)`.

Read `InterpretOutcome` struct (`evaluation.rs:297-298`, read): public `exit_code` field is what a caller inspects.

No caller in `omega-rust` rewrites `exit_code` when `error` is `Some` (grep `InterpretOutcome` in `omega-rust`, read).

Verdict **REACHABLE**: any interpreted run that traps, hits an unsupported construct, exhausts a resource budget, fails pre-run validation via `error_outcome`, or panics on the interpreter thread yields success exit code 0 with a populated `error` field.

## A1091

Read `try_component_assumption_acceptance` (`build_machine.rs:15-35`): requires the Build facet method `accept_component_assumption`, evaluates builder and digest expressions, returns `Value::Unit` without comparing digest bytes to any stored assumption.

Read dispatch (`build_machine.rs:74`, read): routed from `try_build_facet_routes`.

Read `note_completed_build_output_attempt` (`build_machine.rs:758-821`): records post-completion mutations only for paths discovered via `rooted_path_operand_resolutions` or tracked open writers from rooted opens; descriptor-only writes without rooted path resolution are not added to `mutated`.

Verdict **REACHABLE**: a build-time machine that calls `accept_component_assumption` with an arbitrary digest spelling is accepted and returns unit; output completed through a borrowed or duplicated descriptor without a rooted path operand is not counted as a post-completion mutation.

## A1092

Read `Evaluator::new` / `new_build` (`execution.rs:2237-2243, 2217-2221`, read): `BuildMachineProgram::Typed` leaves `checked: None`; only `Checked` sets `checked` and `operator_facts`.

Read `normal_cleanup_actions` (`execution.rs:1221-1222`, read): returns empty `Vec` when `self.checked` is `None`, so nominal affine cleanup plans are never collected on the Typed path.

Read `execute_completion_cleanup` (`execution.rs:1399-1469`, read): the checked-tree branch is gated on `self.checked.and_then(...)`; the fallback calls `normal_cleanup_actions`, which is also empty when `checked` is absent. Call sites at 2952-2986 (read) run on state completion.

Read `ReadLine` (`execution.rs:1013-1025`, read): after `read_stdin_line`, destination write runs only inside `if let Ok(cell) = self.resolve_place(...)`; a failed resolve still returns `Ok(Value::Bool(!line.is_empty()))`.

Read `bind_build_time_argument` (`execution.rs:619-623, 690-692`, read): returns `Ok(())` when `supply_mode != CheckedShape` and when the value is neither struct nor enum after the early matches, so shape binding is skipped.

Verdict **REACHABLE**: Typed-tree interpretation omits affine cleanup; a console read line with an unresolvable destination still succeeds; a build-time argument whose type is not `CheckedShape` is accepted without field or case binding.

## A1093

Read `resolve_static_call` failure branch (`execution_expressions.rs:896-1001`, read): for unqualified calls (`qualified` false at 908), a fixed list of callee spellings returns fabricated integers or window handles; `read_byte` reads stdin; `blit` returns a cloned argument; only unmatched names fall through to `Err(halt)` at 1001.

Called from expression evaluation (`execution_expressions.rs:896`, read) on every unresolved static call attempt.

Verdict **REACHABLE**: a user machine that calls one of the listed names without a resolved declaration is still executed and receives a synthetic value instead of rejection.

## A1094

Read `cast` to `PrimitiveType::Bool` (`execution_scalars.rs:143`, read): `value.as_bool().unwrap_or(false)` makes any non-boolean operand become false rather than trapping.

Read `values_equal` on enums (`execution_scalars.rs:1615-1621`, read): compares `variant_name` strings only, ignoring `type_symbol`.

Read `selected_realization` (`execution_scalars.rs:720-742`, read): checked-selection cross-check runs only when `application.is_valid()`.

Read `try_selected_named_call` (`execution.rs:324-329`, read): passes `ExpressionHandle::invalid()` into `try_operator_application`, so statement-position operator calls skip that cross-check while `operator_facts` may still be present on checked runs.

Verdict **REACHABLE**: equality can treat variants of different enum types as equal when names match; casting a non-boolean to bool yields false; a statement-level operator call can run the selected realization without verifying it matches the checked expression selection.

## A1095

Read `RealFs::descriptor_root` (`filesystem_real.rs:106-118`, read): returns `None` when the descriptor is not captured and either `grants` is `None` (unscoped) or the fd is absent from `files`.

Read Build path preparation (`build_machine.rs:985-991`, read): for Read/Write/Close, `trap` runs only when `descriptor_root(*fd)` is `Some(issued)` and `issued != root`; if `descriptor_root` is `None`, the check is skipped and `Ok(call)` is returned.

Read interpreter setup (`interpreter.rs:1051-1054`, read): `FilesystemAccess::RealUnscoped` constructs `RealFs::new(None, None)`.

Verdict **REACHABLE**: in an unscoped real filesystem run, a Build descriptor operation on an fd that does not resolve to a grant root is not rejected for belonging to another facet.

## A1096

Read `real_seek` (`filesystem_real.rs:1268-1290`, read): `whence` 1 is current, 2 is end, and all other values including 0 and 3+ use `SeekFrom::Start(offset.max(0) as u64)`.

Read `virtual_seek` (`filesystem.rs:3880-3885`, read): only `whence` 0, 1, and 2 are accepted; any other value returns `None` (error).

Verdict **REACHABLE**: the same seek operation with an unsupported `whence` is rejected on the virtual provider but proceeds as start-relative on the real provider, so behavior diverges and the real path does not reject the call.

## What I did not read

- Full bodies of every `lookup_top_level_by_name_and_kinds_from_source_matching` call site in `symbols.rs` (eight call sites; only the table API and `lookup_top_level` chain were read in full).
- Terminal interpreter or compiler validation paths outside the cited lines for these rows.
- Runtime consumers of `InterpretOutcome` outside `omega-rust` (e.g. CLI wrappers in other crates).
- `wiki/spec/build/scoped_execution.md` quoted in comments at `table.rs:1623`.
- Git object `457adf4a11f` (current checkout lines were read directly; digest line 2547 for `find_child_by_name` was re-located to 2588).

SLICE COMPLETE
