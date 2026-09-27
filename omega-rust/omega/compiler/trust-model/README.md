# Trust admission and report reconstruction

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

- [trust_admission.rs](src/trust_admission/mod.rs) defines admission values, strong
  subject digests, and identity comparison. Compact report identities are not
  admission authority.
- [trust_report.rs](src/trust_report/mod.rs) reconstructs diagnostic report evidence.
- [provider_grants.rs](src/provider_grants/mod.rs) resolves selected-provider grants;
  [accepted_machine_grants.rs](src/accepted_machine_grants.rs) resolves the separate
  accepted-machine selectors.
- [accepted_templates.rs](src/accepted_templates/mod.rs) holds exact accepted-template
  classifications used during reconstruction.

The sibling [trust-ledger](../trust-ledger/README.md) owns policy-file reading
and explicit replacement. Compiler callers supply admissions in memory; report
rendering cannot turn a report into policy.
