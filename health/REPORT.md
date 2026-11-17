# Main health inventory at e47f05f7

On `main` at `e47f05f7` (2026-11-16, "docs: list every form the integer kernel
rules accept (#3840)"), **244 of 7126 Rust tests fail** and **35 of 104
`tests/fixtures/run/**/main.omg` programs fail `omega run --both`**. Two of
those 35 are library packages, not programs. One PR causes most of it:
**#3352** (`5f8be54a`, "compiler: retain stored machine selections and public
call contracts", the second commit after the last green post-merge commit
`b30b6519`). 136 of the 168 tests that passed at `b30b6519` already fail at
`5f8be54a`. **#3855** (`fadcf709`, which refuses arm bindings that reuse a
parameter name) caused 25 more: `source/library/core/compact_binary.omg`
`machine copy` still reuses `next`/`remaining_steps`, so every test that types
the real `core` stops there. That error now hides whatever #3352 did to those
tests.

| Status at `b30b6519` (last green) | Tests |
| --- | ---: |
| Passed then, fail at `5f8be54a` (#3352 and its one-line parent) | 136 |
| Passed then and at `5f8be54a`, fail later (25 of them show the #3855 error) | 32 |
| Already failing then (older) | 66 |
| Test did not exist then | 9 |
| Not rerun (`windows_canonicalize_receiver`, 2.4 GB of output) | 1 |

This is a measurement only: no fix, no PR. Twelve PRs landed after the pin,
including #3878 (operation contracts through checking), #3884 (gives the
`terminal-wire` contract test an envelope) and #3886 (std entry schema for an
unlocked run). They may fix some rows below; that is not verified.

## Test failures grouped by cause

