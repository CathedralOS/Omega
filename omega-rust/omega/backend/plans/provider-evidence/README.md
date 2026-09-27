# Provider realization evidence

Start at [provider_evidence.rs](src/provider_evidence.rs).
`SelectedProviderPlanFacts` retains the exact selected plans, their closure
identity, execution scope, opaque admissions and installation reach. Its
construction checks unique plan identities and boundary slots before computing
the closure's report coordinate. Artifact replay consumes the retained records
and strong commitments, not that compact coordinate alone.

Follow the subordinate owners:

- [providers/](src/provider_evidence/providers/mod.rs): provider plans and their
  canonical encoding, approvals and boundary-call audits.
- [authority/](src/provider_evidence/authority/mod.rs): terminal mechanisms,
  physical authority classifications and service permissions.
- [executable_scopes/](src/provider_evidence/executable_scopes/mod.rs): admitted
  executable manifests, scope boundaries and static service evidence.
- [component_eras/](src/provider_evidence/component_eras/mod.rs): era membership,
  entry ledgers and progress manifests.

The Cargo package remains `effects`; its public exports and evidence formats are
unchanged. Source-language flow and effect summaries belong to
[Psi flow-effects](../../../../psi/foundation/flow-effects/src/lib.rs).
