# Replacement rejection inventory

Catalog of every rejection obligation
[component publication](../../spec/build/component_publication.md) and the linked
boundary specs name for component replacement, mapped to the site that enforces
it at `340e2b5ca4`. Each row is either enforced (site + test) or residual
(owner item on TASKS.md). Delete this draft when the residual rows have landed
and the ledger/journal modules stand alone as the replacement admission
surface.

## Enforced: era admission and lifecycle

Source of every rule below: `component_publication.md` §Service bindings and
era entry, §Replacement and coexistence; enforcement lives in
`omega-rust/omega/representations/effects/src/component_eras/`.

| Spec obligation | Enforcement | Pinning tests |
| --- | --- | --- |
| Candidates cannot publish themselves; publication is a supervised ledger operation | `component_era_entry_ledger.rs` `publish` takes a candidate + receipt, never self-issues | `publication_enforces_retention_limit_and_receipt_identity` |
| Era candidate must be complete (non-zero era/report identity, non-empty contract/plan identities, entry-plan admission receipt) | `publish` rejects "component era candidate is incomplete" | same |
| `max_live_eras` is a private capacity, not a change to replacement meaning | `publish` rejects "exceeds the live-era retention limit" | same |
| Replayed or zero publication identity rejects | `publish` rejects "publication receipt is zero or replayed" via `consumed_publications` | `leave_and_retirement_receipts_cannot_drift_or_replay` |
| A duplicate live era identity rejects | `publish` rejects "already live" | `publication_enforces_retention_limit_and_receipt_identity` |
| Publication must bind and expose the exact candidate — contract identities, previous-era, occurrence digest, compatibility-report, entry-plan and admission-receipt identities all exact | `publish` rejects "does not bind and expose the exact candidate" | `publication_receipt_retains_the_complete_candidate_not_only_compact_ids` |
| A replacement must close the previous era to future entry | `publish` rejects "does not close the previous era to future entry" when `previous_era_closed` is unset | journal-side `replay_rejects_replayed_identities_and_reordered_facts` |
| A candidate's executable-TCB manifest must admit under the coexisting report | `publish` forwards `executable_tcbs.admit_era` diagnostics | `program_local_root_epoch_lease_requires_current_open_exact_contract_and_artifact` |
| Entry linearizes exactly once into the current open era — non-zero fresh invocation, exact contract/era/plan identities, linearized flag | `enter` rejects "does not linearize exactly once into the current open era" and reports "no published era" / "absent from its ledger" | `routing_switch_closes_old_entry_but_retains_its_active_invocation` |
| Leave must match the entered invocation, contracts, era, plan, completion flag, and an actually-active entry | `leave` rejects the non-exact receipt | `leave_and_retirement_receipts_cannot_drift_or_replay` |
| Retention/pin accounting: a program-local-root epoch lease must name the current open era with exact contract and artifact | `acquire_program_local_root_epoch_lease` four reject sites | `program_local_root_epoch_lease_requires_current_open_exact_contract_and_artifact`, `epoch_lease_release_rejects_ledger_substitution_and_identity_replay` |
| Era removal waits for closed entry, zero active entries/pins, complete dispositions | `live_program_local_root_epoch_lease_blocks_quiescence_and_retirement` | ledger + journal retirement rejects |

## Enforced: journal replay (restart-truthfulness)

Source: the journal must refuse any fact sequence the live ledger would not
have accepted — `component_era_journal.rs` `replay` gives every refusal a
positioned `ComponentEraJournalReplayError`. Covered: incomplete candidates,
foreign binding/entry contracts, non-current previous era, zero/replayed
publication identity, already-live era, unexposed candidate, unclosed previous
era, foreign-contract entry/leave facts, entries naming a non-live era, leaves
for entries that never entered, incomplete leaves, and retirement of a current
or non-quiescent era. Tests: `replay_rejects_a_fact_the_ledger_never_accepted`,
`replay_rejects_replayed_identities_and_reordered_facts`,
`replay_rejects_substituted_and_foreign_contract_facts`,
`replay_rejects_retirement_of_a_current_or_non_quiescent_era`,
`replay_reconstructs_the_full_transition_roster`.

