# Trust admission

This realization package reconstructs exact trust obligations from Psi's complete
accepted-machine declarations, explicit root grants, and selected-provider
evidence. It has no typed-tree, checked-tree, interpreter, or compiler-driver
dependency. The declarations are invocation-owned; this boundary does not yet
establish canonical publication or independently admitted source-discard replay.

Start at [admission_settlement.rs](src/admission_settlement.rs).
`reconstruct_trust_obligations` resolves providers first, applies package policy,
resolves accepted-machine selectors, and constructs role-separated strong trust
digests. An explicit root grant is acceptance; the operation reads no policy
file and invents no approval.

Psi's existing [provider inventory](../../../../psi/compilation/representations/checked-trees/src/provider_inventory.rs)
owns the immutable source judgment and prepares its complete trust-declaration
roster once. Its [producer](../../../../psi/compilation/representations/checked-trees/src/trust_declarations.rs)
records exact declaration coordinates, eligible template identities, concrete
contract commitments, and missing or duplicate evidence. Dormant declarations
remain present. Their evidence failures stay deferred until the selected grant
actually needs them. A specialized accepted instance is not another grantable
template. Package admission and final reconstruction read the same inventory.

[accepted_machine_grants.rs](src/accepted_machine_grants.rs) resolves exact names
before unqualified leaf matches and rejects ambiguous names or repeated exact
symbols. [root_grants.rs](src/root_grants.rs) preserves the package-grant fence.
Backend [provider_grants.rs](../../backend/plans/provider-evidence/src/provider_grants.rs)
owns selected-provider lookup. [trust_admission](src/trust_admission/mod.rs)
constructs distinct provider, template, and concrete-contract digest domains;
compact report fingerprints never replace commitment authority.

[Component-assumption records](src/component_assumptions.rs) retain authored
digests and declaration provenance. They neither harvest source nor accept a
component. The former unused static harvester is removed; Build production and
component verification remain independently responsible for their contracts.

Behavior exclusions belong to Product orchestration. The
[Product selection owner](../../driver/src/checked/checking/product_selections.rs)
resolves executed Build requests against final source declarations. The
[Terminal admission adapter](../../driver/src/terminal/behavior_exclusions.rs)
checks the selected unoptimized composition and attributes refusals without
reopening source trees.

The workspace [boundary guard](../../../../../tools/check_native_boundary.py)
checks this crate's transitive production dependency closure, including optional,
build, and foreign-target edges. The [tests](src/tests.rs) distinguish selector
precedence, deferred cardinality failures, package admission, exact declaration
identity, and role-separated strong commitments.
