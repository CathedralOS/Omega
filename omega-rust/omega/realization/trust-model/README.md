# Trust admission and report reconstruction

This realization package owns the trust obligations of a checked source program
and its selected providers. It consumes Psi judgments and backend provider
evidence; it does not read project policy or depend on compiler orchestration.
Checked-tree inputs remain a transitional boundary, not a Terminal-only API.

Start at [admission_settlement.rs](src/admission_settlement.rs).
`reconstruct_trust_settlement` reconstructs obligations, compares the supplied
admissions, then reconstructs and validates the trust report. Its result
distinguishes consumed, unresolved, and unused admissions; it never reads a
policy file or invents approval.

[root_grants.rs](src/root_grants.rs) owns the package-grant fence and the private
grant meanings retained during that invocation. Provider selectors resolve once
against the selected plans. Accepted-machine selectors resolve in authored order
alongside their exact definition and template checks, preserving which error is
reported first. Report reconstruction borrows those meanings and still replays
selected candidates, selected grants, and report evidence. No prepared grant
value crosses the public API or survives into another compilation.

The earlier typed package gate and standalone report reconstruction each resolve
their own inputs. Their independent checking boundaries remain explicit.

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
- [trust_report.rs](src/trust_report/mod.rs) reconstructs diagnostic report evidence.
  Its [records](src/trust_report/records.rs) retain and validate the exact report
  vocabulary. They belong with admission, not host output writing.
- Backend [provider_grants.rs](../../backend/plans/provider-evidence/src/provider_grants.rs)
  resolves selected-provider grants independently of report reconstruction;
  [accepted_machine_grants.rs](src/accepted_machine_grants.rs) resolves the separate
  accepted-machine selectors.
- [accepted_templates.rs](src/accepted_templates/mod.rs) holds exact accepted-template
  classifications used during reconstruction.

The CLI's independent [trust-ledger](../../omega/trust-ledger/README.md) owns policy-file reading
and explicit replacement. Compiler callers supply admissions in memory; report
rendering cannot turn a report into policy.
