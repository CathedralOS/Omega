# Provider realization evidence

Start at [provider_evidence.rs](src/provider_evidence.rs).
`SelectedProviderPlanFacts` keeps the exact selected plans, their closure
identity, execution scope, opaque admissions and installation reach. Building
it checks for unique plan identities and boundary slots before computing the
closure's report coordinate. Artifact replay uses the kept records and
strong commitments, not just that coordinate.

Subordinate owners:

- [providers/](src/provider_evidence/providers/mod.rs): provider plans and their
  canonical encoding.
- [authority.rs](src/provider_evidence/authority.rs): terminal mechanisms,
  physical authority classifications and service permissions.
- [executable_scopes.rs](src/provider_evidence/executable_scopes.rs): admitted
  executable manifests, scope boundaries, static service evidence, the
  canonical exclusion union, selected Terminal closure replay, and physical
  exclusion checks against reviewed mechanism dispositions. Build
  evaluation keeps authored selections; this source-free owner uses the
  selected plans and evidence without running a Build or loading source.
- [component_eras.rs](src/provider_evidence/component_eras.rs): era membership,
  entry ledgers and progress manifests.

The Cargo package is still `effects`. Source-language flow and effect summaries belong to
[Psi flow-effects](../../../../../psi/foundation/flow-effects/src/lib.rs).
