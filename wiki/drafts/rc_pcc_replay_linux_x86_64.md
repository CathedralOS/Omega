# RC-PCC-REPLAY — linux_x86_64 row

Witnessed row of the `RC-PCC-REPLAY` release gate on the Linux x86-64
host. Recorded at revision `18cebfa1062bf07d80a7adaab7169c93e605fdb5`
(2026-09-21), host `x86_64-unknown-linux-gnu`, rustc
1.100.0-nightly (a69a63265 2026-09-03), cargo-nextest 0.9.144 — `mbx`
absent on this host, so `cargo`/`cargo nextest` was substituted verbatim
per the board row's explicit allowance.

Verdict: **red** — and now for an additional, independent reason. The
row's hypothesis is confirmed: `checked-trees-to-lowered-psi::suite
nominal_affine_source::integer_comparison::mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
never returned a verdict (SIGTERM'd at 1383.9s under an external 1500s
harness bound; the repo's nextest profile configures no per-test timeout).
But even if that member converged, the gate is red on 53 ordinary
failures across all five named packages.

## Command 1 — `nextest run -p checked-trees-to-lowered-psi -p terminal-codec -p terminal-verifier -p terminal-interpreter -p terminal-psi-to-abstract-operations --no-fail-fast`

3840 tests run in 1488.6s: **3786 passed** (9 slow), **54 failed**, 0
skipped. Per-package:

| package | pass | fail | verdict-blocker |
|---------|------|------|-----------------|
| checked-trees-to-lowered-psi | 2181 | 22 + 1 SIGTERM | hang + InvalidUnitMachinePlan family |
| terminal-codec | 359 | 22 | canonical marker-byte drift family |
| terminal-verifier | 842 | 4 | nominal-affine-cleanup rejection family |
| terminal-interpreter | 290 | 3 | affine-discard + extended-format family |
| terminal-psi-to-abstract-operations | 114 | 2 | unsupported structural/control-flow legs |

## Command 2 — `test --doc -p checked-trees-to-lowered-psi -p terminal-codec -p terminal-verifier -p terminal-interpreter -p terminal-psi-to-abstract-operations`

**green** — all five packages report 0 failed; the only doctest in the
set is the `terminal-psi-to-abstract-operations` compile-fail doctest on
`artifact_admission::retention::AdmittedOptimizationArtifact`, which
passes.

## Failure inventory with owning-row attribution

### Family A — checked-`Unit`-plan / provider-attachment / structural-result lowering — 22 c2l members

18 panic on `InvalidUnitMachinePlan { reason: "attached Unit closure is
missing a checked transitive machine plan" }` (checked `Unit` transitive
plan absent; e.g. `Main::main`/`Counter::run` "has no admitted body");
the remainder are sibling signatures of the same surface —
`a_provider_carrying_argument_still_stops_at_provider_attachment_requirements`
drifted on the omission stage (`matches!(omission.stage,
LocalConstruction{provider attachment requirements})` no longer holds),
`effectful_discarded_call_writes_before_return_across_fuel` hits
`Lowering(Unsupported("composed Unit scalar call requires structural
call custody"))`, and `source_replay_rejects_return_parameter_and_carrier_substitution`
unwraps a `None` in its replay setup. This is the known
transitive-plan residual recorded on BUILD-SEMANTIC-EXCLUSIONS / the
direct-Unit-crash-planning leg — owning rows STRUCTURAL-UNIT-LOWERING,
PROVIDER-ATTACHMENT-MACHINE-PLAN, C2L-RESIDUAL-FAILURE-ATTRIBUTION,
CONSERVATION-CONTRACT:

- `provider_attachment_source::*` — 6 members (provider_attached_scalar_result_forwards_to_later_call, provider_backed_main_retains_attachment_and_exact_installation_requirements, provider_attachment_tampering_fails_closed, straight_line_console_projection_accepts_zero_one_two_and_sixteen_writes, source_projection_is_deterministic_and_perturbations_fail_closed, unused_provider_field_retains_relevance_and_identity_without_boundary_roots)
- `unit_state_graph::provider_attachments::*` — 9 members (all the authored_provider_receiver_rejects_* / canonical_cyclic_attachment_roots / checked_attachment_requirements / checked_graph_replays / cyclic_provider_fields legs)
- `unit_plan_omissions::*` — 3 members (a_provider_carrying_argument_still_stops_at_provider_attachment_requirements, a_routed_task_result_into_self_rejects_claim_custody_corruption, a_routed_task_start_call_plans_and_owned_settle_reaches_module_production)
- `guarded_scalar_returns_source::stored_returned_cases_support_borrowed_refined_getters`
- `owned_record_return_source::*` — 3 members (discarded_scalar_invocation_precedes_whole_owned_return, effectful_discarded_call_writes_before_return_across_fuel, source_replay_rejects_return_parameter_and_carrier_substitution)

### Family B — canonical format-marker drift — 23 codec/interpreter members

Pinned fixtures still encode/expect format marker byte `0x67` (103); the
current encoder emits `0x68` (104) and the decoder rejects the pinned
bytes as `UnsupportedFormatMarker(103)`. Representative assertion:
`left: [104, 0, 107, 0], right: [103, 0, 107, 0]` (canonical
bounded_integer_fields). The bump was landed without repinning the codec
corpus — same pattern as the omega-request gate repin miss. Owning
family: canonical terminal-Psi format pinning / REPRESENTATION-OWNERSHIP
contract (codec surfaces fenced this wave by DURABLE-CODEC-EXTRACTION):

- `terminal-codec::suite canonical::*` — 11 members (bounded_integer_fields, operation_crash_contracts, structural_call_results, suspension_and_scalar_round_trips×9 incl. current_vocabulary_has_one_stable_canonical_encoding_and_identity, natural_ranking, nominal_affine_unit_return, partial_affine_unit_return, proof_recursive_components, ranked_countdown, scalar_return, suspension_call_plan)
- `terminal-codec` (lib) `sections::semantic_module::structural_block_wire_tests::*` — 2 members
- `terminal-codec` (lib) `sections::trust_graph::tests::*` — 2 members (canonical_bytes_and_decoder_bind_signature_wire_and_declaration_validation unwraps None; current_graph_is_closed_canonical_and_explicitly_not_fully_derived loses the canonical-bytes root)
- `terminal-codec::suite ledger_spike::*` — 3 members (exact_current_terminal_bytes fixtures, full reviewed-replacement hex printed in stderr)
- `terminal-codec::suite publication::*` — 3 members — **these are the hostile/substituted-evidence legs themselves**: destination_changes_only_at_the_validated_rename_boundary, tool_accepts_the_declared_nonzero_success_status_and_expected_artifact, valid_but_substituted_terminal_meaning_rejects_when_expected_is_bound — all fail on `Decode(UnsupportedFormatMarker(103))` before the substitution check runs, so the gate's core claim is currently exercised only up to the decode boundary, not against current-bytes evidence
- `terminal-codec::suite quotient_correspondence::quotient_correspondence_round_trips_and_enters_module_identity`
- `terminal-interpreter::unit case_membership::projected_case_encoding_requires_the_extended_operation_format` — `left [104,0] right [102,0]` — expects the pre-bump extended marker

### Family C — terminal-verifier nominal-affine-cleanup rejection — 4 members

`structural_unit::nominal_affine_cleanup::*` — the four negative legs
expect `ModuleError::InvalidNominalAffineCleanup` and now get acceptance
or a different error (executable_nominal_affine_cleanup_rejects_contract_or_unattached_helpers,
nominal_affine_cleanup_rejects_contract_carrying_members_and_self,
one_call_nominal_affine_cleanup_rejects_nonexact_closures,
two_call_nominal_affine_cleanup_rejects_repeated_or_nonempty_helpers).
Owner family: the structural-unit cleanup contract lanes.

### Family D — terminal-interpreter affine-discard accounting — 2 members

`affine_cleanups::conditional_commits_only_the_selected_affine_cleanup_after_edge_charge`
and `affine_cleanups::scalar_return_performs_affine_discard_only_after_edge_charge`
— `InvalidModule(ScalarReturnAffineDiscardsMismatch)`. Owner family:
affine-cleanup edge-charge accounting.

### Family E — terminal-psi-to-abstract-operations unsupported legs — 2 members

`partial_affine_call_results::continuations::source_continuations_retain_distinct_result_owners_and_ordered_residuals` —
`UnsupportedStructuralArray(StructuralTypeId(4))`, and
`scalar_affine_cleanup::structural_return::omega_preserves_exact_singleton_structural_return_custody`
— `UnsupportedControlFlow(MachineId(1))`. Owner family: a2t structural
/control-flow lowering legs (COORDINATOR-SCOPE-AUDIT territory).

### Family F — non-termination — 1 member

`checked-trees-to-lowered-psi::suite nominal_affine_source::integer_comparison::mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
— SIGTERM'd at 1383.9s under the harness bound; reproduces the ~892s/~900s
readings recorded on C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT. This member
alone previously masked the gate; with `--no-fail-fast` the rest of the
suite still reported around it, so the gate is now also red on 53 ordinary failures across families A–E.

## Disposition

This row is a measurement record only — no source or fixture files were
modified. The gate cannot produce a green row at this revision: the
hang needs the C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT decision (bounded
search or fail-closed refusal, not a longer timeout), and the 53
ordinary failures need their owning rows (A: c2l transitive plan; B:
codec fixture repin; C: verifier nominal-affine rejection; D: interpreter
affine discards; E: a2t unsupported legs). Notably the PCC-specific legs
in `publication::*` fail on the marker-byte drift before the hostile/
substituted-evidence path executes, so family B must be repinned before
this gate can even exercise the contract it names.

The gate remains a matrix row: the release contract still requires all
eight gates green on one clean commit across the four required hosts;
this records the linux_x86_64 leg.
