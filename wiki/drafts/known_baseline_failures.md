# Known baseline failures

Attribution evidence for test failures that reproduce at a recorded revision
without a task diff. A worker that hits one of these may cite this table —
command, revision, and failure set — instead of re-running a stash baseline.
Refresh or remove a row when its failures are fixed or when a task's diff could
plausibly interact with them; a listed failure does not excuse an unexplained
failure in affected behavior, and this file is not validation policy (see
[AGENTS.md](../../AGENTS.md#validation-scope)).

Rows verified by independent stash-baseline reproduction at revision
`8220f55febc1` on 2026-09-13, macOS arm64, unless noted otherwise.

> **Whole-doc caveat at `2dbfecd98e` (2026-09-21):** the workspace does not
> compile — `StructuralTypeShape::ElementView` (`04f2fdbb853`, merged via
> `48873065b72`) left every `StructuralTypeShape` match non-exhaustive
> (terminal-verifier ×8, optimization-unit, image-emission codec, downstream
> lowering). Every row below that names a nextest/cargo witness is
> unmeasurable at this HEAD until the ElementView consumer legs land;
> per-row memberships are carried from their last measurable readings.

## typed-trees-to-checked-trees

`cargo nextest run -p typed-trees-to-checked-trees --lib --no-fail-fast` at
d936717fd2 (2026-09-21, linux x86-64): 5065 run, 5065 passed, 0 failed. The
`--lib` cluster is closed: every member recorded below now passes.

Prior reading at 660f5af762 (2026-09-18, macOS arm64): 4159 run, 4153 passed,
6 failed — superseded. The `rank_ranges` trio
(`field_endpoint_formation_never_uses_final_cancellation_to_excuse_overflow`,
`field_endpoints_require_defined_intermediates_and_exact_owned_carriers`,
`constant_rank_endpoints_preserve_landing_and_rational_meaning`) landed under
**TERMINATION-RANKING-CHECKS** and now passes. The earlier reading at
30f4189a58 was 3991 run, 3988 passed,
3 failed. The 13-failure row recorded at
2c234a684c was worked through test by test; ten were stale fixtures or retired
premises (each commit names the introducing revision and the rule that decided
it), and the two renamed tests are now
`static_boundary_reaches_keep_every_direct_intrinsic_and_requirement_call` and
`general_state_graph_retains_interleaved_scalar_storage_write`. The last three
members of that cluster, all closed:

- Resolved: `tests::multiplicity::borrowed_observations::indexed_operand_access_preserves_shared_collection_and_owned_index`
  landed under **BASELINE-T2C-INDEXED-OPERAND-ACCESS** — `7ec7ee32e8` routes
  indexed operand zero through the attached-receiver loan
  (`receiver_self_match` in `indexing.rs`), so `machine [] Buffer::index(&self,
  ..)` admits a `Buffer` place exactly as `buffer.at(index)` borrows it, and
  `b845a7afd7` retains the explicit-parameter control
  (`ordinary_first_parameter_gains_no_receiver_adaptation`). Passes in the
  d936717fd2 reading. Original attribution:
  since f1f9f898e2 `build_operator_facts` no longer re-seeds `NestedExpression`
  value rows, which had resolved the `[]` occurrence with wildcard operands;
  under exact typing the `self.buffer: Buffer` place does not match the
  declared `items: &Buffer` operand because
  `typed-trees/src/typed_trees/declarations/operator/indexing.rs::shared_collection_elements`
  adapts only slice shells (a `buffer: &Buffer` parameter operand passes; a
  `Buffer` place fails in every spelling). The settled
  [indexing receiver rule](../spec/language/expressions.md#indexing-and-ranges)
  uses ordinary attached-receiver borrowing, not a new auto-borrow rule for
  ordinary first parameters. OPERATOR-MACHINE-SUPPLY owns expressing this
  fixture with an attached receiver, retaining a separate explicit-parameter
  control, and checking the real loan/custody route. This recorded failure is
  not closed by the documentation settlement or by restoring wildcard matching.
- Repaired: `tests::multiplicity::obligations_and_state_call_results::consuming_call_that_returns_an_obligation_transfers_its_origin`
  was a checker regression, not a stale pin. Since 2dc27270bc
  `validation/src/value_custody/permission_provenance.rs::static_namespace_receiver`
  returned `Err("namespace differs from its selected nonself state")` when the
  callee takes `self` and is spelled through its data namespace
  (`Receipt::forward(issued)`), so provenance fell back to a fresh
  Statement-1 establishment for the result while `claim_identity` kept the
  Statement-0 origin. A bare data-namespace receiver is a non-operand
  regardless of whether the selected state takes `self` — the explicit `self`
  argument supplies the common origin — so the `is_self` rejection is gone
  and the sibling pin in `permission_provenance/tests.rs` ("a self formal
  cannot be replaced with a namespace") now asserts the namespace is omitted
  for the self-target spelling as well.
- Repaired: `execution::terminal_unit::calls::computation_arguments::tests::scalar_caller_retains_call_produced_record_local_before_getter`
  was a broken fixture: its inline source spelled `Region { base, ... }`,
  but struct literals require explicit `field: value` pairs; the fixture now
  spells `base: base`.

## terminal-verifier

`cargo nextest run -p terminal-verifier --no-fail-fast` at a66852a558
(2026-09-18, macOS arm64, dependency crates rebuilt from the same tree):
726 run, 719 passed, 7 failed, then 721 passed and 5 failed after the
jump-edge repair, and 732 run, 729 passed, 3 failed after the byte-field
repair beside this row. Only two of the six non-ledger failures were
`InvalidPartialAffineCleanup`; each of the others had its own cause.

Re-measured at 7b224763615 (2026-09-21, linux x86-64), same command: 846
run, 841 passed, 5 failed. The ledger row below continues — the drifted
set now covers `proof-admission`'s `mathematical_core/subtraction` and
`validation`'s `affine_cleanup.rs`, `frontier.rs`,
`scalar_qualifications.rs`, and `argument_checks.rs` — and one new family
appeared:

- New — nominal affine-cleanup rejection loss (4 tests):
  `structural_unit::nominal_affine_cleanup::two_call_nominal_affine_cleanup_rejects_repeated_or_nonempty_helpers`,
  `...::one_call_nominal_affine_cleanup_rejects_nonexact_closures`,
  `...::nominal_affine_cleanup_rejects_contract_carrying_members_and_self`,
  and
  `...::executable_nominal_affine_cleanup_rejects_contract_or_unattached_helpers`
  each assert `Err(ModuleError::InvalidNominalAffineCleanup { .. })`
  (tests/structural_unit/nominal_affine_cleanup.rs:161/235/427/470) and
  the rejection no longer fires — the modules validate. In-window suspect
  `4d34752d7e4` ("cleanup: lower and invoke the real owner-attached drop
  hook body"), which moved the cleanup lane's owner hook; not bisected.

- `trusted_surface::recorded_digests_match_the_working_tree` (and its
  `source_coverage_fires_on_a_changed_implementation` sibling): the ledger
  in `trusted_surface/sites.rs` records digests for twelve implementations
  (`semantic-vocabulary/src/content.rs` and `proposition`, terminal-psi
  proof-bundle admission and nodes, proof-admission `integer_rules/*`,
  `kernel.rs`, `lib.rs`, `evidence.rs`) that have since changed; the ledger
  needs re-recording by the lane that changed them, after review. It also
  reports `proof-admission/src/mathematical_core/tests/strict_layer.rs` as
  an unregistered source file under a trusted root. Fix leaf: the standing
  **TRUSTED-SURFACE-DIGEST-RE-RECORDING** item on the board.
- Repaired: `unranked_bindings::cyclic_scalar_targets_and_reachability_are_checked_before_dominance`
  and `unranked_views::every_cyclic_view_jump_checks_exact_arity` saw
  `InvalidPartialAffineCleanup` in place of `UnknownTargetBlock(BlockId(99))`
  and `StructuralJumpArityMismatch { edge: EdgeId(1), expected: 1, actual: 0 }`.
  The Jump-edge lane of `validation/affine_cleanup/continuation.rs`, added by
  517e86d465, resolved the terminator's target block and compared successor
  argument and parameter counts itself, in a pass that runs before
  `control_flow::validate_control_flow` and the structural frontier own
  those two checks. That lane now defers both shapes.
- Repaired: `structural_unit::boundary_buffers::ordinary_unit_byte_subloan_rejects_wrong_leaf_type_access_and_path`
  (mutation 1) and
  `structural_unit::boundary_buffers::boundary_buffer_rejects_wrong_leaf_erasure_access_and_type`
  were a lost rejection, not an ordering change: `canonical_leaf_shape`
  answered `Some` for `ByteSequence(carrier)` since 2fc3f6ad67, so a record
  field typed `ByteSequence(BorrowedView)` resolved by shape equality to the
  module's standalone declaration and satisfied a callee parameter directly,
  bypassing the inline presentation route that
  [byte views](../spec/terminal-psi/byte_views.md) and
  [structural access](../spec/terminal-psi/structural_access.md) make the
  only way a field-projected byte argument reaches a parameter. It answers
  `None` now, which closes the same hole in terminal-codec's independent
  validation; all seven leaf-shape consumers were followed and none needed
  explicit byte handling.
- Repaired: `structural_unit::boundary_buffers::fixed_array_views::fixed_byte_array_unit_view_keeps_existing_zero_array_admission_fence`
  expected `InvalidStructuralArrayLength(StructuralTypeId(3))` for a
  zero-length fixed byte array and saw
  `StructuralArgumentTypeMismatch { operation: OperationId(1), argument_index: 0, expected: StructuralTypeId(1), actual: StructuralTypeId(3) }`.
  The type-table fence in `validation/foundation/structural_types.rs` fires
  only when `terminal_semantics::scalar_array_leaf_shape` is `None`, which a
  byte array's primitive-scalar element is not, so the rejection had moved to
  the presentation check. The settled semantics admit a zero-length fixed
  array whose element carries a scalar leaf as an empty scalar array, while
  the `InvalidStructuralArrayLength` fence remains for elements without a
  scalar leaf: d96a0fda39 retargeted the fixture's element at a record so the
  pin still exercises the type-table fence on both the machine and boundary
  routes. The fence question this row left open is answered — presentation
  admits the scalar-leaf case, the type table owns the remaining rejection.
- Repaired: `structural_scalar_fields::owned_reads::block_parameters::owned_successors_reject_same_arity_aliases_and_transfer_after_disposal`
  expected `InvalidStructuralSuccessorArgument { edge: EdgeId(3), place: PlaceId(2) }`
  and saw `EdgeAffineDiscardsInvalid { edge: EdgeId(3) }`; both reject the
  same module. The documented order in `frontier/block_parameters.rs` —
  phase one consumes each owned source before the residual and trivial
  cleanup for the same edge runs — is the settled order: the fixture now
  pins both diagnostics, so a same-arity alias still reports
  `InvalidStructuralSuccessorArgument`, while naming a still-live
  transferred place in the edge's trivial-discard roster reports
  `EdgeAffineDiscardsInvalid` as malformed discard evidence rather than a
  bad argument. Passing at bbfda8bc2e.

## pass fixtures that no roster runs

Measured on 2026-09-20 at df5faae187 by comparing every
`tests/omega/pass/<group>/<fixture>/main.omg` against the rosters in
`compiler/tests/canary_suite.rs` and `compiler/tests/fixture_rosters/`:
2011 pass fixtures exist, 1707 sit on an executing roster
(`ACTIVE_PASS_CANARIES`, `CHECKED_ONLY_PASS_CANARIES`, or a rooted-target
row), and 304 do not. Of those, 260 are inventoried in `fixture_rosters/`
and so belong to a dedicated target, often host-gated; 44 appear in no
roster of either kind and are compiled by nothing at all.

The 44 are worth treating as a live process problem rather than historical
debt, because several were added within hours of this measurement. They
group as: 18 under `objc/`, 6 under `terminal_psi/`, 5 under `filesystem/`,
3 under `float/`, 2 each under `arithmetic/`, `borrows/`, `control_flow/`,
`domains/` and `inline_asm/`, and one each under `generics/` and
`progress/`. Three separate fixtures this session were found unexercised
this way before the pattern was measured, so the check is worth repeating
after a wave of fixture work.

Re-measured on 2026-09-21 at 7b224763615 (linux x86-64), same comparison
(fixture directories holding `.omg` files, counted unrostered when no
roster string covers the directory or an ancestor): 42 fixture
directories remain uncovered — 18 under `objc/`, 6 under `terminal_psi/`,
5 under `filesystem/`, 3 under `float/`, 2 each under `arithmetic/`,
`control_flow/`, `domains/`, and `termination/`, and one each under
`generics/` and `progress/`. The recorded `borrows/` pair is rostered
now; `termination/` and the second `domains/` entry are the new names.

Registering one is not automatically the repair. A fixture on
`CHECKED_ONLY_PASS_CANARIES` is compiled only through checking: forcing the
lowering custody validation to reject unconditionally leaves those canaries
green, so a checked-only registration pins checked semantics and cannot
exercise a lowering repair. Where the subject is lowering or native
realization, the fixture needs an executing roster that reaches that stage,
or a crate-level test beside the code. Fix leaf: roster registration and
the repeat-the-measurement sweep are corpus maintenance — **CANARY-CORPUS**
on the board.

## compiler canary suite (pass canaries)

`cargo nextest run -p compiler --test canary_suite -E
'test(pass_canaries_compile) | test(fail_canaries_reject_with_expected_diagnostic_fragment)
| test(/registered_/) | test(checked_only_canaries_are_not_backend_umbrella_members)'`
at 942f23e6f4 plus the climbing-sum repair beside this row (2026-09-18,
macOS arm64, 1157 s): 6 run, 5 passed, 1 failed. Only
`pass_canaries_compile` fails, on 22 registered fixtures, and this is the
freshly measured distribution **CANARY-CORPUS** asks for rather than the
older reading:

- 19 report `Lowering(InvalidUnitMachinePlan { .. })` from native-artifact
  Terminal production. The six `core/numeric_*` rows are
  **ARITHMETIC-POLICY-REALIZATION**, not a control-builder gap; the rest are
  the missing transitive Unit plan class (**GENERAL-CYCLIC-EXECUTION** and
  the state-graph route). The families:
  `core/numeric_*` conversion surfaces (6), `float/float_trapping_*` (5),
  `expressions/arithmetic_domain_trapping_*` (3),
  `control_flow/runtime_*_literal_dispatch_exit` (2),
  `capabilities/acquires_through_helper_return`,
  `host/runtime_gui_foreground_window_exit`, and
  `wire/runtime_wire_exact_array_without_count_exit`.
- `calls/statement_call_recursive_argument_compile`: "duplicate named
  machine overload `add`", with sibling reports of a duplicate `Nat` data
  declaration and a public interface selecting private `Nat`.
  **Repaired 2026-09-20 at e5912f303a**: the fixture's own `Nat`/`add`
  collided with the `core/nat.omg` exports (added 2026-09-17); renamed to
  `Peano`/`peano_add`, and the sibling `read_line`/`extent_shape`
  diagnostics proved collision collateral.
- `operators/runtime_integer_division_value`: "native-artifact production
  requires one exact selected program entry".
  **Repaired 2026-09-20 at e5912f303a**: the fixture carried no `build.omg`
  entry binds; added them for all four hosted targets and re-scoped the
  operands to `u64`, the realized `ExactIntegerDivide` carrier. Signed
  `i32` exact division remains attributed to the unsigned-quotient /
  arithmetic-policy lane (**ARITHMETIC-POLICY-REALIZATION**).
- `atomics/atomic_field_declared`: "macOS hosted receiver bridge lost exact
  contract, storage, or entry custody" (**ENTRY-CONTENT-ROOTS**; migrate the bare
  receiver field to the [intrinsically established service carrier](../spec/build/component_publication.md#bindings-and-era-entry)).
  **Retired from this cluster as host-bound**: verified green on Linux
  x86-64 at 4607987316 and re-confirmed on bbfda8bc2e under
  `OMEGA_PASS_CANARY_FILTER=atomics/atomic_field_declared`; the recorded
  failure is the macOS hosted-receiver leg, which stays with
  **ENTRY-CONTENT-ROOTS**.

The `filesystem/native_*` family was reconstructed on 2026-09-18 at
40e22e0ce9. Its 54 fixtures beside `native_close` carried no `build.omg` and
imported the retired `omega::language::std` spelling, so
`OMEGA_PASS_CANARY_FILTER=filesystem/native_` reported "matched no active
pass canaries" rather than a failure: the family was inventoried through
`fixture_rosters/native_filesystem_canaries.rs` but sat on no executing pass
roster. With an ordinary build declaration and the `omega_language_std`
alias, and with the core library's unsigned widening machines publishing
their source carrier's range at 96c08b0109, and with the eight `struct stat`
byte-assembly fixtures re-spelled at 285da57703, 53 reach checked semantics
and are registered in `CHECKED_ONLY_PASS_CANARIES`. The one left
unregistered is `native_wrapper_write_all_result`, on
**MATCH-SELECTIVE-LOWERING**'s value-dispatch pattern limit. The eight that
assembled a field with `widen_u8_to_i64(byte) << 56`, and in three cases a
32-bit field with `widen_u8_to_i32(byte) << 24`, were unsound as authored:
those intermediates reach about 1.84e19 and 4.28e9 against `i64` and `i32`
ceilings of about 9.22e18 and 2.15e9, which no return range can discharge.
Each now assembles in the unsigned carrier of the field's own width, where
every shifted byte is representable, and reinterprets once at the landing.

`tests/omega/pass/filesystem/windows_set_file_time_exit` carried the same
`widen_u8_to_i64(byte) << 56` idiom; the **WINDOWS-SET-FILE-TIME-RESPELL**
leg repaired it at ff782bdf21 — `st_mtime` now assembles in the unsigned
`u64` carrier like the eight native stat/metadata siblings. Its canary is
Windows-gated, so native execution there remains unmeasured on this host,
but the repair itself is measured here: the fixture sat on an inventory
roster only, so nothing compiled it, and registering it in
`CHECKED_ONLY_PASS_CANARIES` takes
`OMEGA_PASS_CANARY_FILTER=filesystem/windows_set` from "matched no active
pass canaries" to one passing, while restoring the signed spelling makes
that gate refuse it with the decision-17 diagnostic. The rooted
`windows_x86_64` backend roster is not its home: compiling it there refuses
because the entry's `Main::fs` service field wants a selected fused
provider for `FilesystemHost`, the same gate under which the registered
`filesystem/windows_raw_roundtrip_exit` is red on this host today and which
keeps the `native_*` family off native realization.

`proofs/proof_inductive_climbing_sum` left this set when its accumulator
was bounded; the other four tests in the command pass, so the roster,
fail-canary fragments, and umbrella membership are all consistent.

2026-09-20 refresh (Linux x86-64, c2ccb2a202): the wire `runtime_*_exit`
canaries moved to a new diagnostic family. `pass_canaries_compile` under
`OMEGA_PASS_CANARY_FILTER=exact_array_without_count` now fails
`wire/runtime_wire_exact_array_without_count_exit` with "selected
ProgramEntry establishment rejoins 0 Terminal attachment identities;
expected one" — verified identical on the unmodified fixture — and the
`*_canary_runs` legs for `runtime_wire_encode_primitive_exit`,
`runtime_wire_decode_rejects_wrong_era_exit`,
`runtime_wire_roundtrip_repeated_exit`, and
`runtime_wire_exact_array_without_count_exit` all fail compile with the
same diagnostic at ~43 s each. Every one of these calls a synthesized
`Schema::encode`/`decode` with `&mut self.buffer`/`&mut self.written`
field borrows, so the whole wire codec-call shape is red at the entry
gate; this is the same attachment-identity family that drops composed
plans on `&[u8]`-bearing unit-machine calls (also recorded under
CANARY-WIRE-EXACT-ARRAY-WITHOUT-COUNT-EXIT's swarm verdict). An
interrupted full-corpus sweep in the same window additionally showed the
`runtime_text_and_transitions` family red. The 2026-09-18 row's
`InvalidUnitMachinePlan` distribution is superseded for the wire set, not
refuted: the old failures were measured before the attachment-identity
rework landed.
`roster::registered_pass_canaries_have_source_on_every_host` is also red
at c2ccb2a202 on two unregistered fixtures —
`core/wait_wake_boundary_surface` (11dd1d8084) and
`generics/authored_const_application_local_destination` (e6a5bbe37a) —
each added by a same-day commit without a roster entry.

The recorded set was re-measured at 7b224763615 (2026-09-21, linux
x86-64) by filtering `pass_canaries_compile` to the 22 recorded members
plus the `control_flow/runtime_` prefix group: the family attribution is
unchanged but the dominant surface moved. Several recorded
`Lowering(InvalidUnitMachinePlan)` members now stop one gate earlier at
"selected ProgramEntry establishment rejoins 0 Terminal attachment
identities; expected one; the machine's unit plan was omitted at local
construction at <phase>" (`float/float_trapping_*` ×5,
`expressions/arithmetic_domain_trapping_*` ×3,
`control_flow/runtime_{integer,string}_literal_dispatch_exit`,
`wire/runtime_wire_exact_array_without_count_exit`, and others) — the
same missing transitive checked machine plan, now reported by the
selected-entry rejoin. `core/numeric_*` keeps its split: three members
still end at `Lowering(Unsupported("checked trapping conversion requires
runtime policy realization"))` (ARITHMETIC-POLICY-REALIZATION), while
`numeric_conversion_surface` now reports `Lowering(Unsupported("Unit
graph has unreachable states"))` and
`numeric_cross_signed_conversion_surface` stays an InvalidUnitMachinePlan
("scalar callee has no checked executable body").
`capabilities/acquires_through_helper_return` now passes.
`host/runtime_gui_foreground_window_exit` moved to "selected ProgramEntry
Service field `Main::gui` requires a selected Fused provider for boundary
`Gui`". The filter additionally caught ten `control_flow/runtime_*`
fixtures in the same missing-plan/entry classes
(`runtime_branching_helper_{guard,string,struct}`,
`runtime_branching_helper_local_guard_value`,
`runtime_nested_branch_{value,prelude_value,assignment_prelude_value}`,
`runtime_multi_assignment_value_calls`,
`runtime_guarded_leaf_ordering_call` — the last on "native-artifact
production requires one exact selected program entry"); whether they
were added since the recorded reading or regressed is unbisected, but
every diagnostic is one of the owned families above.
`runtime_compare_pair_dispatch_exit` left this set when composed-control
admission learned the checker's exact guard complements — builtin `==`/`!=`
over identical operands and opposite Boolean labels in either authored
order — so the pair no longer stops at a missing unit plan.

## compiler canary suite (fail-canary diagnostic fragments)

`mbx nextest run -p compiler --test canary_suite
proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment`
at e12b9e8e06 (2026-09-20, macOS arm64): 1108 registered members checked,
11 red. Of the eleven, six are stale `expected.txt` fragments — the
member still rejects for the same reason under reworded diagnostics —
and five are true failures (the member accepts, or rejects for a
semantically different reason that masks the intended check). The full
audit, including specialized-owner members and the three unregistered
fixtures reported by
`roster::registered_fail_canaries_have_source_and_their_owned_expectations`,
is at `build/swarm/dev-l3/fail-map.md` on the `swarm/dev-l3-failsweep`
branch worktree.

Verified stale fragments (a later leg can re-pin `expected.txt`
wording; none unblock a correct rejection):

- `build/program_entry_binding_outside_build`: expected
  "root binding requires a compiler-issued &mut Build place", actual
  "…&mut Build receiver"
  (`typed-trees-to-checked-trees/src/authored_selections/finalization.rs:78`).
- `comptime/fuel_exhausted_const_array_length`: expected
  "machine `table_size` is not build-time admissible", actual
  "fixed-array length `[i64; table_size()]`: const evaluation of
  `table_size` failed: step budget exceeded" — the fuel backstop still
  fires (`build-time-evaluation/src/const_evaluation/const_lengths.rs:71`).
- `expressions/indexed_qualified_call_argument_mismatch`: expected
  "cannot prove requires contract", actual
  "index compatibility condition … `Coordinate<7>` and expected
  `Coordinate<9>` are distinct normalized instances …"
  (`typed-trees-to-checked-trees/src/facts/index_compatibility.rs:441`).
- `providers/provider_selection_outside_build`: expected
  "has no local state `select_provider`", actual
  "value call `select_provider(..)` does not resolve to a state of this
  machine, an attached sibling machine, or a free machine -- it would
  silently bind 0 (ZII) at runtime"
  (`validation/src/machine_calls/calls/expression_scanning/target_resolution.rs:173`).
- `generics/const_data_machine_call_requires_zero_arguments`: expected
  "takes 1 parameter(s); a const-evaluated generic argument must call a
  zero-argument machine", actual "const-generic application evaluation
  failed: constant call argument count differs from its exact entry"
  (`build-time-evaluation/src/const_evaluation/const_generic_calls.rs:260`).
- `generics/const_data_machine_call_requires_pure`: expected
  "const-generic evaluation of `loud_size()` failed: …", actual
  "const-generic application evaluation failed: machine `loud_size` is
  not build-time admissible: service reach [Console]; …" — identical
  inner reason under a renamed wrapper (`const_generic_calls.rs:260`).
- `generics/closed_indexed_qualification_unknown_const` and
  `generics/closed_indexed_qualification_wrong_arity` (masked inside
  `closed_indexed_domain_canaries`, verified by direct check): expected
  "neither a canonical named const nor a direct in-scope const binder"
  and "requires 1 closed const argument(s)", actual "machine index
  operand must select a constant in its original lexical scope; …" and
  "indexed domain `Quantity` requires 1 closed index argument(s), but 0
  were supplied"
  (`build-time-evaluation/…/lexical_selection.rs:364`,
  `syntax-trees-to-symbol-resolved-trees/…/const_evaluation/domains.rs:319`).
- `canary_suite/relational_invariants.rs:5` shares the stale inline pin
  `INDEX_REJECTION = "cannot prove index `self.i` is within length 8"`
  across six `dependent/relational_loop_invariant_*` members whose own
  `expected.txt` files are accurate; the constant needs per-member or
  shortened wording (actuals: "within length 1", "within unknown slice
  length of `self.items`"). Test-code drift, not corpus drift.

True failures the audit found (not wording drift):

- `ownership/linear_ambiguous_state_result_mapping` and
  `calls/guarded_value_call_terminal_rejected` compile successfully;
  `calls/free_machine_named_transition_rejected` compiles through its
  owner test's native path.
- `proofs/mathematical_declaration_lowering_rejected` compiles cleanly
  on `linux_x86_64` (its only bound target); on macOS hosts it is red
  for an unrelated root-slot reason that masks the intended check.
- `domains/boundary_operator_mutation_invalidates_domain` rejects
  earlier on a `&mut` lending refusal, masking the NoNul-invalidation
  proof it exists to pin.
- `generics/colon_bound_rejected` rejects because `T: copy` now parses
  as a value-parameter binder refused by the data-template gate; the
  colon-bound guidance diagnostic no longer exists.
- `host/console_byte_field_target_rejected` rejects on undeclared
  service reach before the byte-op serving-shape blocker it pins.
- Unregistered fixtures (roster gap, all verified green):
  `borrows/borrow_proposition_opaque_mut`,
  `operators/mismatched_operand_tuple`,
  `generics/authored_const_call_operator_unselected_provider`.

Re-measured at 7b224763615 (2026-09-21, linux x86-64): every member named
above now passes its expected rejection — the six stale `expected.txt`
fragments were re-pinned, the two masked `closed_indexed_qualification`
members agree, the `relational_invariants` `INDEX_REJECTION` constant pin
is repaired, and the seven true failures reject for their intended
reasons again (`OMEGA_FAIL_CANARY_FILTER` covering all named members on
`fail_canaries_reject_with_expected_diagnostic_fragment`, plus the two
`relational_invariants` tests). Two of the three roster-gap fixtures
(`borrows/borrow_proposition_opaque_mut`,
`operators/mismatched_operand_tuple`) are registered in `canary_suite.rs`
now; `generics/authored_const_call_operator_unselected_provider` is still
unregistered. Section closed pending the next audit.


## checked-trees-to-lowered-psi

Repaired: `unit_scalar_result_source::boundary_wrappers::ordered_boolean_guarantees::ordered_boolean_call_computations_preserve_normal_guarantees`
previously failed with `OperationProofUnavailable(ObligationId(9223372036854775809))`
at `tests/unit_scalar_result_source/boundary_wrappers.rs:41` (recorded at
00d0f9c15f on 2026-09-16 and at 51f21bb168, reconstruction inside
`src/proofs/operation_proofs.rs`). At d8d48fe4ff it passes in 13.8s under
nextest, so the operation-proof obligation on the `machine_calls` path is
discharged by the borrow-proof landings since.

`cargo nextest run -p checked-trees-to-lowered-psi --no-fail-fast` at
d8d48fe4ff (2026-09-20, Linux x86-64) runs the whole crate: 2143 run, 2085
passed, 58 failed (57 FAIL plus one test killed by SIGTERM after ~1300s).
Reproduced unchanged at bcb0086e22: 2146 run — the three tests added since
all pass — with the same 57 FAIL signatures and the same nonterminating
`mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
(killed after ~1380s).
The prior whole-crate reading at 9d0d864656 (2026-09-18, macOS arm64) was
2032 run / 20 failed; of its named groups, boundary byte buffers,
crash-member byte entries, and the ordered-boolean row are green now, while
scalar-return custody, provider attachment, and the attached-unit
borrowed-self case continue (the last under a new diagnostic — see the
Service<R> family). The current failure set attributes to six families:

- Repaired: the bare boundary-trait fixture spelling (33 tests) is gone.
  The 21 `console: Console` and `output: Output` spellings across the
  library `tests::*` sources became `Service<R>` carrier fields at
  00a69f066b0, and `tests/unit_plan_omissions.rs`'s 4 `runtime: TaskRuntime`
  spellings became `&'s mut TaskRuntime` receivers at 37e309e6060. Verified
  at 00e1da7ae2a on macOS arm64: `cargo nextest run -p
  checked-trees-to-lowered-psi --no-fail-fast` reports 2199 run, 2174
  passed, and not one `validate_no_bare_boundary_trait_values` rejection in
  the log. Every declared boundary trait in those trees was scanned for a
  value-position field and none remains. The 30 library cases pass, and the
  3 `unit_plan_omissions` members moved into the missing-transitive-machine-
  plan family below, stopping at `signature`-phase local construction; the
  shared-borrow negative control still pins that stop.

- Missing checked transitive machine plan (16 tests in this crate, but the
  cluster is wider than this section records — see the cross-suite note at the
  end of this entry).
  `provider_attachment_source` ×6 stop at `signature` and
  `unit_state_graph::provider_attachments` ×9 plus
  `guarded_scalar_returns_source::stored_returned_cases_support_borrowed_refined_getters`
  stop at `state graph: state signature: parameter signature: attached data
  shape, state 0`, all surfacing as `InvalidUnitMachinePlan` "attached Unit
  closure is missing a checked transitive machine plan" / `` `X` has no
  admitted body (local construction stopped at <phase>) ``. The `signature`
  phase site is `execution/unit/control/checked_machine.rs` and the
  attached-data-shape guard is `execution/unit/calls/signatures.rs`, both in
  typed-trees-to-checked-trees unit construction; the fixtures pass source
  checking (already migrated in 0e1977994b) and stop while admitting the
  attached closure's bodies. Fences: `execution/unit/{control,state_graph,composed_control}`
  is under GENERAL-CYCLIC-EXECUTION and `execution/unit/{mod.rs,candidate_closure,calls}`
  plus `checked-trees-to-lowered-psi/src/unit` under UEFI-OS-HANDOFF.

  **Cross-suite span, measured 2026-09-21 on macOS arm64 once
  `cargo check --workspace` came back green.** The same refusal accounts for at
  least 17 failures across three suites, not 16 in one:

  - `checked-trees-to-lowered-psi` — 8 more than this section lists, in
    `retention::conformance_applications::tests` ×3 and
    `tests::composed_operand_catalogs` ×5. These were not measurable before:
    the crate's test target could not build while the `StructuralTypeShape::ElementView`
    consumer legs were outstanding, so they are newly visible rather than new.
  - `compiler --test canary_suite -E 'test(/inline_asm/)'` — 5 of 13 legs, all
    the `x86_asm_*` byte-emission ones. Each pins an explicit
    `target_name: linux_x86_64` cross-compile, so this is not host-dependent.
  - the native-differential suite — 4 `terminal_psi_runnable` legs.

  A **third** omission phase appears alongside the two recorded above
  (`signature` and `state graph: state signature: ...`): the asm legs stop at
  `statement sequence: call: call operation, statement 0`, i.e. `asm` statements
  pass checking and stop at the lowering wall. Closing those five needs the
  asm-statement lowering arm, not more catalog members.

  Ownership is unchanged and this is engineering, not a design question: the
  producing surfaces carry live fences (GENERAL-CYCLIC-EXECUTION,
  UEFI-OS-HANDOFF), and the board's own attribution row calls this "a distinct
  Unit-plan admission gate, not the check diagnostic".

  This is the continuing "provider attachment and results" group from the 9d0d864656
  reading.
- Crash predicate outside the selected scalar namespace (3 tests).
  `exact_affine_sibling_source::landed_affine_sibling_custody_crosses_source_codec_and_independent_verification`,
  `exact_shift_left_certificate_source::bounded_exact_left_shift_uses_only_its_canonical_certificate`,
  and `mixed_shift_source::erased_arithmetic_prefix_still_requires_its_own_certificate`
  each fail lowering with ``Unsupported("crash predicate value position is
  outside the selected scalar namespace")`` from
  `src/proofs/crash_routes/scalar_terms.rs`. On the `crash.site_guard` path
  (`scalar_graph/scalar_graph_module/state_emission.rs`,
  `lower_checked_crash_predicates(&crash.site_guard, self.parameters)`)
  predicate `Parameter`/`Local` positions index past the lowered `values`
  roster and that path supplies no erased roster
  (`checked_boolean_scalar_term(expression, values, &[])`), so positions
  moved into the proof-only erased lane by the erased-formal term work
  (294b6cfbf4, 2026-09-19) reject — consistent with the PROOF-RELEVANCE
  item's erased-binding namespace rule, not bisected; the identity-less
  crash-route discharge change ebef2636d8 (2026-09-20) is the adjacent
  suspect. `src/scalar_graph` is under the WRITE-ONLY-BORROW
  integer-entry-ranges claim.
- Scalar-return custody (4 tests, `tests/owned_record_return_source.rs`).
  `discarded_scalar_invocation_precedes_whole_owned_return`: the checked
  `facts.flow.terminal_unit_effects.for_machine` returns `None` for the
  record-returning `retain` after `_ = identity(mask); record` — no
  unit-effects plan is catalogued. `effectful_discarded_call_writes_before_return_across_fuel`:
  ``Lowering(Unsupported("composed Unit scalar call requires structural call
  custody"))`` at `src/unit/attached_unit/composed_control/admission.rs` for
  the `stamp(&mut output, ...)` out-parameter — structural call-custody
  territory (`validation/machine_calls/structural_call_custody.rs` and
  `src/unit/attached_unit*` are under the WRITE-ONLY-BORROW
  integer-entry-ranges claim). `source_replay_rejects_return_parameter_and_carrier_substitution`
  unwraps `plan.structural_result` at `tests/owned_record_return_source.rs:392`
  on `None` — the negative control finds no structural-result plan to
  tamper. `source_replay_requires_the_exact_affine_return_transfer`:
  `Record: changed return transfer 0` — mutating the return transfer's
  `machine_symbol` is not rejected by
  `terminal_production::TerminalProductionRequest::produce_artifact`, a
  source-replay verification gap. This is the continuing "scalar-return
  pure source custody" group; its fix leaf on the board is
  **C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES**.
- `established by` call-result qualification (1 test).
  `registered_callback_lifetime::interpreted_register_unregister_round_trip_drives_the_ledger`
  fails at source check with ``cannot establish call-result qualification
  `Registration::Live`: the exact invocation, authorized route or consumed
  qualified claims, and result correspondence are not proved`` from
  `typed-trees-to-checked-trees/src/checks/content/call_results.rs` — the
  fixture's `domain Registration::Live established by Registrar::register`
  route is the ENTRY-CONTENT-ROOTS documented residual ("`established by`
  establishment routes ... stay red until the receiver lifecycle leg
  lands"). `src/checks/content` is under the BOUNDARY-ISSUANCE claim.
- Proof-search blowup (1 test, SIGTERM).
  `nominal_affine_source::integer_comparison::mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
  was killed after ~1300s at ~573% CPU inside `lower_machine` on this host;
  the fixture (an 18-formal machine carrying ~60 requires conjuncts) is
  unchanged since August, so the cost is proof search, plausibly
  interacting with the bound-closure/premise relaxations of 2026-09-19/20
  (20ceb1e0a4, e37e1a8afc, a9f1a8aa8b, ebef2636d8) — not bisected, and
  kernel hardening on this host (`ptrace_scope=1`,
  `perf_event_paranoid=4`) blocked stack sampling. PROOF-SEARCH-MEASUREMENT
  owns `proof/src/checker.rs` and the per-plan measurement work
  (0750f6a18b); rerun alone via
  `cargo nextest run -p checked-trees-to-lowered-psi --test nominal_affine_source`
  to time it.

Residual attribution at 9d07a59a48 (2026-09-20, Linux x86-64), same command:
2146 run, 2088 passed, 58 failed (57 FAIL plus the same proof-search member,
killed externally after >1440s — the same blowup, still unbisected). Every
failure maps onto the six families above with identical diagnostics —
the bare `Service<R>` spellings (since repaired, see above), 16 missing
transitive machine plans, 3
site_guard crash-namespace rejections, 4 scalar-return custody cases, 1
`established by` qualification, 1 blowup — so the residual tail is empty.
A host note worth its own attention: at 00e1da7ae2a on macOS arm64,
`nominal_affine_source::integer_comparison::mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
ran past 1800 seconds with every other test in the crate finished, and was
terminated. Linux readings above record the same member killed past 1400
seconds at high CPU, so it is either nonterminating or pathological on both
hosts, and it taxes every full run of this crate.

The three added tests since d8d48fe4ff all pass. One boundary-timing note:
`owned_match_nested_record_replays_every_selected_payload` passed at 336s
(was not flagged slow in the d8d48fe4ff reading) — a near-threshold pass on
this host, not a failure.

Re-read at 6b610300e8 (2026-09-20, Linux x86-64), same command: 2146 run —
no tests added since 9d07a59a48 and the crate is unchanged in that window —
with the identical 57 FAIL set test-for-test (33 bare `Service<R>` fixture
spellings, 16 missing transitive machine plans, 3 site_guard
crash-namespace rejections, 4 scalar-return custody cases, 1 `established
by` qualification) and the same proof-search member still nonterminating
(killed externally after >1560s). The tail remains empty.
`owned_match_nested_record_replays_every_selected_payload` passed at 297s
this reading — still near-threshold, not a failure.

Confirmation at 210ffe3c93 (2026-09-20, Linux x86-64), same command: 2146
run, 2088 passed (10 slow), 58 failed — the same 57 FAIL members plus the
same nonterminating
`mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`,
killed externally after >1400s at ~570% CPU. Every failure maps onto the
six families above with verbatim-identical diagnostics — the bare
`Service<R>` spellings (since repaired), 16 missing transitive machine
plans, 3 site_guard
crash-namespace rejections, 4 scalar-return custody cases, 1 `established
by` qualification, 1 blowup — so the residual tail is still empty. The two
crate-local commits since 9d07a59a48 (400c353604 machine_lowering
coordinator domain shedding, 89f3a708b2 custody/selection doc-link repair)
change no test behavior, and the near-threshold member
`owned_match_nested_record_replays_every_selected_payload` passed again at
264s — still slow-flagged, still not a failure. All 58 members remain owned
by the families' named items.

Residual attribution at 6ef64f6dd6 (2026-09-20, Linux x86-64), same command
plus a separate `--test suite` run for the blowup member: 2152 run, 2093
passed (18 slow), 59 failed, and the same
`mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
member aborted by SIGTERM at ~892s — the blowup persists, still owned by
PROOF-SEARCH-MEASUREMENT. The tail moved since 210ffe3c93 and is
re-attributed member-by-member below.

- Unchanged families at identical panic sites: 30 `tests::*` cases panic at
  `src/tests.rs:82` on the bare `Service<R>` fixture spelling plus the 3
  `unit_plan_omissions` members at `tests/unit_plan_omissions.rs:16` (33
  total, ENTRY-CONTENT-ROOTS); 16 missing checked transitive machine plans
  (`provider_attachment_source` ×6 at :97,
  `unit_state_graph::provider_attachments` ×9 at :220/:50, and
  `guarded_scalar_returns_source::stored_returned_cases_support_borrowed_refined_getters`
  at tests/:84); 3 site_guard crash-namespace rejections; 4 scalar-return
  custody cases; 1 blowup.
- Closed: `registered_callback_lifetime::interpreted_register_unregister_round_trip_drives_the_ledger`,
  the family's sole member, now passes — in-window closers are
  851052b4f8f (admit constrained-result spelling as boundary issuance
  witness) or 1fc01bb6907 (transferred input is not fresh supply), both in
  `typed-trees-to-checked-trees/src/checks/content`.
- New — ranked safe-point segment bounds (2 tests):
  `structural_control_cases::ranked_countdown_lowers_to_verified_resumable_interpreter_execution`
  now reads per-edge ceiling 3·2³³ (0x600000000) instead of 3, and
  `ranked_u64_countdown_fails_closed_when_fixed_fuel_exceeds_u64` gets
  `Err(BoundOverflow)` where per-edge segments were asserted to stay within
  the u64 schedule. `terminal-fixed-fuel`'s `derive_fixed_safe_point_segments`
  is unchanged in-window; the change entered through its verified input —
  prime suspect 39e156c73a0 (retain integer entry ranges through terminal
  psi; scalar_qualifications +151 lines). Post-base 7591b2607c7 (bound
  segments through ranked cyclic components) is actively migrating this
  surface, so the family is mid-flight — the ranked-cycle/fuel lane owns it;
  not bisected.
- New — closed-projection replay admission (1 test):
  `expression_preparation::bindings::tests::closed_record_projections_replay_exact_sources_carriers_and_all_siblings`
  (bindings/tests.rs:209) — mutating a member's symbol to invalid/foreign no
  longer fails replay; the checked bound-expression facts appear to be
  authoritative for field identity. In-window suspects 39e156c73a0
  (result_contract/scalar_contracts facts rework) or 143636cec8a
  (retained-borrow boundary custody); unbisected.
- Counts: +6 tests and +1 net FAIL versus 210ffe3c93; the near-threshold
  member `owned_match_nested_record_replays_every_selected_payload` passed
  at 261s — still slow-flagged, still not a failure.

Residual attribution at 23392bc467 (2026-09-20, Linux x86-64), same
command: 2183 run, 2127 passed, 56 failed (55 FAIL plus
`mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
killed externally after ~1390s at ~570% CPU — the same blowup, still
unbisected, still PROOF-SEARCH-MEASUREMENT's). Every failure maps onto the
recorded families with identical diagnostics — 30 `tests::*` bare
`Service<R>` spellings at `src/tests.rs:82` plus the 3
`unit_plan_omissions` members (33 total, ENTRY-CONTENT-ROOTS); 16 missing
checked transitive machine plans (`provider_attachment_source` ×6,
`unit_state_graph::provider_attachments` ×9,
`guarded_scalar_returns_source::stored_returned_cases_support_borrowed_refined_getters`);
4 scalar-return custody cases; 2 ranked safe-point segment bounds. Both new
families from the 6ef64f6dd6 reading closed since: the three site_guard
crash-namespace rejections now pass
(`landed_affine_sibling_custody_crosses_source_codec_and_independent_verification`,
`bounded_exact_left_shift_uses_only_its_canonical_certificate`,
`erased_arithmetic_prefix_still_requires_its_own_certificate`), and
`closed_record_projections_replay_exact_sources_carriers_and_all_siblings`
passes again at 0.020s — shrinkage, not a new tail; the closed families'
unbisected suspects are in the retained-borrow/result-contract lane.
`owned_match_nested_record_replays_every_selected_payload` passed at 258s —
still slow-flagged, still not a failure. The unattributed tail remains
empty; all 56 members remain owned by the families' named items.

Residual attribution at 71fb20485e (2026-09-21, Linux x86-64), same command
with the nonterminating blowup member filtered out of the pass and run
alone under a 200s bound: 2198 run, 2175 passed, 23 failed — 56 FAIL down
to 23, every member still on an owned family. The blowup
`nominal_affine_source::integer_comparison::mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
persists: run alone it had not finished at 200s when SIGTERM aborted it,
so it remains nonterminating/pathological on this host, still owned by
PROOF-SEARCH-MEASUREMENT.

- Closed since 23392bc467: the 33 bare `Service<R>` fixture spellings
  (`src/tests.rs:82` panic sites) now pass — the fields migrated to
  `Service<R>` carriers at 00a69f066b0; the 2 ranked safe-point segment
  bound members pass again (`terminal-fixed-fuel` charges safe-point rows
  as single block traversals at 29983459ec1); the scalar-return member
  `source_replay_requires_the_exact_affine_return_transfer` now passes;
  and the `established by` call-result qualification member stays closed
  (it passed at the prior reading too — the count change is 33+2+1 net).
- Missing checked transitive machine plan (19 tests). Identical
  `InvalidUnitMachinePlan` "attached Unit closure is missing a checked
  transitive machine plan" diagnostic on the same members as before:
  `provider_attachment_source` ×6 (provider_attached_scalar_result_forwards_to_later_call,
  provider_attachment_tampering_fails_closed,
  provider_backed_main_retains_attachment_and_exact_installation_requirements,
  source_projection_is_deterministic_and_perturbations_fail_closed,
  straight_line_console_projection_accepts_zero_one_two_and_sixteen_writes,
  unused_provider_field_retains_relevance_and_identity_without_boundary_roots),
  `unit_state_graph::provider_attachments` ×9 (all five
  authored_provider_receiver members, canonical_cyclic_attachment_roots,
  checked_attachment_requirements, checked_graph_replays_boundary_call,
  cyclic_provider_fields_reload),
  `guarded_scalar_returns_source::stored_returned_cases_support_borrowed_refined_getters`,
  and the 3 `unit_plan_omissions` members — two surface the same
  InvalidUnitMachinePlan (`a_routed_task_result_into_self_rejects_claim_custody_corruption`,
  `a_routed_task_start_call_plans_and_owned_settle_reaches_module_production`),
  while `a_provider_carrying_argument_still_stops_at_provider_attachment_requirements`
  still rejects but its omission stage assertion no longer matches
  `LocalConstruction{phase:"provider attachment requirements"}` — the
  closure now stops earlier on the same missing plan, so the test's phase
  pin needs updating when the family is repaired. Still the
  provider-attachment lane's claim.
- Scalar-return custody (3 tests, `tests/owned_record_return_source.rs`).
  `discarded_scalar_invocation_precedes_whole_owned_return` still fails on
  the absent unit-effects plan ("ordered body retains a structural result
  independently of preceding scalar calls" — same `terminal_unit_effects
  .for_machine` None);
  `effectful_discarded_call_writes_before_return_across_fuel` still fails
  with `Lowering(Unsupported("composed Unit scalar call requires
  structural call custody"))` at
  `src/unit/attached_unit/composed_control/admission.rs`;
  `source_replay_rejects_return_parameter_and_carrier_substitution` still
  panics unwrapping `plan.structural_result` on `None` — no
  structural-result plan exists to tamper. The fourth member
  (`source_replay_requires_the_exact_affine_return_transfer`) is repaired.
- New — fixed-fuel unranked-loop verdict (1 test):
  `unit_state_graph::bindings::unranked_self_bindings_validate_without_claiming_finite_fuel`
  asserts `terminal_fixed_fuel::derive_fixed_entry_fuel` returns
  `Err(FixedFuelError::ControlCycle(actual))` for the changed entry block
  and the match no longer holds — in-window suspects are the two
  `terminal-fixed-fuel` commits (05115e3ba88 absence-of-bound reports name
  the unbounded cycle component; 29983459ec1 safe-point rows as single
  block traversals). Belongs to the ranked-cycle/fuel lane that owned the
  now-closed segment-bound pair; not bisected.
- The near-threshold member
  `owned_match_nested_record_replays_every_selected_payload` passed at
  218s — still slow-flagged, still not a failure. The unattributed tail
  remains empty: all 24 failures (23 FAIL + the blowup member) are owned.

Residual attribution at b8d336adcf21 (2026-09-21, Linux x86-64), same
command: 2203 run, 2179 passed (14 slow), 24 failed — 23 FAIL plus
`mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
SIGTERM'd at 1431s (~570% CPU, the same blowup, still
PROOF-SEARCH-MEASUREMENT's). The five tests added since 71fb20485e all pass
and every red member re-fails at an identical panic site with an identical
diagnostic — the 19 missing transitive machine plans
(`provider_attachment_source` ×6, `unit_state_graph::provider_attachments`
×9, `unit_plan_omissions` ×3,
`guarded_scalar_returns_source::stored_returned_cases_support_borrowed_refined_getters`),
the 3 scalar-return custody members, and the fixed-fuel unranked-loop
verdict. `owned_match_nested_record_replays_every_selected_payload` passed
slow-flagged at 300s+. Ownership delta only: the bindings.rs verdict member
now sits inside the live FIXED-FUEL-CONTROL-CYCLE-REPIN claim on
`tests/unit_state_graph/bindings.rs`. The unattributed tail remains empty.

Residual attribution at 7b224763615 (2026-09-21, linux x86-64), same
command with the blowup member filtered out and run alone under a 150s
bound: 2203 run, 2180 passed, 23 failed, 1 skipped — the identical
23-member set as the 71fb20485e reading (19 missing transitive machine
plans, 3 scalar-return custody, 1 fixed-fuel unranked-loop verdict); the
five tests added since all pass. The blowup member did not finish at 150s
— still nonterminating/pathological, still PROOF-SEARCH-MEASUREMENT's.

Residual attribution at e7c0099cb2b7 (2026-09-21, linux x86-64), same
command with the blowup member filtered out: 2206 run, 2183 passed, 23
failed, 1 skipped — same headline count as 7b224763615, different
composition. Every member still attributes to an owned family:

- Missing checked transitive machine plan (18 tests, same
  `InvalidUnitMachinePlan` "attached Unit closure is missing a checked
  transitive machine plan" diagnostic). The family's membership shifted
  without changing shape: `provider_attachment_source` ×6,
  `unit_plan_omissions` ×3, and
  `guarded_scalar_returns_source::stored_returned_cases_support_borrowed_refined_getters`
  all pass now (10 members drained), while `unit_state_graph::
  provider_attachments` remains at 9 members (the same nine under the
  reworked `*_rejects_*_with_unchanged_plan` spellings; the omission now
  reads "local construction stopped at structural field store: record
  literal field, state 0") and nine new members joined with the
  identical diagnostic — `retention::conformance_applications` ×3
  (panicking at `src/retention/conformance_applications.rs:477`),
  `tests::composed_operand_catalogs` ×5 (the `dynamic_unit` trio plus
  `dynamic_continuation_operands_preserve_forwarding_and_helper_identities`
  and `dynamic_result_continuation_calls_an_observable_ordinary_unit_body`,
  panicking at `src/tests/composed_operand_catalogs.rs:11`), and
  `tests::composed_unit_internal_calls::
  composed_caller_lowers_a_retained_scalar_graph_call_result` (`Main::main`
  calls scalar `Main::rot`, "neither a registered target nor an ordinary
  body"). Still the provider-attachment lane's claim.
- Scalar-return custody (1 test): only
  `owned_record_return_source::effectful_discarded_call_writes_before_return_across_fuel`
  remains (`Lowering(Unsupported("composed Unit scalar call requires
  structural call custody"))`); `discarded_scalar_invocation_precedes_whole_owned_return`
  and `source_replay_rejects_return_parameter_and_carrier_substitution`
  pass since the 7b224763615 reading.
- New — cyclic scalar-array establishment panic (4 tests):
  `scalar_array_source::cyclic::cyclic_{affine_empty_record,
  trivial_affine_local}_{reestablishes_inside_the_component,
  rejects_when_an_edge_drops_its_disposal}` all panic `index out of
  bounds: the len is 1 but the index is 3` at
  `src/scalar_graph/scalar_graph_lowering/call_lowering.rs:419` (the
  erased-proof-argument roster zip). The members entered with
  f0f808f419989's affine empty-record admission and the
  member-established relocation work at 576b9a76dc49a; an index panic
  is a lowering bug, not an authored rejection. Candidate lane:
  scalar-graph call lowering / LICM relocation.
- Closed since 7b224763615: the fixed-fuel unranked-loop verdict member
  `unit_state_graph::bindings::unranked_self_bindings_validate_without_claiming_finite_fuel`
  passes again; the unattributed tail remains empty — all 23 failures
  sit on owned families.


`cargo nextest run -p checked-trees-to-lowered-psi --no-fail-fast` at
9d0d864656 plus the anonymous-arithmetic repair beside this row (2026-09-18,
macOS arm64) runs the whole crate: 2032 run, 2012 passed, 20 failed. With the
`validation/affine_cleanup/continuation.rs` repair recorded in the
terminal-verifier section it is 1999 passed, 24 failed: that repair also
restores `unit_state_graph::bindings::structural_successors_reject_missing_and_surplus_arguments`,
which asserts the arity diagnostic from the producer side.

Fresh member-by-member reading at 6ef64f6dd6 (2026-09-20, Linux x86-64,
`cargo nextest run -p checked-trees-to-lowered-psi --no-fail-fast`): 2152
run, 2093 passed, 59 failed; the PROOF-SEARCH-MEASUREMENT blowup member
SIGTERM'd at ~892s. Prior family owners reconfirmed at identical panic
sites: 33 bare `Service<R>` spellings, 16 missing transitive machine plans,
3 site_guard crash rejections, 4 scalar-return custody cases, and the
blowup above. The `established by` call-result qualification family closed
in-window (`registered_callback_lifetime` green; 851052b4f8f, 1fc01bb6907).
C2L-UNATTRIBUTED-FAILURE-TAIL's census at 23392bc467 attributes all 56
remaining reds onto owned families with identical diagnostics.

Two families opened by that reading, bisected at c267df86acb8 (Linux
x86-64):

- **Ranked safe-point segment bounds** (2 tests, still red):
  `structural_control_cases::ranked_countdown_lowers_to_verified_resumable_interpreter_execution`
  reads per-edge segments `0x600000000` instead of 3
  (`src/tests/structural_control_cases.rs:1497`), and
  `ranked_u64_countdown_fails_closed_when_fixed_fuel_exceeds_u64` rejects
  `BoundOverflow` (`src/tests/structural_control_cases.rs:1890`). First-bad
  commit 7591b2607c77 ("bound segments through ranked cyclic components") —
  green at its parent, red at the commit; recorded suspect 39e156c73a0 is
  green at itself, cleared by test. The break is the derivation rewrite's
  own component-scale charging (`natural_component_geometry` in
  terminal-fixed-fuel `fuel_certification/outcome_bounds.rs`), not its
  inputs. Owning lane: ranked-cycle/fuel.
- **Closed-projection replay admission** (1 test, closed):
  `expression_preparation::bindings::tests::closed_record_projections_replay_exact_sources_carriers_and_all_siblings`
  admitted invalid/foreign member symbols. First-bad 090802e8a790
  ("evaluate member-read leaves of computed aggregate constants in
  constant position"); recorded suspects 39e156c73a0 and 143636cec8a both
  green, cleared by test. Fixed by 7af30a1f839a ("replay the complete
  closed record projection for scalar member sources"); green at
  c267df86acb8.

The four scalar-return custody failures
(`tests/owned_record_return_source.rs`) are owned by
C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES.

Scoped residual re-verification at `59e0b5ec22d` (2026-09-21, Linux
x86-64; `cargo nextest run -p checked-trees-to-lowered-psi -E
'test(~owned_record_return_source) or test(~scalar_array_source)'` — 80
run, 75 passed, 5 failed): the residual set is unchanged. The single
scalar-return custody member still fails at
`Lowering(Unsupported("composed Unit scalar call requires structural call
custody"))`, and all four `scalar_array_source::cyclic` members still
panic `index out of bounds: the len is 1 but the index is 3` at
`src/scalar_graph/scalar_graph_lowering/call_lowering.rs:419`. Ownership
stands as recorded above: scalar-return custody under
C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES and the cyclic establishment
panic under the scalar-graph call-lowering lane; the unattributed tail
remains empty.

## compiler build-target activation

`mbx nextest run -p compiler --test build_target_activation` — 49/51 pass; 2
fail:
`x86_feature_admission::source_fma_then_attached_unit_call_stays_inside_one_canonical_mxcsr_envelope`
and
`x86_feature_admission::terminal_product_retains_exact_fma_operation_plan_and_x86_admission`,
both with `native artifact native instruction selection failed: FMA provider
transport is not implemented in the common instruction pipeline`. x86 FMA
provider transport is unimplemented; the failure is not host-specific. Fix
leaf: **FMA-PROVIDER-PIPELINE-TRANSPORT** on the board.

Re-measured at 7b224763615 (2026-09-21, linux x86-64): 118 run, 112
passed, 6 failed — the suite grew and two members joined the FMA pair.
The two recorded members persist verbatim (same FMA-transport message).
New in-window:
`x86_feature_admission::admitted_x86_fma_demand_retains_exact_plan_associations`
and `x86_feature_admission::aarch64_fma_demand_is_not_an_x86_feature_association`
fail one gate earlier on the ProgramEntry rejoin diagnostic from the
canary refresh above ("rejoins 0 Terminal attachment identities …
omitted at local construction at `outer calls: statement window`");
`x86_feature_admission::exact_x86_fma_demand_fails_closed_without_feature_admission`
fails on a windows_x86_64 entry-schema mismatch ("require either the
exact bundled Windows x86-64 contract or one accepted package-owned
Windows x86-64 binding, not `<std>/targets/windows_x86_64/entry.omg`");
and
`x86_feature_admission::boundary_operator_and_float_adapters_retain_terminal_execution`
fails at fixture compile on the `Service<R>` gate ("field `console` on
data `Main` names bare boundary trait `Console` in value position; the
intrinsic `Service<R>` carrier is the only service value spelling") —
fixture drift, same migration family as the c2l `tests::*` cluster that
was repaired for the library sources.


## compiler `package_compilation_inputs`

`cargo nextest run -p compiler --test package_compilation_inputs
--no-fail-fast` at c94c4af56e (2026-09-20, macOS arm64): 190 run, 177
passed, 13 failed. Both failures this row previously recorded are closed.
`cross_package_visibility::public_dynamic_return_may_carry_private_producer_selected_evidence`
was fixed at ec36cd564c, bisected: reverting that commit reproduces "state
`code` requires an exact retained loan origin for its shared receiver"
exactly. The float-identity test is now
`module_constants::public_float_identity_preserves_explicit_literal_landings`
and passes.

The 13 live failures are build and packages side, none in Psi, and eight of
them share one message, "provider selection operand does not resolve to one
visible product declaration": six in `authority_and_build_files` and two in
`artifact_identities_and_entries`. Treat those as one provider-selection
regression rather than eight rows. The remainder are
`native_package_entrypoint_uses_the_same_reconciled_binding_mode` (a
`Console` boundary wanting a selected fused provider), two reporting an
authored call-selection occurrence left unresolved after successful
checking, one generated-dependency handoff, and
`one_root_source_cannot_join_both_dependency_scopes`, whose noncanonical
directory mode is bound to the macOS temp directory. Not bisected. Fix
leaf: **BASELINE-PACKAGE-COMPILATION-INPUTS** on the board — its newer
reading at `1f7301b71020` (linux x86-64) already shows the tail down to
three failures, so this section's 13-failure macOS census is stale.

The three composition-mode admission failures earlier recorded on the
COMPONENT-SUBSTRATE board item are closed (0e6c25c4dc attributed, fixed at
65d71153a9).

Re-measured at 7b224763615 (2026-09-21, linux x86-64), same command: 196
run, 194 passed, 2 failed — the 13-member set shrank to two. The
eight-member provider-selection family is down to one:
`artifact_identities_and_entries::free_process_exit_helper_lowers_without_a_synthetic_attachment`
("provider selection operand does not resolve to one visible product
declaration", same message); the other is
`artifact_identities_and_entries::accepted_package_uefi_binding_selects_exact_ordinary_schema`
("authored Call declaration selection occurrence 1673 remained
unresolved after successful checking (CheckedCall)"). The recorded
fused-provider and macOS-temp-dir members are closed.

## package-evidence

`cargo nextest run -p package-evidence --no-fail-fast` at 34cc842d85 plus the
three fixture commits beside this row (2026-09-16, macOS arm64): 632 run,
631 passed, 1 failed:
`authority::toolchain_provenance::declared_hardware_service_reach_does_not_infer_physical_authority`
("domain `Extent::Granted` establishment route `ExtentRootProvider::grant`
does not resolve to one exact trait"). The fixture package declares its own
`pub boundary trait ExtentRootProvider {}` and imports nothing; since
5d134569b6 seeded the hosted entry contract and its `core` imports into
every hosted package-aware compilation, `core/extent.omg` is in the program
and `syntax-trees-to-symbol-resolved-trees/src/selection/signature_free_requirements.rs`
(`resolve_signature_free_requirement`) collects every same-named trait
program-wide, filtered only by resolution stratum, so the package's trait and
`core`'s collide as `TraitNotUnique`. Per wiki/spec/language/modules.md a
package's declarations are not in `core`'s scope, so the fixture is valid and
the resolver over-collected. Repaired at `9898f252ec` ("psi: signature-free
trait routes scope candidates to the occurrence's package") with
`0d51bad72a`: `signature_free_trait_candidates` and
`signature_free_machine_candidates` now select through
`lookup_signature_free_top_level_from_source_matching(..., use_span, ...)`
rather than collecting every same-named top level program-wide, and
`tests/module_namespace_residuals.rs::signature_free_route_keeps_the_imported_trait_with_an_unimported_competitor`
pins the imported trait winning over an unimported competitor. This paragraph
is retained for the diagnostic's history; the over-collection it describes is
closed.

Re-measured at 7b224763615 (2026-09-21, linux x86-64), same command: 662
run, 616 passed, 46 failed — the recorded member passes, but the suite
carries a large in-window regression tail. The failures cluster by
diagnostic, not by module:

- `Service<R>` routing gates — "places the routed `Service<R>` carrier
  outside the first direct field/owned concrete-machine-parameter rung",
  "the core `Service` carrier is closed; it admits no authored
  [supplements]", and "no `domain` named `Bound` is declared for
  `Service<ClockHost>`" — hit the `callable_policy::{flows,mutation}`
  fixtures (`reachable_flow_keeps_private_authority_without_including_unreachable_helpers`,
  `transitive_capability_flow_retains_all_five_verbs_without_helper_coordinates`,
  `float_and_boundary_source_views_restore_both_nesting_orders`,
  `selected_service_receiver_write_frame_rejoins_checked_source`) and
  `operational::authored_sources::authored_service_reaches_retain_exact_review_sources_and_empty_ceiling_presence`
  ("trait `Child` requires unknown trait `Service`"), plus several
  `terminal_permission_policy` members.
- "canonical `FilesystemHost` realization table names `close`, which the
  accepted service schema does not declare" — 10 diagnostic instances,
  the largest single-message family
  (`boundary_supply::boundary_bodies::empty_boundary_body_is_checked_callable_and_remains_directly_invocable`
  among them).
- "cannot prove requires contract for specification call `observes`" —
  8 instances.
- "reviewed callable `…` contract-call intrinsic identity disagrees with
  its exact checked call-selection row or is not yet represented by
  package review" — the whole
  `contract_expressions::evidence_calls`/`collection_views` cluster (9
  members) plus `obligation_ledger` ×2.
- "authored Operator/Call declaration selection occurrence … remained
  unresolved after successful checking" — `public_api` ×3.
- `calling_policy_{source,substitution}` ×6 panic on
  `Option::unwrap()` on `None` at
  tests/calling_policy_substitution.rs:17 — a fixture helper returning
  None, likely fixture shape drift rather than a verdict change.
- `boundary_calling_plan_realizations().is_empty()` pins now observe
  rows: `representation_policy::unused_placement_and_semantic_copy_selections_both_remain_visible`,
  `selected_provider_policy::families::unused_selected_plan_retains_its_explicit_grant`,
  `selected_provider_policy::signatures::source_qualified_service_types_do_not_require_a_calling_policy`.
- Provider-selection operand refusal ("does not resolve to one visible
  product declaration") in `source_custody` and `terminal_permission_policy`
  members; a `CheckedMath::convert` static-telescope arity (0 vs 2)
  mismatch in `selected_provider_policy::signatures::empty_static_service_rejects_changed_authored_lifetime_binders`;
  `exact_contract_identity`'s `builtin_function_review_rejects_checked_target_symbol_tamper`
  (projection shape drift), `trait_contracts` ×2, and
  `capture::quotients::tests::total_direct_define_projects_one_deterministic_recoverable_review_row`
  complete the set. Unbisected; none of the 46 names overlap the recorded
  row.

## native-differential `terminal_psi_source`

`cargo nextest run -p omega-native-differential-test --test terminal_psi_source
--no-fail-fast` at 9cf696b9b5 plus the compile repair beside this row on
2026-09-17 (macOS arm64): 90 run, 82 passed, 8 failed, in three pre-existing
families. (65dc530cef removed `ArtifactEmissionPolicy` but left two
`with_artifact_policy` calls in this target, so it did not compile between
that commit and the repair; 51f21bb168 moved the emptied-scalar-contract
rejection to the earlier `scalar contract lost authored requirements` gate
and the expectation now names it.)

Partial refresh at `d6a0625f6ba4` (2026-09-21, linux x86-64): the staged-local
member below now passes. The hosted-receiver custody five remain live; two of
the original eight failures were never itemized here.

- Hosted-receiver custody (5): `control_flow_cleanup_source_reaches_the_publication_gate`,
  `unimplemented_post_terminal_phase_rejects_before_native_publication`,
  `selected_preterminal_optimizers_rejoin_one_native_pipeline`,
  `selected_progress_free_source_stages_non_visible_terminal_candidate`,
  `selected_source_entry_retains_build_bound_progress_for_terminal_publication`
  fail with `native artifact ProgramEntry receiver provisioning failed: hosted
  receiver requires exact checked initialization and cleanup custody`
  (`native-realization/src/native_realization.rs`, since 96854008a0 and
  4f65a07840): the target's `stage_terminal_component_with_policies` helper
  builds the native request by hand and never supplies the checked entry the
  production route attaches through `with_checked_entry`. Repair is a harness
  migration onto the stage crate's route (**ENTRY-CONTENT-ROOTS** area).
- ~~`locals_calls_and_short_circuit::checked_source_staged_local_sequences_before_an_explicit_crash`
  (1): `UnsupportedControlFlow(MachineId(1))` from
  `abstract-operations-to-target-operations/src/lowering/control_flow.rs`;
  expectation from 2694d433d3, not bisected.~~ Verified stale: the member
  now passes end to end at `d6a0625f6ba4` (linux x86-64, 9.9 s) — staged
  local sequences lower through the target-operation route.

Closed at 7b224763615 (2026-09-21, linux x86-64): the target compiles and
all six recorded members pass — the five hosted-receiver members (the
harness migrated onto the checked-entry route) plus
`checked_source_staged_local_sequences_before_an_explicit_crash` (the
`UnsupportedControlFlow(MachineId(1))` verdict is gone).

## native-differential compile floor (2026-09-20 refresh, Linux x86-64)

`cargo nextest run -p omega-native-differential-test --all-targets
--no-fail-fast` does not compile at c2ccb2a202 (also at 2abada2995):

- `pipeline_ownership/fixtures/ordinary_graph_controls.rs:32` —
  non-exhaustive match on `LegalizedScalarTerminator::Crash`, added at
  bf8769cce1 without the fixture arm (file inside the live
  STRUCTURAL-UNIT-CALL-GRAPH-JOINS claim).
- `abstract_publication/decision_custody.rs:58` — asserts a 6-entry
  catalog, but `PSI_PASS_CATALOG` has carried 7 entries since 9a9d1a8b32
  (file inside NATIVE-DIFFERENTIAL-MATRIX / GRAPH-FEATURE-PROJECTION-SCHEMA
  claims).

Running the 26 buildable targets at 2abada2995 (643 tests): 531 passed,
112 failed, 1 skipped — dominated (~75) by fixture sources still spelling
bare boundary-trait fields (`console: Console`, `fs: FilesystemHost`),
rejected since 32f5182254 requires the intrinsic `Service<R>` carrier;
plus stale `omega_language_std/` paths in `gui_headless`/`recast_views`
(layout removed ~24ab0f1c87), a stale `lower_to_target_operations is_err`
negative (crash leaves emit natively since bf8769cce1),
`InvalidUnitMachinePlan` attached-closure omissions, hosted-receiver
custody rejections, and proof-subject mismatches. A 213-test rerun of the
red set at c2ccb2a202 left ~75 failing in the same families plus a new
"composed Unit scalar call requires structural call custody" diagnostic in
`scalar_case_results`.

## native-differential `pipeline_ownership`

`cargo nextest run -p omega-native-differential-test --test pipeline_ownership
--no-fail-fast` at 7176821bc6 plus the repairs beside this row (2026-09-20,
macOS arm64): 392 run, 387 passed, 5 failed. The target could not compile
until 7176821bc6, so these expectations drifted unseen. Four are repaired:
the compiler baseline budget pin follows ed8d2d6d8d's aligned iteration
ceiling; the fixed and precolored segment-home usage pins follow 1f6f851930's
incremental conflict accounting, with the domain and assignment counts and
all four receipt identities unmoved, so the plan did not change and only the
two counters that commit reduced did; and the text-section manifest corpus
now substitutes tag 3, because d5e8ceef51 gave tag 2 a real meaning, which
had quietly turned that corruption into a parse that failed one check later
instead of the unknown-tag rejection it names. That repair keeps an
assertion on tag 2 as well, so the file gained custody coverage rather than
losing it.

- Structural Unit fail-closed (5):
  `stages::realization::structural_units::leaf_object::structural_extent_unit_leaf_reaches_canonical_object_artifact`,
  `..::publication::claim_completion_prefixes_publish_as_metadata_without_instruction_spans`,
  `..::publication::installed_structural_provider_call_reaches_shared_publication`,
  `..::publication::structural_call_publication_preserves_owned_indirect_arguments`
  and `..::structural_call::structural_unit_call_reaches_post_allocation_machine_custody`
  stop at `UnsupportedControlFlow` from
  `abstract-operations-to-target-operations/src/lowering/control_flow.rs`.
  These are one cause, not five: `lower()` rejects any function whose
  structural parameters carry qualifications, `unobserved_owned::parameter`
  demands the same emptiness, and every fixture declares a granted extent.
  Probed by stripping that one domain from the fixtures, then reverting: the
  leaf test passes end to end through object artifact, image emission and
  installation replay, so the qualification gate is its only blocker.
  `claim_completion_prefixes` is additionally held by the `entry_claims` arm
  of the same gate, and the two publication legs additionally fail
  `InvalidInternalUnitCall` in image emission's installation-record builder.
  The spec makes qualifications "semantic metadata, not additional ABI
  words" and retains them independently of the native parameter contract, so
  this is an explicit implementation limit rather than a semantic rule. The
  expectations are deliberately not rewritten to assert the rejection: these
  are route tests, and pinning the refusal would leave five tests observing
  nothing. Owned by **STRUCTURAL-UNIT-CALL-GRAPH-JOINS**; the two lowering
  files are under a live CML4 claim and the image-emission area under
  WIRE-RUNTIME-AND-INSTALLATION.

  Closed at 7b224763615 (2026-09-21, linux x86-64): all five members pass.
  The qualification gate no longer stops `lower()` —
  `structural_extent_unit_leaf_reaches_canonical_object_artifact`,
  `installed_structural_provider_call_reaches_shared_publication`,
  `structural_call_publication_preserves_owned_indirect_arguments`, and
  `structural_unit_call_reaches_post_allocation_machine_custody` all run
  end to end, and the recorded `claim_completion_prefixes_...` member
  appears renamed to
  `claim_completion_machines_fail_closed_at_native_lowering`, which also
  passes. The `UnsupportedControlFlow` and `InvalidInternalUnitCall`
  verdicts are gone.

## checked-interpreter integration tests

`cargo nextest run -p checked-interpreter --test borrowed_subslices` at
c7465c23bc (macOS arm64): `inline_const_generic_selectors_execute_distinct_inferred_extents`
fails at checking with "machine `Main::endpoint` state `endpoint` terminal
expression returns a value not provably within its declared range" for a
`<const N: u64>` endpoint returning a `u64` constrained to 0 through 3 by the
legacy scalar range-annotation suffix, from an inferred extent; a
generics checker gap (**STRUCTURAL-GENERIC-MATCHING** / **RUNTIME-VALUE-GENERICS**
areas, both under live claims when recorded). This historical diagnosis does not
endorse the removed source syntax. `REMOVE-BRACKETED-RANGE-ANNOTATIONS` on the
[board](../../TASKS.md) tracks migration of the compiler and unchanged fixture;
no replacement-syntax validation is claimed here.

Closed at 7b224763615 (2026-09-21, linux x86-64):
`inline_const_generic_selectors_execute_distinct_inferred_extents`
passes; the `borrowed_subslices` members now live under the crate's
`--test suite` target.

## workspace `--lib` red cluster

> Refresh at `2dbfecd98e` (2026-09-21, linux x86-64, NEW-KBF-LIB-CLUSTER-AND-
> STALE-ROWS-REFRESH leg): the gate is **unmeasurable** — the workspace does
> not compile. `StructuralTypeShape::ElementView` (`04f2fdbb853`, merged via
> `48873065b72`) makes every exhaustive match non-exhaustive;
> `terminal-verifier` (8 sites: `validation/foundation.rs:555`,
> `foundation/structural_types.rs:55/:114`, `references.rs:119`,
> `affine_cleanup.rs:581`, `structural_operations/structural_arguments.rs:307`,
> `structural_paths.rs:25`, `structural_result_contracts.rs:174`) and
> `optimization-unit` (`identity/structural_encoding.rs:379`) fail E0004,
> and the `image-emission` structural-type codec encode match needs the new
> variant plus a wire tag. Every named member below depends on that chain and
> is unwitnessable — unwitnessable is not green; membership pending the
> consumer legs is carried from the last measurable reading.

`cargo nextest run --workspace --lib --no-fail-fast` members that fail on
linux x86-64 at `0f75a052f09` (2026-09-21), carried into this doc by exact
name per **KNOWN-BASELINE-FAILURES-REFRESH**; the owning row is
**RC-REPOSITORY-CLOSURE** (TASKS.md), whose macOS reading at `5958706064`
recorded a superset — 13 of 16,169 — with different membership: the
checked-trees-to-lowered-psi `structural_control_cases::ranked_*` pair, the
compilation-report `pcc::native_evidence` member, the compiler
`native_publication_writes_only_declared_products` member and the
external-roots `stack_and_fuel::fixed_fuel` member all pass on linux here,
while four terminal-codec members fail only on macOS
(`sections::structural_block_wire_tests::{primitive_local_operations_round_trip_with_exact_result_and_operand_identities,structural_block_bindings_round_trip_and_bind_each_argument_order}`,
`sections::trust_graph::tests::{canonical_bytes_and_decoder_bind_signature_wire_and_declaration_validation,current_graph_is_closed_canonical_and_explicitly_not_fully_derived}`).
Neither host reading alone is the baseline.

- package-manager (6):
  `operations::check_project::tests::semantic::retained_check_root_uses_final_consumer_bindings_and_requested_entry`,
  `operations::compile_project::tests::receiving_admission::accepted_console_customer_receives_admission_only_under_a_sufficient_policy`,
  `review::candidate::compilation::tests::discovery_proposes_the_root_console_exit_permission_as_a_blocking_row`,
  `review::candidate::compilation::tests::discovery_proposes_the_root_console_output_and_input_permissions_per_declared_leaf`,
  `review::candidate::compilation::tests::discovery_proposes_the_root_filesystem_cohort_permissions_per_declared_leaf`,
  `review::candidate::compilation::tests::review_publishes_the_named_component_description_for_an_independent_selection`
  (witnessed: 335 run, 6 failed at `0f75a052f09` — the same membership as
  the macOS row's reading).
- native-realization (1):
  `native_product::realization::tests::exclusion_taking_entry_reaches_mechanism_adjudication`.
- package-evidence (1):
  `capture::quotients::tests::total_direct_define_projects_one_deterministic_recoverable_review_row`.

## Host note (Linux x86-64, 2026-09-20)

- `cargo fmt --all` is not clean at c2ccb2a202: it rewrites ~18 unrelated
  files (external-roots interrupt table, module_machine_indices,
  multiplicity/termination checks, validation tests, ...). Treat
  repo-wide fmt output as drift, not signal; `cargo fmt --check` on touched
  files only is the scoped gate.
- `tools/verify.py` requires `tomllib` (Python 3.11+); this host runs
  3.10. Equivalent raw `omega --check --build-dir <dir> <app>/main.omg`
  and `omega run --keep <app>/main.omg` invocations cover the same legs.

## Host note (macOS)

`rust-objcopy` emits `dyld: Library not loaded: @rpath/libLLVM.dylib` (SIGABRT)
during test-binary linking on the pinned `nightly-2026-09-04` toolchain on
this host. It is a warning in build output only and does not fail compilation
or tests; do not investigate it as a test failure.

## Intel macOS host gap (x86_64-apple-darwin)

Environmental, not a test regression, verified across the macw3 wave on
2026-09-14 (independent sessions reproduced identical sets on unmodified
bases including `d1b7165cd6`, `07782416b4`, and `1055e88f31`):

- `TargetProfile::host()`
  (`omega-rust/omega/representations/target/src/lib.rs`, `host()`) has cfg
  arms for macos-aarch64, linux-aarch64, linux-x86_64, and windows-x86_64
  only, so host-profiled tests panic with `unsupported host profile for
  Omega native planning`. Observed sets: ~20 `package-manager` ops tests and
  2 `compiler::request` tests in the workspace `--lib` run.
- `native_hosted_target()` in `compiler/tests/canary_suite.rs` has the same
  four cfg arms, so `mbx nextest run -p compiler --test canary_suite` does
  not compile on this host.

On this host, route `omega` invocations through an explicit `--target` (for
example `linux_x86_64`) and report native-host coverage as unavailable rather
than re-running the baseline; a `MacosX64` host profile is open board work,
not a fix to inline into a task.

## workspace `--lib` cluster (RC-REPOSITORY-CLOSURE)

The `cargo nextest run --workspace --lib --no-fail-fast` leg of
**RC-REPOSITORY-CLOSURE** ([TASKS.md](../../TASKS.md)) measured RED 13 of 16,169
on linux x86-64 at `5958706064` (2026-09-20, cargo, no mbx). That row owns the
live measurement; the names are carried here so an exact-name search finds the
attribution. Every member below reproduces at that revision without a task
diff:

- `package-manager` 6:
  `operations::check_project::tests::semantic::retained_check_root_uses_final_consumer_bindings_and_requested_entry`,
  `operations::compile_project::tests::receiving_admission::accepted_console_customer_receives_admission_only_under_a_sufficient_policy`,
  `review::candidate::compilation::tests::discovery_proposes_the_root_console_exit_permission_as_a_blocking_row`,
  `review::candidate::compilation::tests::discovery_proposes_the_root_console_output_and_input_permissions_per_declared_leaf`,
  `review::candidate::compilation::tests::discovery_proposes_the_root_filesystem_cohort_permissions_per_declared_leaf`,
  `review::candidate::compilation::tests::review_publishes_the_named_component_description_for_an_independent_selection`.
- `checked-trees-to-lowered-psi` 2 — both green at `42759dd5295`
  (2026-09-21, linux x86-64, direct nextest re-run):
  `tests::structural_control_cases::ranked_countdown_lowers_to_verified_resumable_interpreter_execution`,
  `tests::structural_control_cases::ranked_u64_countdown_fails_closed_when_fixed_fuel_exceeds_u64`.
- `compilation-report` 1 — green at `42759dd5295`:
  `pcc::native_evidence::tests::section_round_trips_canonically`.
- `compiler` 1 — green at `42759dd5295`:
  `compiler::tests::native_publication_writes_only_declared_products`.
- `external-roots` 1 — still red at `55f2c09f15699` (2026-09-21, linux
  x86-64, direct re-run); current signature is the ranked component-scale
  segment-bound family (per-edge segments read `3 << 33` instead of `3`,
  `[1, 25769803776, 25769803776, 25769803776, 1]` vs expected
  `[1, 3, 3, 3, 1]`), the same family the
  CHECKED-TREES-TO-LOWERED-PSI-UNATTRIBUTED-SET bisect pinned to first-bad
  `7591b2607c77` — owned by the ranked-cycle/fuel lane:
  `stack_and_fuel::fixed_fuel::tests::installed_natural_cycle_safe_point_catalog_binds_to_one_occurrence`.
- `native-realization` 1:
  `native_product::realization::tests::exclusion_taking_entry_reaches_mechanism_adjudication`.
- `package-evidence` 1 — a superseded assertion owned by
  **QUOTIENT-THEOREM-LIFT**: the test asserts
  `typed_trees_to_checked_trees::lower_typed_trees` rejects the proof-only
  `Quotient::define` request, which that row now admits through checked
  lowering (termination-gated `admit_checked_quotient_requests`); the
  compiler is behaving as landed and the fixture needs a respell there:
  `capture::quotients::tests::total_direct_define_projects_one_deterministic_recoverable_review_row`.

Host divergence: a macOS aarch64 reading of the same gate at `7b7d0fa8d5`
(12 failed of 16,290) reproduces the package-manager 6 and the
native-realization and package-evidence singletons, passes the
checked-trees-to-lowered-psi 2 and the compilation-report, compiler and
external-roots singletons, and adds four terminal-codec members that pass on
linux — `sections::structural_block_wire_tests::{primitive_local_operations_round_trip_with_exact_result_and_operand_identities,structural_block_bindings_round_trip_and_bind_each_argument_order}`
and `sections::trust_graph::tests::{canonical_bytes_and_decoder_bind_signature_wire_and_declaration_validation,current_graph_is_closed_canonical_and_explicitly_not_fully_derived}`
(the two trust_graph members fail on a missing graph root and were confirmed
pre-existing by re-running at `50559da3ab`). Neither reading alone is the
baseline; cite the row for the matching host.

Re-verified at `42759dd5295` (2026-09-21, linux x86-64, per-name nextest
re-runs rather than the full suite): the live linux set is the 9 names not
marked green above — the `package-manager` 6 plus the `external-roots`,
`native-realization` and `package-evidence` singletons all reproduce; the
`checked-trees-to-lowered-psi` pair and the `compilation-report` and
`compiler` singletons now pass and are marked accordingly. The failure
signatures on this host are `Service<R>`-spelling stale fixtures
(`0e1977994b9`'s bare-boundary-trait rejection — the package-manager 6 and
the `native-realization` member are stale fixtures under the
**ENTRY-CONTENT-ROOTS** migration, not checker regressions; the
receiving-admission member instead names
`select_provider<Console, ConsoleNativeProvider>` unqualified and fails
operand resolution), the unqualified
`select_provider` operand spelling (`4406917695b`), a quotient-capture
refusal, and the ranked component-scale segment-bound family
(`7591b2607c77`'s component-scale charging; the external-roots member now
exhibits it — re-run at `55f2c09f15699`) — no unexplained failures.

Refresh at `2dbfecd98e` (2026-09-21, linux x86-64): superseded by the
build-break — `04f2fdbb853`'s `StructuralTypeShape::ElementView` vocabulary
addition (merged `48873065b72`) left every consumer match non-exhaustive
(terminal-verifier ×8, optimization-unit ×1, image-emission codec, plus
downstream lowering crates), so `cargo nextest run -p package-manager --lib`
cannot even link the dep chain. The 9 live names above are **unwitnessable
at this HEAD, not repaired** — re-run them once the consumer legs land
(ElementView arms + codec tag + the checked-trees-to-lowered-psi emission
flip for `borrowed slice view has no Terminal descriptor`).