The "Regression?" column compares each test with `b30b6519` and with
`5f8be54a` (#3352). "Same message" means the test fails at `5f8be54a` with the
error it shows now.

| # | Group | Tests | Representative test and first error | Likely owner area | Claim / board | Regression? |
| --- | --- | ---: | --- | --- | --- | --- |
| 1 | ARMNS: core `compact_binary::copy` reuses a parameter name in an arm | 36 | `build-time-evaluation source_jobs::tests::callable_queries_use_the_same_landed_application_as_invocation`: ``machine `copy` state `entry` pattern binding `next` conflicts with an existing name: parameters, locals and the bindings of one arm share one namespace`` | `source/library/core/compact_binary.omg:296` (rename the arm bindings); rule from Psi 02 `symbols.rs:2809` | **Unowned** | Regression: 34 passed at b30b6519; 25 broke at #3855, and 9 already failed at #3352 for other reasons that this error now hides |
| 2 | CHKOP: "admitted source selection Operator/Call remains unresolved as CheckedOperator" | 25 | `terminal-psi-to-terminal-psi tests::indexed_contracts::indexed_call_contracts_and_mutable_handback_…`: ``admitted source selection Operator remains unresolved as CheckedOperator in `PathBuffer` at units[4095]`` | Psi 05 `selection_questions.rs`, operator completion | **Claimed**: STD-CONSOLE-CHECKEDOPERATOR-REGRESSION (that claim is bisecting; it starts at #3352: 14 same message, 8 show the earlier form "completed operator … has no exact source owner") | Regression (#3352): 22/22 |
| 3 | ENVELOPE: "named machine has no exact complete operational envelope" (and callable/suspension-envelope variants) | 13 | `compiler::lowered_route_ownership whole_owned_transfers_consume_their_homes_once`: `named machine has no exact complete operational envelope` | lowered-form checker / terminal-wire publication of operational envelopes | Unowned by name; the #3352 author's claim RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY is still open. #3884 (after the pin) fixes only the terminal-wire test | Regression (#3352): 11/11 |
| 4 | PROV: "callable provenance cannot reconstruct current Terminal storage flow" | 12 | `terminal-interpreter engine_execution::capabilities::tests::capability_graph_ignores_unreachable…`: `ScalarApplicationEvaluation("callable provenance cannot reconstruct current Terminal storage flow")` | terminal-interpreter capability graph / boundary supply (also the 3 `stage_validator_requests` tests) | Unowned by name (#3352 author's claim is open) | Regression (#3352): 11/11 |
| 5 | HERMETIC: Build constants or captured declarations refused as "non-hermetic source origin `User`" | 10 | `omega::build_applied_provider_choices a_package_chooses_an_applied_provider_…`: ``source declaration-selection admission: declaration `configure::entry` has non-hermetic source origin `User` `` | build-time-evaluation `source_selection.rs`; typed-trees `type_identity.rs:9238` | **Unowned** | Regression (#3352): 9/9 |
| 6 | STATICARGS: "call static machine arguments while reading the body" | 9 | `compiler::access_view_calls shared_helper`: ``not supported yet: call static machine arguments while reading the body of `observe` `` | Psi 03 typing of static-argument calls | TYPED-STATIC-PARAMETERS (board item, no claim) | **Older**: all 9 failed at b30b6519 (8 with "callee's parameter types typing does not select yet") |
| 7 | GEN: `compiler::lowered_route_generics` | 9 | `a_value_parameter_body_is_checked_once_and_carried`: ``generic machine read without arguments: `Selected::value` `` | Psi generics specialization | **Claimed**: MAIN-REGRESSION-LOWERED-ROUTE-GENERICS | Regression (#3352): 9/9 |
| 8 | GENFAM: finite-family, width-generic and static-implementation-family routes refused | 9 | `compiler::route_probe a_width_generic_body_is_judged_at_every_width`: verdict `["not-supported"]`, expected `["accepted","accepted"]` | same generics mechanism as row 7 | Probably the same as the generics claim (not confirmed) | Regression (#3352): 8/8; 1 older |
| 9 | DIAGTEXT: tests pin old diagnostic wording | 8 | `terminal-psi-to-terminal-psi tests::references::a_selected_reference_result_needs_its_public_requirement`: got ``…different targets in machine `relay` ``, test expects it without the suffix | tests to update (message now names the machine) | Unowned | 4 regressions (#3352), 3 older, 1 new test |
| 10 | STALECOORD: "missing or stale declaration coordinate" / "specialization binding … missing or stale" | 7 | `terminal-psi-to-terminal-psi tests::owned_cleanup::owned_cleanup_requires_are_proved_at_the_exact_…` | source selection / Psi 04 specialization | Unowned | Regression: 4 at #3352, 2 later, 1 older |
| 11 | REVIEWRETAIN: "package review cannot retain the plan's exact authored provider and service selection" | 7 | `compiler::package_review review_policy_round_trip_preserves_authored_reach_slack` | `omega/packages/review-evidence` | Unowned | Regression (#3352): 6/6 (5 showed another message at #3352) |
| 12 | SEALORDER: `Seals(NonCanonicalOrder("provider candidates by exact conformance identity"))` | 6 | `omega::target_selected_implementations each_target_runs_the_implementation_its_build_row_selects` | Build provider publication ordering | Unowned | Regression (#3352): 5/5; 1 older |
| 13 | IDENT: `MalformedIdentity` when reading a rendered identity back (callable contract form) | 6 | `typed-trees type_identity::package_policy_reading_tests::every_type_form_the_renderer_writes_reads_back` | typed-trees `type_identity.rs` reader | Unowned | Regression (#3352): 5/5; 1 new test |
| 14 | IMPORTSEL: "import `vendor::main` does not select one declaration, callable family, or module" | 6 | `omega::validation_subjects the_valid_program_checks` | package import selection | VALIDATION-SUBJECTS (board item) | Older: 6/6 |
| 15 | SVCSCHEMA: "service schema selection does not name one exact declaration application" | 5 | `compiler::package_review native_process_exit_review_preserves_its_source_type…` | provider/service selection | Unowned | Regression (#3352, other message there): 5/5 |
| 16 | SPECCOUNT: generic programs produce more judged parts than the test expects (78 machines where it expects 6; 6 verdicts where it expects 4) | 5 | `compiler::lowered_route_publication the_published_module_states_each_machines_declaration_identity` | specialization roster / test expectations | Unowned | Older: 5/5 |
| 17 | RESDOMAIN: "declared result-domain set does not equal its expected result-domain set" | 5 | `terminal-psi-to-terminal-psi tests::qualified_terms::the_lowered_scalar_type_retains_its_declared_domain` | Psi 03 result-domain selection | NAMED-DOMAIN-CASTS / OPERATION-ARITHMETIC-POLICY area | Older: 5/5 |
| 18 | TYPEDOCC: float conversions "source invocation lost its typed occurrence or original lexical owner" | 4 | `compiler::lowered_route_float_values exact_float_widening_retains_its_unary_contract…` | source declaration-selection admission | Unowned | Regression (#3352): 4/4 |
| 19 | ARMTYPE: process exit in a match arm, "typing arm value type" | 4 | `compiler::route_probe a_program_that_ends_through_process_exit_is_judged` | Psi 03 arm typing for `Never`-like calls | PUBLISHED-NO-RETURN / ENTRY-CONTENT-ROOTS | Older: 4/4 |
| 20 | UNKNOWNCALL: Build stage calls report "this call has no visible declaration in its source scope" | 4 | `omega::build_staged_dependency_calls a_call_written_before_the_wait_names_nothing` | Build staged imports | STAGED-BUILD-CAPABILITIES | Older: 4/4 |
| 21 | LITTYPE: "literal assigned to a place whose declared type is not computed yet" | 4 | `terminal-psi-to-terminal-psi tests::case_payloads::an_owned_payload_store_requires_its_exact_selected_case` | Psi 03 | Unowned | Older: 4/4 |
| 22 | BORROWCLOSURE: borrowed storage / conflicting loans | 6 | `terminal-psi-to-terminal-psi tests::rankings::borrowed_slice_iterations_keep_their_measure_and_backing` | lowered-form checker loans | STATE-LOCAL-VALUE-FRONTIER / STRUCTURAL-BORROW-IDENTITY | 3 older, 1 at #3352, 2 later |
| — | 9 small groups (2–3 tests each) and 22 unmatched single failures (OTHER) | 44 | see `failing_tests_grouped.tsv` (groups PROVREC, INDEX200, STRUCTDOM, BOUNDARYIDENT, FIXTUREGONE, CASTENTRY, CALLINGREL, ROOTSLOT, DISCRETE, OTHER) | mixed | — | 15 of 16 OTHER regressions are #3352 |

Notes on rows:
- FIXTUREGONE (2, older): tests read fixtures that are gone: `samples/cli/facts/nominal_declaration_owner/local/invalid`, and
  `tests/fixtures/pass/traits/runtime_dyn_slot_metadata_exit`, whose `dyn` syntax is now retired. Fix the tests.
- `compiler::filesystem_receiver windows_canonicalize_receiver` (CHKOP) prints **2.4 GB** of
  diagnostics when it fails. That output made nextest itself OOM-kill at 8.7 GB and stop the
  first workspace run at 5380/7126 tests. Until it is fixed, a full `mbx nextest run --workspace`
  cannot finish on a 15 GB host.
- The reports this inventory was asked to check: "57 lowered-form checker tests" = 50 in
  `terminal-psi-to-terminal-psi` + 7 elsewhere, split over rows 2, 3, 9, 10, 17, 21; "12 Build provider
  tests in package_review" = 19 `package_review` failures (rows 11, 12, 15, 5, 10…); "9 capability_graph"
  = 8 in row 4 plus 1 "callable cleanup has an ambiguous owning edge" (OTHER); "3 in Psi 03" = rows 17
  plus 2 OTHER; `terminal-wire declaration_contract_roundtrip…` = row 3 (#3884 targets it after the
  pin); "4 stage_validator_requests" = 3 here, all row 4. `terminal-runtime retained_runtime` **passes**
  (`retained_execution_preserves_identity_and_resumes_without_decoding`, 56 s); no test named
  `machine_self_call_recursion_compile` exists at this commit.

## Fixture programs (`omega run --both`)

69 of 104 pass, and both engines agree on the expected exit. The 35 failures by first error:

| Cause (matches test group) | Programs |
| --- | --- |
| GEN / GENFAM (claimed generics regression) | `generics/{finite_family_contract, implementation_parameter_contract, inferred_static_call_types, named_map_own_telescope, static_callable_specialization, width_parameter_identity}` (6) |
| CHKOP (claimed console regression) | `calls/indexed_array_contract`, `layouts/c_layout_size_alignment_query`, `numeric/policy_operators`, `providers/default_console_provider_reached` (4) |
| TYPEDOCC | `float/{exact_integer_cast, exact_integer_to_float, exact_narrowing, exact_widening}` (4) |
| "entry declaration was not prepared in the shared product roster" | `calls/{never_returning_call_in_expressions, non_returning_call_lends_storage, statement_after_process_exit}` (3) |
| "callable invocation actuals do not match the selected formal telescope" | `views/{byte_view_store, fixed_array_byte_arguments, mutable_byte_view_arrival}` (3) |
| "entry schema does not match the consumer's accepted source package and contract" (#3886 may fix) | `calls/{published_process_exit, published_scalar_postcondition}` (2) |
| SVCSCHEMA | `effects/native_process_exit_review`, `providers/native_boundary_supply` (2) |
| optimizer `SelectedOptimization(UnsupportedComposition)` | `optimizer/selected_allocation_{high,low}_pressure_exit` (2) |
| Not programs: library packages the glob picked up | `operators/qualified_operator_home/provider`, `reflection/build_parameter_description/library` (2) |
| PROV | `calls/indexed_case_discriminator` |
| ARMTYPE | `calls/process_exit_conditional_helper` |
| "a data declaration cannot own a machine" | `calls/qualified_atomic_operation_names` |
| LITTYPE | `ownership/known_case_payload_store` |
| STRUCTDOM | `ownership/stored_descriptor_boundary` |
| SEALORDER | `providers/target_selected_implementation` |
| conflicting loans | `views/ranked_slice_iteration` |

The fixtures were not rerun at `b30b6519`. Each one's regression status is
inferred from the test group that shares its message.

## Method and provenance

- Pinned `e47f05f7d…` (origin/main at 2026-11-16 23:39 UTC); pool slot `worker-1`; Linux x86_64 cloud container, 4 CPUs, 15 GB.
- `mbx nextest run --workspace --no-fail-fast` (all test kinds, not only `--lib`): 7126 tests across 236 binaries.
  The first run was OOM-killed at 5380/7126 (see above): `nextest_part1.log`. Part 2 reran the 21 unfinished
  binaries (`-E binary_id(...)`) and a container restart cut it at 2302/2366: `nextest_part2.log`. Part 3 reran the
  last 69 tests: `nextest_part3.log`. Every one of the 7126 tests has a final result (6882 pass, 244 fail).
- Fixtures: `omega run --both <main.omg>` per program with stdin closed, binary from the same build; rows in
  `fixtures_run_both.tsv` (exit 0 also checked against `expected_*_exit.txt`), first errors in `fixtures_failures.tsv`,
  full output in `fixture_logs/`.
- Regression check: the 243 failing tests (all but the 2.4 GB one) rerun at `b30b6519` (`nextest_at_b30b6519.log`) and at
  `5f8be54a` = #3352 (`nextest_at_5f8be54a.log`). Groups and per-test statuses: `failing_tests_grouped.tsv`.
- Claims snapshot: `claims_status.txt` (2026-11-16 23:41 UTC).
- Logs are trimmed to 2000 bytes per line; the raw logs (2.4 GB) are not committed.
- Wall time: workspace part 1 86 min (cold build included), part 2 about 60 min (its timing was lost in the restart),
  part 3 4 min, fixtures 56 min, `b30b6519` rerun 79 min, `5f8be54a` rerun 41 min: about 5.5 h of machine time.
