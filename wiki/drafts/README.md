# Drafts

Temporary investigations, working notes, and migration plans live here.
Useful project-specific material without a permanent documentation owner also
belongs here—for example, Cathedral alignment notes. It need not be a language
proposal or become part of the specification merely to remain available.
They do not define language or toolchain rules.

Each draft names its purpose and when it can be deleted. Promote useful settled
content to its proper owner, then delete the draft. Execution status belongs on
the existing task boards, not in a second tracking system.

Drafts are grouped by kind. A draft belongs to exactly one folder. This index
does not list every draft, so check the folder as well.

## Reference

Cross-cutting notes that outlive any single rung.

- [Floating-point realization](reference/float_realization.md): selected float
  plans, x86 FMA admission, IEEE comparison custody and control-register
  envelopes as the Rust backend realizes them; delete once each boundary has a
  permanent owner.
- [Bootstrap cost investigation](reference/bootstrap_cost_review.md): bounded Delta/P1
  feasibility evidence and its recorded decisions; delete when both sections
  are absorbed or superseded.
- [Bootstrap-chain comparisons](reference/bootstrap_chain_alternatives.md): reference
  tradeoffs, not a proposed replacement; delete when superseded by concrete
  measured work or no longer relevant.
- [Cathedral alignment](reference/cathedral_alignment.md): temporary cross-repository
  ownership and dependency map for OS bring-up; delete when the map is
  captured by the owning tasks and Cathedral design documents.
- [External proof import](reference/matching_logic_external_proof_import.md): the bounded
  comparison's checked-translation leg and its diagnostic inventory; delete
  once the comparison reaches a verdict or the importer is retired.
- [Learned optimization](reference/learned_optimization_policy.md): exploratory workload,
  ranking, and search ideas; delete when a measured workload justifies a
  concrete proposal or the product compiler takes the question.
- [Matching-logic interchange](reference/matching_logic.md): research background and
  possible proof-route comparisons; delete when superseded by a concrete design
  or no longer useful.
- [Proof-search caching](reference/proof_search_cache.md): exploratory derivation reuse;
  replace with a measured proposal or delete when no longer useful.
- [Rust compiler completion](reference/rust_compiler_completion.md): required release
  coverage before resuming self-hosting; delete the plan after closure,
  retaining the release evidence and maintained gates with their owners.
- [Typed-to-one-sorted encoding](reference/matching_logic_sort_encoding.md): how Omega
  types, slices, sums, and loans encode into the one-sorted fragment; delete
  once the bounded comparison rules on the encoding.
- [Compiler parallelism](reference/compiler_parallelism.md): primary-source research
  on how other compilers divide and schedule work, as hypotheses for Omega.
- [Ordinary invocations without normal completion](reference/terminal_no_normal_invocations.md):
  implementation note on `InvokeNoNormal` for the reference producer.

## Audits

Inventories of an existing surface and what each row still owes.

- [`dyn` retirement inventory](audits/dyn_retirement_inventory.md): why `dyn`
  migrates through a checked rewrite rule and the order of the compiler work;
  delete once source `dyn` parsing and `TypeReferenceNode::DynamicTrait` are gone.
- [Operator-introducer retirement inventory](audits/operator_introducer_retirement_inventory.md):
  every surface the separate `operator` introducer must retire, with removal
  order; delete once the introducer and its representation are gone.
- [Pipeline placement audit](audits/pipeline_placement_audit.md): which ownership
  bucket each durable identity and codec sits in, plus recorded rulings; delete
  once every open row-1 violation moves or is boarded.
- [Historical producer/checker audit](audits/producer_checker_decision_sharing_audit.md):
  findings about the audited revision, including superseded artifact formats;
  use the pipeline contract for current guarantees.
- [Proposition surface retirement](audits/proposition_surface_retirement.md): every
  grammar, pipeline and library surface the `proposition` retirement
  must drain; delete once the migration drains them.
- [Stage-output orphan audit](audits/stage_output_orphan_audit.md): whether each rule
  stage's produced route and each accessor channel has a reader; delete once
  the zero-reader accessors are pruned or ruled on.
- [Python outside `tools/`](audits/python_inventory_2026-11-13.md): every Python
  file outside `tools/`, classified, so each can be deleted or replaced by
  `mbx xtask`.
- [Recursive walker inventory](audits/recursive_walkers_2026-11-12.md): the Psi
  stage 01–03 walkers whose stack recursion follows source nesting, as the
  checklist for storing recursive terms as flat tables.

## Measurements

- [Incomplete optimizer promotion evidence](measurements/optimizer_promotion_evidence.md):
  retained evidence pointers, without approval or an automated gate; delete when
  the candidates receive completed reviews or are abandoned.

Dated cost and failure evidence, superseded by a later run.

- [Benchmark records](measurements/benchmarks.md): methodology and host coverage for the
  committed compile/memory/size/runtime records; delete once the harness
  README owns the method and the host-coverage account.
- [Test-cycle measurements](measurements/test_cycle_measurements.md): dated scheduling and
  package-review route attribution evidence; delete once the optimization items
  citing it close or a later measurement replaces those sections.

## Designs

- [Required issuance choice](designs/required_issuance_choice.md): every
  access plan states who may create values of its domain, with stock
  `IssuedBy` policies and issuance shown in package review; delete when the
  board item lands.
- [Boundary slots as static implementations](designs/boundary_slot_implementation_views.md):
  a boundary trait that inherits an ordinary trait can be passed, named and
  stored as a static implementation of it, still routed through the slot;
  delete when the board item lands.
