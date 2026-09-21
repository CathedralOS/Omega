# Trust admission and report reconstruction

Start at [admission_settlement.rs](src/admission_settlement.rs) to follow
reconstruction of required admissions and comparison with the supplied policy.
Its result distinguishes consumed, unresolved, and unused admissions; it never
reads a policy file or invents approval.

The other owners are meaningful siblings:

- [trust_admission.rs](src/trust_admission/mod.rs) defines admission values, strong
  subject digests, and identity comparison. Compact report identities are not
  admission authority.
- [trust_report.rs](src/trust_report/mod.rs) reconstructs diagnostic report evidence.
- [provider_grants.rs](src/provider_grants/mod.rs) resolves selected-provider grants;
  [non_provider_grants.rs](src/non_provider_grants/mod.rs) resolves the separate
  accepted-machine selectors.
- [accepted_templates.rs](src/accepted_templates/mod.rs) holds exact accepted-template
  classifications used during reconstruction.

The sibling [trust-ledger](../trust-ledger/README.md) owns policy-file reading
and explicit replacement. Compiler callers supply admissions in memory; report
rendering cannot turn a report into policy.
