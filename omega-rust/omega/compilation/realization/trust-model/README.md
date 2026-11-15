# Trust admission

This realization package rebuilds a program's exact trust obligations from
Psi's complete accepted-machine declarations, the root's explicit grants, and
selected-provider evidence. It has no typed-tree, interpreter or compiler-driver
dependency. The invocation owns the declarations; this package
does not yet publish canonically or replay discarded source.

Start at [admission_settlement.rs](src/admission_settlement.rs).
`reconstruct_trust_obligations` resolves providers first, applies package
policy, resolves accepted-machine selectors, and builds role-separated strong
trust digests. A root grant is the acceptance; the operation reads no
policy file and invents no approval.

Psi's provider inventory owns the fixed source judgment and prepares the
complete trust-declaration list once. It records exact declaration
coordinates, grantable template identities, concrete contract commitments, and
missing or duplicate evidence. Unused declarations stay listed; their evidence
failures wait until a selected grant needs them. A specialized accepted
instance is not another grantable template. Package admission and final
reconstruction read the same inventory.

[accepted_machine_grants.rs](src/accepted_machine_grants.rs) matches exact names
before unqualified leaves and rejects ambiguous names or repeated exact
symbols. It also keeps the package-grant fence
(`reject_package_non_provider_grants`). Backend
[provider_grants.rs](../../backend/plans/effects/src/provider_grants.rs)
looks up selected providers. [trust_admission](src/trust_admission.rs)
builds separate digest domains for providers, templates and concrete
contracts; short report fingerprints never replace the commitments.

Component-assumption records keep authored digests and declaration
provenance. They neither collect source nor accept a component. The old unused
static harvester is removed; Build production and component verification own
their contracts.

Product orchestration owns behavior exclusions. The
[Product selection owner](../../compiler/src/checked/checking/product_selections.rs)
resolves executed Build requests against final source declarations. The
Terminal admission adapter checks the selected unoptimized composition and
attributes refusals without reopening source trees.

`mbx xtask check-boundary` checks this crate's
whole production dependency closure, including optional, build and
foreign-target edges. The [tests](src/tests.rs) cover selector precedence,
deferred count failures, package admission, exact declaration identity, and
role-separated strong commitments.
