# Trust admission

This realization package owns the trust obligations of a checked source program
and its selected providers. It consumes Psi judgments and backend provider
evidence; it does not read project policy or depend on compiler orchestration.
Checked-tree inputs remain a transitional boundary, not a Terminal-only API.

Start at [admission_settlement.rs](src/admission_settlement.rs).
`reconstruct_trust_obligations` reconstructs ordinary root grant obligations;
it never reads a policy file or invents approval.

[root_grants.rs](src/root_grants.rs) owns the package-grant fence and the private
grant meanings retained during that invocation. Provider selectors resolve once
against the selected plans. Accepted-machine selectors resolve in authored order
alongside their exact definition and template checks, preserving which error is
reported first. No prepared grant value crosses the public API or survives into
another compilation.

Its [authored grant checker](src/root_grants/authored.rs) reconstructs static
acceptances with their exact source occurrences. Build execution and package
review both call it directly. [Component assumptions](src/component_assumptions.rs)
retain exact accepted digests and reject malformed declarations.
The [exclusion selection checker](src/behavior_exclusions/selections.rs)
rejoins executed interpreter receipts to exact toolchain cases or boundary
traits; unreachable calls still select nothing. These are typed-source admission
boundaries, not claims that this package is already a Terminal-only consumer.

The other owners are meaningful siblings:

- [behavior_exclusions.rs](src/behavior_exclusions.rs) admits authored exclusions
  against the retained unoptimized Terminal composition, using backend provider
  evidence. It owns source attribution for prohibited behavior and incomplete
  evidence; both remain rejections. Optional optimization never establishes
  absence. Authored exclusion records live beside the backend evidence checker,
  so Build evaluation and admission share them without a compiler dependency.
- [trust_admission.rs](src/trust_admission/mod.rs) defines admission values, strong
  subject digests, and identity comparison. Compact report identities are not
  admission authority.
- [assumed_bindings.rs](src/assumed_bindings.rs) describes exact assumed runtime
  relationships and joins them to independent receiving consent. It requires
  both declaration revisions and applications, their public contracts, positional
  correspondence, selected implementation and dependencies, calling/layout and
  structural compatibility, domain authority, and the exact semantic assumption.
  Missing producer evidence remains an error; a readable description cannot
  supply it. The receipt accepts the relationship only and creates no provider
  authority or checked conformance.
- Backend [provider_grants.rs](../../backend/plans/provider-evidence/src/provider_grants.rs)
  resolves selected-provider grants;
  [accepted_machine_grants.rs](src/accepted_machine_grants.rs) resolves the separate
  accepted-machine selectors.
- [accepted_templates.rs](src/accepted_templates/mod.rs) holds exact accepted-template
  classifications used during reconstruction.

A root grant in `build.omg` is the root's authorization; package acceptance is
retained through `omega.lock`. An assumed binding needs its own exact extremely
high risk relationship acceptance where receiving policy applies. Ordinary
provider acceptance cannot answer it. This crate reads no acceptance file and
creates no parallel approval database.
