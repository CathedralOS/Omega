# Drafts

Temporary investigations and implementation plans live here. Current language
rules belong in the specification; current stage ownership belongs in
[the pipeline contract](../../omega-rust/pipeline.md). Notes can explain those
rules but cannot override them. Proposals are not accepted contracts.

Keep useful constraints and unfinished dependencies, not another history of
rulings, old pass totals or completed migrations. Git retains previous versions.
Each draft states its purpose and deletion condition. Promote settled content
to its owner and delete redundant material. Execution status stays on
[TASKS.md](../../TASKS.md), not a second tracking system.

The groups below are not an exhaustive file inventory.

## Reference

- [Floating-point realization](reference/float_realization.md): selected plans,
  FMA admission, comparison custody and control-state envelopes; delete once
  each boundary has a permanent owner.
- [Bootstrap cost investigation](reference/bootstrap_cost_review.md): bounded
  construction feasibility and open decisions; delete when their owners hold
  the useful constraints.
- [Bootstrap-chain comparisons](reference/bootstrap_chain_alternatives.md):
  tradeoffs, not a replacement proposal; delete when no longer useful.
- [Cathedral alignment](reference/cathedral_alignment.md): cross-repository
  ownership and dependencies; delete when the owning tasks carry the map.
- [External proof import](reference/matching_logic_external_proof_import.md):
  checked-translation experiment; delete after its verdict or retirement.
- [Learned optimization](reference/learned_optimization_policy.md): search
  hypotheses awaiting a measured customer, not product acceptance.
- [Matching-logic interchange](reference/matching_logic.md): possible proof-route
  comparisons; delete when superseded or no longer useful.
- [Proof-search reuse](reference/proof_search_cache.md): constraints on a measured
  in-memory experiment; no current instrumentation or hit rate is claimed.
- [Rust compiler completion](reference/rust_compiler_completion.md): six gates,
  existing test commands and four hosted runs; delete after closure, retaining
  maintained checks and the product's evidence with their owners.
- [Typed-to-one-sorted encoding](reference/matching_logic_sort_encoding.md):
  encoding in the bounded proof-route experiment; delete after its verdict.
- [Compiler parallelism](reference/compiler_parallelism.md): research hypotheses
  about work ownership and scheduling, not measured Omega speedups.
- [Ordinary invocations without normal completion](reference/terminal_no_normal_invocations.md):
  implementation guidance for `InvokeNoNormal`.

## Audits

An audit reports its recorded revision. Recheck a finding before treating it as
current. Retain unresolved evidence with its owning repair; completed history
belongs in Git rather than an enduring instruction to preserve old machinery.

- [`dyn` retirement](audits/dyn_retirement_inventory.md): drain the parser and
  representation, then delete the inventory.
- [Operator-introducer retirement](audits/operator_introducer_retirement_inventory.md):
  remove the separate introducer and its consumers, then delete the inventory.
- [Pipeline placement](audits/pipeline_placement_audit.md): ownership of durable
  identities/codecs; delete when each live violation is repaired or boarded.
- [Producer/checker decision sharing](audits/producer_checker_decision_sharing_audit.md):
  revision-scoped evidence, not current trust or artifact-format instructions.
- [Proposition surface retirement](audits/proposition_surface_retirement.md):
  drain the named surface and delete its inventory.
- [Stage-output orphans](audits/stage_output_orphan_audit.md): produced channels
  without consumers; delete after the remaining channels are ruled on.
- [Python outside tools](audits/python_inventory_2026-11-13.md): replacement
  checklist, not a permanent file census.
- [Recursive walkers](audits/recursive_walkers_2026-11-12.md): source-nesting
  recursion to replace with bounded explicit storage, then delete the inventory.

## Measurements

Keep inputs, command, source revision and host with a useful measurement. A
superseded run is not a current correctness or performance claim.

- [Optimizer promotion evidence](measurements/optimizer_promotion_evidence.md):
  incomplete candidate evidence, not approval or an automated gate.
