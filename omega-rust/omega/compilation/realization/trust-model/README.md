# Trust admission

This realization package owns the trust obligations of a checked source program
and its selected providers. It consumes Psi judgments and backend provider
evidence; it does not read project policy or depend on compiler orchestration.
Checked-tree inputs remain a transitional boundary, not a Terminal-only API.

Start at [admission_settlement.rs](src/admission_settlement.rs).
`reconstruct_trust_settlement` reconstructs obligations and compares the
supplied admissions. Its result distinguishes consumed, unresolved, and unused
admissions; it never reads a policy file or invents approval.

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
These are typed-source admission boundaries, not claims that this package is
already a Terminal-only consumer.

Behavior exclusions belong to Product orchestration, not grant reconstruction.
The [Product selection owner](../../driver/src/checked/checking/product_selections.rs)
resolves executed Build requests against the final source declarations. The
[Terminal admission adapter](../../driver/src/terminal/behavior_exclusions.rs)
checks the selected unoptimized composition against that policy and attributes
refusals. Its checker and shared records remain in backend provider evidence;
it never reopens source trees or resolves declarations.

The other owners are meaningful siblings:

- [trust_admission.rs](src/trust_admission/mod.rs) defines admission values, strong
  subject digests, and identity comparison. Compact report identities are not
  admission authority.
- Backend [provider_grants.rs](../../backend/plans/provider-evidence/src/provider_grants.rs)
  resolves selected-provider grants;
  [accepted_machine_grants.rs](src/accepted_machine_grants.rs) resolves the separate
  accepted-machine selectors.
- [accepted_templates.rs](src/accepted_templates/mod.rs) holds exact accepted-template
  classifications used during reconstruction.

A root grant in `build.omg` is the root's acceptance; dependency claims are
accepted through `omega.lock`. This crate compares against no second acceptance
record.
