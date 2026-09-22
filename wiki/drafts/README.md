# Drafts

Temporary investigations, working notes, and migration plans live here.
Useful project-specific material without a permanent documentation owner also
belongs here—for example, Cathedral alignment notes. It need not be a language
proposal or become part of the specification merely to remain available.
They do not define language or toolchain rules.

Each draft names its purpose and when it can be deleted. Promote useful settled
content to its proper owner, then delete the draft. Execution status belongs on
the existing task boards, not in a second tracking system.

- [Backend vocabulary rejection audit](backend_vocabulary_rejection_audit.md):
  catalog of the compiler-owned closed vocabularies and where non-admitted
  values or authored extension reject; delete once a permanent spec section
  owns the inventory or the closed sets stop moving.
- [Benchmark records](benchmarks.md): methodology and host coverage for the
  committed compile/memory/size/runtime records; delete once the harness
  README owns the method and the host-coverage account.
- [Bootstrap-chain comparisons](bootstrap_chain_alternatives.md): reference
  tradeoffs, not a proposed replacement; delete when superseded by concrete
  measured work or no longer relevant.
- [Bootstrap cost investigation](bootstrap_cost_review.md): bounded Delta/P1
  feasibility evidence and its recorded decisions; delete when both sections
  are absorbed or superseded.
- [C2L conjunct-lowering cliff](c2l_conjunct_lowering_cliff.md): measured
  hotspots behind the lowering member that never returns; delete once hotspot
  2 is repaired and the subject fixture returns a verdict.
- [Call-containing requirement transport](call_containing_scalar_requirement_transport.md):
  design for citing calls inside scalar contract clauses through Terminal;
  delete once cited-call transport lands and the reproducer's artifact
  verifies.
- [Canary corpus distribution](canary_corpus_distribution.md): measured census
  of the authored fixture inventory by group and outcome suffix; delete once a
  generated inventory report carries these counts.
- [Canary package-mode signal](canary_package_mode_signal.md): design replacing
  the harness's substring package-mode probe with a parsed build declaration;
  delete once that read lands and the named build-declaration exception is
  gone.
- [Cathedral alignment](cathedral_alignment.md): temporary cross-repository
  ownership and dependency map for OS bring-up; delete when the map is
  captured by the owning tasks and Cathedral design documents.
- [Contract-exit-fact checking cost](checking_contract_exit_fact_cost.md):
  sampled checking-stage hotspot with its landed hoist and two open
  candidates; delete once those candidates land or are ruled out.
- [Component substrate coverage](component_substrate_coverage.md): producer and
  replayer census of every component description fact; delete once the row's
  named legs land.
- [Condition-fact equality roster](condition_fact_equality_roster.md): sparse
  projection design for the cliff's open asymptotic centre; delete once the
  projection lands and that hotspot closes.
- [Coordinator over-ownership audit](coordinator_overownership_audit.md): sweep
  of the named sequencers against the coordinator rule; delete once F2, F3 and
  F4 are relocated by their sibling board items.
- [Coordinator scope audit](coordinator_scope_audit.md): sweep of coordinator
  and entry files for absorbed domain work; delete once the F3 watch item
  moves the product fences to the admission owner.
- [Crate-root responsibility audit](crate_root_responsibility_audit.md): sweep
  of every workspace crate root against the responsibility rule; delete once
  the three heavyweight representation roots move their vocabulary out.
- [Dynamic erased-argument lane](dynamic_erased_lane_channel.md): smallest
  sound channel for proof-only actuals on `&dyn` calls; replace or delete when
  the design question is settled elsewhere.
- [Evaluated foreign bindings](evaluated_foreign_bindings.md): resume recipe for
  the privileged port-effect producer chain; delete once the producer legs land
  and a port-bearing artifact replays its effects.
- [Filesystem release contract](filesystem_release_contract_z140.md): scope
  verification and paused-closure plan for the bounded release proof; delete
  once the occurrence derivation and structural field-store closure land.