- [Benchmark method](measurements/benchmarks.md): methodology and host coverage;
  delete when owning harness documentation carries them.
- [Test-cycle measurements](measurements/test_cycle_measurements.md): evidence
  for active repairs; remove superseded reports when those repairs close.

## Designs

- [Required issuance choice](designs/required_issuance_choice.md): explicit
  establishment policy and review; delete when its implementation task closes.
- [Boundary slots as static implementations](designs/boundary_slot_implementation_views.md):
  preserve requirement routing through ordinary static views; delete on delivery.
- [`invokes` as an entry ceiling](designs/invokes_entry_ceiling.md): callback and
  inbound-entry demands separate from provider plumbing; delete after migration.
- [Required automatic discharge](designs/required_automatic_discharge.md):
  implement the proof floor and `--floor-only`, then delete the plan.
- [Published write frames](designs/published_write_frames.md): retain inferred
  write footprints in contracts, then delete the migration plan.
- [Record or sum data](designs/record_or_sum_data.md): data holds fields or cases;
  migrate users instead of keeping two source dialects, then delete the plan.
- [Typed static parameters](designs/typed_static_parameters.md): static callable
  values and exact binding; delete after migration.
- [Activation storage](designs/activation_storage_plans.md): explicit backing at
  activation start and preservation across suspension; delete after delivery.
- [Translation coordinates](designs/translation_coordinate_space.md): interval
  projection and implementation limits; delete when its customer passes.
- [Owned cleanup](designs/owned_cleanup.md): consuming cleanup and decomposition;
  delete when the existing implementation task closes.
- [Open callable contracts](designs/open_callable_contracts.md): explicit runtime
  and boundary ceilings; delete after migration.
- [Named domain casts](designs/named_domain_casts.md): explicit accounting for
  qualifications; delete after the implementation and source migration.
- [Validation subjects](designs/validation_subjects.md): full subject checking and
  semantic dependency closure; delete after implementation.
- [Static binder contract dependencies](designs/static_binder_contract_dependencies.md):
  one published dependency term per axis; delete after publication and import work.
- [Owning some packages](designs/owning_some_packages.md): hidden owned state,
  uniform representation and lifecycle; delete when the owning task closes.
- [Target-realized float arithmetic](designs/target_realized_float_arithmetic.md):
  exact selected target meaning; delete after delivery.
- [Operation-record implementations](designs/operation_record_implementations.md):
  static selections and evidence in ordinary records; delete after migration.
- [Owned activations and typed replies](designs/owned_activations.md): retained
  execution custody, parking and replies; delete after delivery.
- [Check on the lowered form](designs/check_on_lowered_form.md): current checker
  architecture, obligation-coverage controls and lessons. There is no old-checker
  fallback or paused-work gate to revive.
- [Lowered type and value generics](designs/lowered_type_generics.md): rigid
  generic checking and evidence substitution; implementation stays with the
  checker task, not a superseded experiment pause.
- [Stage trust](designs/stage_trust.md): current distinctions between accepted
  judgments, seals, independent validation and receiving-host admission.
- [Conditional callback carry](designs/conditional_callback_carry.md): execution
  witnesses for synchronous and suspending selections; delete when they pass.
- [Deferred callback reach](designs/deferred_callback_reach.md): migrate inferred
  rows and explicit `reaches none` requirements, then delete the plan.
- [Operational markers and CLI](designs/operational_markers_and_cli.md): call-site
  markers and subsystem commands; delete on implementation closure.
- [Normalized call inventory](designs/normalized_call_inventory_unification.md):
  collapse duplicated forms without losing distinctions; delete after retirement.
- [Scalar range contracts](designs/range_suffix_migration.md): migrate intervals
  to their data/callable contracts, not new anonymous domains; delete on closure.

Unresolved source choices belong in [OWNER_QUESTIONS.md](../../OWNER_QUESTIONS.md).
Missing implementation of a settled rule does not. A link from an active task
is not a reason to keep that draft's obsolete instructions authoritative.