- [`invokes` as an entry ceiling](designs/invokes_entry_ceiling.md): `invokes`
  bounds synchronous entry into callbacks and inbound entries, and a
  provider's host calls move to its derived inventory; delete when the board
  item lands.
- [Required automatic discharge](designs/required_automatic_discharge.md):
  the obligations every checker proves unaided are order reasoning and range
  propagation, with `--floor-only` to confirm sources need nothing more;
  delete when the board item lands.
- [Published write frames](designs/published_write_frames.md): the set of
  places a checked body may write is inferred and published as part of its
  contract, with no new syntax; delete when the board item lands.
- [Record or sum data](designs/record_or_sum_data.md): a `data` declaration
  holds fields or cases, never both; common fields beside alternatives are a
  record holding a sum, and `data` declarations nest; delete when the board
  item lands.
- [Typed static parameters](designs/typed_static_parameters.md): a static
  code parameter is one typed `const` value, and a machine is bound to a
  requirement only by a declared edge, a selection row or
  `(m satisfies T::m)`; delete when the board item lands.
- [Activation storage plans](designs/activation_storage_plans.md): suspension
  preserves the unfinished computation and its storage layout is a
  compile-time `ActivationStorage` argument at the start site, `Fixed` or
  `Split`; delete when the board item lands.
- [Translation coordinate spaces](designs/translation_coordinate_space.md): an
  installed mapping splits under the existing interval algebra by projecting
  each contiguous run into a space indexed by its offset; includes the
  implementation limits observed; delete when the canary passes.
- [Owned cleanup consumers](designs/owned_cleanup.md): the machine selected
  with `dropped by` takes its subject by value, and a consuming pattern in the
  type's own package is what takes such a value apart; delete when the board
  item lands.
- [Open callable contracts](designs/open_callable_contracts.md): an omitted
  `reaches` or `crashes` is open in every callable contract, and runtime
  callable types and boundary requirements must write both; delete when the
  board item lands.
- [Named domain casts](designs/named_domain_casts.md): a type gains or loses
  a domain only through `as =Keep -Remove +Add`, which accounts for every
  attached domain; implicit weakening, annotation-attached domains and
  whole-type cast targets are retired; delete when the board item lands.

- [Validation subjects](designs/validation_subjects.md): check the selected
  packages in full and dependencies only through their semantic closure, with
  `Build::validate`, the workspace default and admission validation; delete
  when the board item lands.

- [Static binder contract dependencies](designs/static_binder_contract_dependencies.md):
  one published dependency term per contract axis for static callbacks, the
  crash and termination rows, and the `reaches none`/`crashes none` spellings;
  delete when the board item lands.

- [Owning some packages](designs/owning_some_packages.md): owned hidden state
  under `some` through uniform carriers, the inductive uniformity rule, opened
  witnesses as construction witnesses and the linear-first lifecycle; delete
  when the board item lands.

- [Target-realized float arithmetic](designs/target_realized_float_arithmetic.md):
  canonical `f32`/`f64` operations realized from the selected target's declared
  table instead of Build provider selection, closing the bulk-selection
  question; delete when the board item lands.

- [Operation-record implementations](designs/operation_record_implementations.md):
  implementations as ordinary records of static callable selections and
  evidence, selection rows restored, structural identity and one schema for
  static and runtime use; delete when the board item lands.

- [Owned activations and typed replies](designs/owned_activations.md): source-held
  execution custody, stepping, typed park and reply, and generators and tasks
  as libraries, in three stages; delete when the board item lands.

- [Check on the lowered form](designs/check_on_lowered_form.md): architecture
  decision. Elaborate typed trees to unsealed Terminal Psi by structure alone,
  then check on that form. It replaces Psi 05, validation and Psi 06, which
  the owner ruled removed and which are deleted. Further construct families
  wait on its four-condition gate.

- [Type and value parameters on the lowered form](designs/lowered_type_generics.md):
  proposal for condition 3 of that gate beyond implementation parameters: a
  rigid sort for an opaque `T`, concrete arms under type equations, and
  certificates carried by substitution.

- [What each stage may trust](designs/stage_trust.md): hand-off audit,
  compiler rules for stage outputs and `"validated"` claims, and the rule that
  the checker accepts only kernel-verified proofs and rejects the rest at
  compile time, at no runtime cost.

- [Conditional callback carry](designs/conditional_callback_carry.md):
  the six acceptance cases for one generic resource helper across synchronous
  and suspending callbacks; the rules are in the spec. Delete when they pass.

- [Deferred callback reach](designs/deferred_callback_reach.md):
  migration and acceptance for inferred service rows through static machine and
  conformance selections, with an explicit service-free requirement audit.

- [Conservative markers and subsystem CLI](designs/operational_markers_and_cli.md):
  implementation route for call-site markers, source rewriting and the
  subsystem CLI; the rules are in the spec. Delete when the owning tasks close.

A proposed shape for work that has not landed.

- [Normalized call-inventory unification](designs/normalized_call_inventory_unification.md):
  design collapsing eight target call variants into one operation; delete once
  the unified `Call` lands and the retired variants are gone.
- [Scalar range contract migration](designs/range_suffix_migration.md): canonical
  per-position mapping to `where`/`requires`/`ensures` plus the audit rule for
  migration-generated domains; delete after the history inventory closes.

Keep useful temporary residue here after review; delete obsolete or redundant
history. Concrete proposed language or toolchain changes belong in proposals.