- [Float FMA native transport](float_fma_native_transport.md): measured state of
  the scalar-FMA transport legs and their fences; delete once legs (b) and (c)
  land and the native-realization fences are removed.
- [Foreign snapshot disposition](foreign_snapshot_disposition.md): vocabulary
  and disposition rules for private copies of foreign-held state; delete once
  issuance attaches a disposition record and the spec owns the taxonomy.
- [General cyclic execution optimizer](general_cyclic_execution_optimizer.md):
  re-verification ledger for the optimizer half of ranked cyclic execution;
  delete once an admitted ranked call cycle reaches that half.
- [Graph cost-model study](graph_cost_model_study.md): inventory of the existing
  candidate seams and what a ranking measurement must show; delete once a
  measured corpus answers the question or the policy question closes.
- [Known baseline failures](known_baseline_failures.md): attribution evidence
  for failures that reproduce without a task diff; delete when no row survives.
- [Learned optimization](learned_optimization_policy.md): exploratory workload,
  ranking, and search ideas; delete when a measured workload justifies a
  concrete proposal or the product compiler takes the question.
- [Lookup-map justification audit](lookup_map_justification.md): census of every
  name-keyed map beside the scoped symbol tree; delete once the architecture
  test and its key-domain catalog carry these justifications.
- [Matching-logic interchange](matching_logic.md): research background and
  possible proof-route comparisons; delete when superseded by a concrete design
  or no longer useful.
- [External proof import](matching_logic_external_proof_import.md): the bounded
  comparison's checked-translation leg and its diagnostic inventory; delete
  once the comparison reaches a verdict or the importer is retired.
- [Matching-logic slice comparison](matching_logic_slice_comparison.md): measured
  route sizes, pinned case timings, and candidate-side slice record; delete once
  the metrics aggregation column lands and the divergence resolves.
- [Typed-to-one-sorted encoding](matching_logic_sort_encoding.md): how Omega
  types, slices, sums, and loans encode into the one-sorted fragment; delete
  once the bounded comparison rules on the encoding.
- [Normalized call-inventory unification](normalized_call_inventory_unification.md):
  design collapsing eight target call variants into one operation; delete once
  the unified `Call` lands and the retired variants are gone.
- [Normalized foreign legalization](normalized_foreign_legalization_leg.md):
  resume recipe for aggregate and descriptor foreign transport; delete once
  legalization and selection admit those argument classes with witnesses.
- [Operator-introducer retirement inventory](operator_introducer_retirement_inventory.md):
  every surface the separate `operator` introducer must retire, with removal
  order; delete once the introducer and its representation are gone.
- [Package-review hotspot attribution](package_review_hotspot_attribution.md):
  where `omega audit packages` time actually goes; delete once the arena and
  symbol-lookup hotspots are repaired or boarded.
- [Physical entry end to end](physical_entry_end_to_end.md): audit of the
  authored-entry-to-host-process route with its linux closure; delete once the
  remaining host legs report their own runs.
- [Pipeline placement audit](pipeline_placement_audit.md): which ownership
  bucket each durable identity and codec sits in, plus recorded rulings; delete
  once every open row-1 violation moves or is boarded.
- [PoC orphan-entrance audit](poc_orphan_entrance_audit.md): entrance inventory
  and caller graph for the parked spill families; delete once the park is
  disposed of — every family sequenced or deleted.
- [Producer/checker decision sharing](producer_checker_decision_sharing_audit.md):
  every surface where a producer-written decision reaches a checker, with its
  verdict; delete once the cluster closes and a spec section owns the table.
- [Proof-search caching](proof_search_cache.md): exploratory derivation reuse;
  replace with a measured proposal or delete when no longer useful.
- [Proposition surface retirement](proposition_surface_retirement.md): every
  grammar, pipeline, library, and corpus surface the `proposition` retirement
  must drain; delete once the migration drains them.
- [Psi stage timing carrier](psi_stage_timing_carrier.md): the Psi-owned timing
  row carrier, what landed, and the remaining call-site leg; delete once the
  prepared-project route threads the flag.