## Enforced: process-static services

Source: §Shared services and executable trust — each process-static service
owns key-collision/coexistence policy; atomic handover needs a non-replayed
receipt binding service, contract, key, registrations, eras, and
publication/retirement/obligation-transfer facts. Enforcement:
`executable_scopes/process_static_services.rs` — rejects handover without an
accepted atomic-handover receipt for an active logical key and without the
three independent provider facts ("requires an accepted atomic handover",
"has no atomic-handover contract identity"). Test:
`atomic_handover_requires_all_three_independent_provider_facts`.

## Enforced: custody on rejection

Source: §Opaque retention and quarantine — foreign-retained callback
registration consumes its external-root handle and returns it only after the
checks; rejection preserves custody. Enforcement:
`external-roots/src/root_entry/opaque_callback_replacement.rs` ("replayed
capacity claim rejects with every input returned"),
`external-roots/src/program_local/program_local_roots/installation_ledger.rs`
reject sites, `provider_execution.rs` ("consumer-validation rejection returns
the complete written carrier").

## Enforced: quarantine and capacity attribution

Source: quarantine detects stale entry but discharges no lock/claim/protocol
debt; repeated quarantines report attributed reserved-address capacity loss.
Enforcement: `executable-installation/.../replacement_quarantine.rs` —
`quarantine_installed`, `StaleEntryFault` (`discharged_obligations`),
`MappingQuarantineReceipt.attributed_capacity_loss`,
`MappingQuarantineCause::is_attributed`; uninstall propagates a
custody-preserving `UninstallError` (`uninstall.rs`).

## Enforced: service-carrier validity at check

Source: a bare boundary trait in value position does not denote a service
carrier and rejects; a record literal cannot manufacture a binding; zeroed
storage/equal bits/injection/proof alone cannot create one. Enforcement:
`04_typed-trees-to-checked-trees/src/checking/program_validation.rs` (bare
boundary trait rejection, "not a service carrier" diagnostic) and the service
custody checks under `execution/terminal_unit/`.

## Residual — no rejection site yet

- **Cohort-cut proof / enlarged-cohort report** (§Replacement granularity): the
  least-fixed-point computation over concrete implementations, selected
  conformances, layout, cleanup, state, and custody edges — "proves the
  requested cut, reports an enlarged cohort for explicit owner acceptance, or
  rejects" — has no code site. Residual under EPOCH-RESOURCE-SNAPSHOTS ("wider
  service-era replacement substrate breadth").
- **Explicit-migration and stable-object-identity tiers** (§Replacement and
  coexistence table): only drain/coexist is exercised — routing closes the old
  era while retaining its active invocations. Holder-cooperative migration
  (old handle consumed, new-era handle issued) and durable-handle redirection
  via a stable object table have no rejection surface yet.
- **State-migration theorem / descriptor reinterpretation fence** (§Returned
  values and custody): "a state-migration theorem cannot reinterpret a live
  inline carrier under a new descriptor — rebuild fused producers/consumers,
  enlarge the cohort, or reject" depends on the cohort machinery above.
- **Mapping-reuse authority proof**: the spec requires proof that no live
  authority reaches a mapping before reuse. Quarantine + attributed capacity
  loss are implemented; the positive proof obligation is residual.
- **Behavior-exclusion propagation on replacement** (§Candidate acceptance):
  "a replacement or rebinding that introduces excluded behavior rejects" —
  exclusion admission exists under BUILD-EXCLUSION-REALIZATION; rejecting a
  *replacement* for introduced exclusions follows the cohort/envelope work.

Verified 2026-09-20 on linux x86-64: `cargo nextest run -p effects --lib`
component_eras suites and `cargo nextest run -p external-roots --lib`
(green at claim time).
