# Drafts

Temporary investigations, working notes, and migration plans live here.
Useful project-specific material without a permanent documentation owner also
belongs here—for example, Cathedral alignment notes. It need not be a language
proposal or become part of the specification merely to remain available.
They do not define language or toolchain rules.

Each draft names its purpose and when it can be deleted. Promote useful settled
content to its proper owner, then delete the draft. Execution status belongs on
the existing task boards, not in a second tracking system.

- [Rust compiler completion](rust_compiler_completion.md): required release
  coverage before resuming self-hosting, removed after that migration closes.
- [Bootstrap cost investigation](bootstrap_cost_review.md): bounded Delta/P1
  feasibility evidence and its recorded decisions, not authority for another
  chain.
- [Bootstrap-chain comparisons](bootstrap_chain_alternatives.md): reference
  tradeoffs, not a proposed replacement; remove when superseded or no longer useful.
- [Learned optimization](learned_optimization_policy.md): exploratory workload,
  ranking, and search ideas; remove when superseded by a concrete design or unused.
- [Specialized-variant identity impact](specialized_variant_identity_impact.md):
  how future specialized variants interact with code identity, deduplication,
  and component replacement; remove when a variant producer lands or it is
  superseded.
- [Matching-logic interchange](matching_logic.md): research background and possible
  proof-route comparisons; remove when superseded by a concrete design or unused.
- [Proof-search caching](proof_search_cache.md): exploratory derivation reuse;
  replace with a measured proposal or remove when no longer useful.
- [Test-cycle measurements](test_cycle_measurements.md): dated Windows evidence
  for future scheduling and selection comparisons.
- [Cathedral alignment](cathedral_alignment.md): temporary cross-repository
  ownership and dependency map for OS bring-up.
- [Replacement rejection inventory](replacement_rejection_inventory.md): catalog
  of every spec-named component-replacement rejection obligation mapped to its
  enforcement site or residual owner; delete once the residual rows land.

Keep useful temporary residue here after review; delete obsolete or redundant
history. Concrete proposed language or toolchain changes belong in proposals.