- [Range-suffix migration recipe](range_suffix_migration.md): mechanical
  per-position recipe for removing bracketed range annotations; delete once no
  corpus file spells the suffix and the parse path is gone.
- [Replacement rejection inventory](replacement_rejection_inventory.md): catalog
  of every spec-named component-replacement rejection obligation mapped to its
  enforcement site or residual owner; delete once the residual rows land.
- [Rust compiler completion](rust_compiler_completion.md): required release
  coverage before resuming self-hosting; delete the plan after closure,
  retaining the release evidence and maintained gates with their owners.
- [Reseal-elimination scope review](scope_review_reseal_elimination.md): which
  identity recomputations are contract-required and which are redundant; delete
  once the wrapper-object plan is sealed once per produced artifact.
- [Selected-rewrite catalog disposition](selected_rewrite_catalog_disposition_e7c0099c.md):
  retain-or-delete audit of the orphaned rewrite modules; delete once each
  retained rewrite carries an executed catalog entry or is deleted.
- [Slice-view sample frontier](slice_view_sample_frontier.md): the twelve
  samples blocked on borrowed non-byte slice views and what moved; delete once
  they reach entry establishment or the vocabulary spec lands.
- [Specialized-variant identity impact](specialized_variant_identity_impact.md):
  how future specialized variants interact with code identity, deduplication,
  and component replacement; delete when a variant producer lands or the
  referenced machinery changes.
- [Squalr geometry native hosts](squalr_geometry_native_hosts.md): per-host
  ledger and ceremony for the geometry parity acceptance; delete once every
  host leg is recorded.
- [Stage-crate ownership audit](stage_crate_ownership_audit.md): module
  inventory of all 21 stage crates against the ownership rules; delete once its
  five findings are boarded or repaired.
- [Stage-output orphan audit](stage_output_orphan_audit.md): whether each rule
  stage's produced route and each accessor channel has a reader; delete once
  the zero-reader accessors are pruned or ruled on.
- [std check duration](std_check_duration_linux_x86_64.md): measured cost of
  `omega --check` over the bundled standard library; delete once the
  `Filesystem::host` join regression closes and the check is remeasured.
- [Structural borrow identity host legs](structural_borrow_identity_host_legs.md):
  recording recipe for the matching-host runtime results; delete once all four
  hosted-target legs are recorded or the row retires.
- [Swarm commit signal](swarm_commit_signal.md): 1,000-commit noise measurement
  against published swarm-orchestration evidence; remove when the board/landing
  workflow makes its numbers stale or a later measurement supersedes it.
- [Terminal trap crash site](terminal_operation_trap_crash_site.md): the
  conforming observation-profile row shapes for an operation-level trap; delete
  once the owner question is answered and the profile moves.
- [Test-cycle measurements](test_cycle_measurements.md): dated scheduling and
  package-review route attribution evidence; delete once the optimization items
  citing it close or a later measurement replaces those sections.
- [Test-cycle selection remeasurement](test_cycle_selection_remeasurement.md):
  controlled Linux replacement for the selection and scheduling costs; delete
  once a later remeasurement supersedes it or the slow-tail policy is decided.
- [Toolchain-settled plan provenance replay](toolchain_settled_plan_provenance_replay.md):
  diagnosis of why minted toolchain-settled plans fail candidate provenance;
  delete once the settlement lane makes them pass replay.
- [Unused `self` receiver pin](unused_self_receiver_signature.md): the verified
  admission and the fixture that will pin it; delete once the fixture and its
  roster seat land.
- [Preserving decode and remainder custody](wire_preserving_decode.md): the
  preserving decode mode's shape, custody, and relay splice; delete when the
  mode and its demand/report surface land or the board retires the leg.
- [Write-only-borrow IEEE store branch](write_only_borrow_ieee_store_branch.md):
  recovery ledger and re-implementation contract for the parked computed-store
  slice; delete once the slice lands and the frontier pin flips.

Keep useful temporary residue here after review; delete obsolete or redundant
history. Concrete proposed language or toolchain changes belong in proposals.
