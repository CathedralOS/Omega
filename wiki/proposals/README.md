# Proposals

Concrete proposed changes belong here, separate from the current specification.
State the problem, proposed behavior, useful alternatives, and open questions.
Acceptance updates the specification; this directory does not define current rules.

## Naming and lifecycle

Use `NNNN_short_descriptive_name.md`, beginning with `0000`. The number is a stable
proposal ID, not priority or acceptance order. Do not routinely renumber or reuse
IDs; an explicit owner-directed reassignment updates every reference in the same sweep.
The next available ID is **0006**; advance this counter when adding a proposal,
including after accepted or withdrawn files have been removed.

Each proposal names its status, affected spec owners, concrete problem, proposed
behavior, viable and rejected alternatives, unresolved questions, and acceptance
evidence. Code examples in a proposal are candidates, not language documentation.
Keep uncertainty explicit; a detailed design or simple syntax is not acceptance.
Use [OWNER_QUESTIONS.md](../../OWNER_QUESTIONS.md) for questions requiring an owner
decision, linking back to the proposal rather than duplicating its design.

Acceptance requires an explicit owner decision. In the same sweep, update specs
and the guide, add bounded implementation tasks, update links, and remove the
superseded proposal. Git retains its history. Revoked acceptance returns to a
proposal and removes normative examples and implementation mandates; no subset
remains accepted without an explicit decision. Unaccepted proposals are not
execution-board prerequisites. Keep this process in this repository, without a
separate review system or historical decision ledger.

## Open proposals

- [0000: Anonymous machines](0000_anonymous_machines.md): all forms are proposed,
  including simple lambdas; compare ordinary named machines with restricted sugar
  and full anonymous contracts before accepting a scope.
- [0001: Named proof-formula syntax](0001_proof_formula_syntax.md): unproven ergonomic
  alternatives to ordinary contracts and named witness/law bundles.
- [0002: Author-invoked assertions and build diagnostics](0002_assertions_and_build_diagnostics.md):
  unapproved library, provider, and diagnostic-build alternatives; admissions
  require no runtime checking independently of this design.
- [0003: Proof-search caching](0003_proof_search_cache.md): untrusted derivation reuse;
  persistence, keys, and granularity remain unselected.
- [0004: Checked boundary topology](0004_checked_boundary_topology.md): explicit
  service-instance wiring, routing policies, codec, and installer in an ordinary
  package; depends on 0005 for build-only imports and generic staged publication.
  Defines verified component consumption, independently authorized owner intent,
  and binding/activation checks; no topology-specific compiler graph engine.
- [0005: Build dependency scopes and isolated inputs](0005_build_dependency_scopes.md):
  build-only imports, non-executing product references, deterministic captured
  inputs, sealed staging, linear output completion, and explicit artifact-only
  mode; compares Cargo, Bazel, and Nix without adopting ambient host access.

Temporary working notes belong in [drafts/](../drafts/README.md).
