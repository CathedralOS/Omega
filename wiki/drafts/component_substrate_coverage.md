# Component-substrate producer/replay coverage inventory

Purpose: the COMPONENT-SUBSTRATE item (TASKS.md) sequences "first inventory
actual producer/replay coverage" before exposing the verified-description
consumer to independent admission/replacement and TOPOLOGY-PLAN-VERIFICATION.
This note records that inventory as surveyed at `b855ed85c8`. It is a coverage
map, not a design proposal; delete or rewrite when the consumer exposure leg
lands.

The carrier, codec, and source-free consumer live in
`omega-rust/omega/backend/artifacts/component-description/`:
`describe_component_facts` builds the canonical `ComponentDescription` from
independently supplied facts; `verify_component` re-verifies one against a
`ComponentVerificationRequest` (subject, admission profile, accepted
assumptions) and yields `VerifiedComponent`; the wire codec round-trips every
roster in canonical order.

## Producers

| Producer | Crate | Facts supplied |
| --- | --- | --- |
| `describe_component` | `backend/artifacts/component-candidate` | Full: embedded artifact, selected provider plans, progress manifest, `stack_demand` (emitter-derived `StackDemandFacts`), `realization_identity` (native artifact digest) |
| `published_independent_component_description` | `compiler` (`compiler/package.rs`) | Psi capsule: artifact, selected provider plans, progress manifest; `stack_demand`/`realization_identity` absent (no native realization yet) |

Both producers funnel through `describe_component_facts`, which derives the
module-side inventory (`derive_component_inventory`) and emits imports,
exports, entries, outgoing authority, service bounds, custody, retained
providers, obligations, and assumptions in canonical order.

The `component-candidate` producer sits behind the runtime quarantine pinned
by `tests/architecture/layering.rs`
(`ordinary_compiler_and_package_closures_exclude_speculative_runtime_owners`,
`component_description_stays_below_the_runtime_quarantine`): only
`component-deployment` reaches it; compiler/package closures may not.

## Consumers and replay

| Consumer | Location | What it binds |
| --- | --- | --- |
| `verify_component` | `component-description` | Subject/profile/schema, entries, exports, outgoing authority, custody, providers, service bounds, obligations, assumptions; re-runs `terminal-verifier` on the embedded module under `admission_profile`; authority scan enumerates every `OperationKind` |
| `realizes_selected_plan` | `provider-planning` (`selection_provenance.rs`, `provider_planning/independent_components.rs`) | Joins each `Independent` selection to exactly one verified component; unmatched rows reject at the fence pinned by `independent_provider_selection_reaches_the_componentization_fence` |
| `verify_independent_component_descriptions` | `build-evaluation` (`provider_settlement/independent_components.rs`) | Settlement re-verifies attached descriptions under the build's admission profile and authored `accepted_assumptions` roster |
| Package-evidence replay | `packages/review/evidence` (`capture/providers/policy/replay.rs`) | Re-verifies the settled compilation's retained descriptions instead of hitting the fence again |
| `verified_components` | `packages/topology` | `AdmittedComponent` binds `verify_component` output to a request; `verified_instance`/`verified_instance_facts`/`check_instance_binding` reconstruct roster entries for `plan_composition`/`plan_verification` — the TOPOLOGY-PLAN-VERIFICATION exposure already exists |
| Emission fence | `terminal_artifact/composition_modes.rs` via `produce_retained_terminal_artifact`/`produce_program_entry_terminal_artifact` | Rejects a settled `Independent` edge rather than emitting a silently fused artifact |

## Coverage gaps found

- **No admission/replacement consumer of `verify_component` exists.**
  `component-deployment::begin_component_deployment` takes a
  `ComponentCandidate` and binds installed-code custody directly (preflight
  checks target architecture and exact materialized image bytes); it never
  verifies a description. `component-publication` (`stack_provision.rs`,
  `callback_registration.rs`) and `executable-installation` likewise never
  call `verify_component`. The "same consumer" the item wants exposed to
  independent admission/replacement is today only reached by build-time
  settlement, evidence replay, and topology.
- **`stack_demand` and `realization_identity` are Psi-absent by design**:
  only the quarantined native producer can fill them; compiler-published
  descriptions leave them unset and nothing invents stand-ins.
- **No mapping or lease row in the obligation/custody vocabulary.**
  `ObligationKind` = {StackProvision, ProviderOccurrence, ImportBinding,
  ProgressDemand, ResourceAdmission}; `CustodyKind` = {PlacedViewInput,
  ReborrowRootHandoff, ReborrowRestoredCall, CompletionReceipt,
  ProgramLocalRootIntroduction, BoundaryContentGuarantee,
  DynamicDescriptorCustody, SuspensionFrontier}. The spec's "mappings" and
  "leases" (mapping cohorts, code leases — `component_publication.md`) have
  no row in either enum yet, so no description can demand or carry them.
  `check_obligations` requires only artifact-derived rows; declared extras
  pass through, so new kinds are a carrier extension plus whichever Psi /
  component / provider owner produces the fact — not a consumer change.
- **The settled `Independent` edge carries nothing into the product** past
  the emission fence: no symbolic import/export rows, entry/leave or
  resource demands, or installation/replacement obligations reach the
  artifact.
- **Retired-domain and host boundaries**: `describe_component` evidence is
  publication-only; it grants no callable authority and satisfies none of
  its obligations. Reading a description grants no callable authority.

## Implications for the exposure leg

- Exposing `verify_component` to independent admission means wiring a
  consumer that currently has none — `component-deployment` (candidate →
  description → verified admission at `begin_component_deployment`) or a
  `component-publication` join — not adding a second census inside the
  verifier.
- Replacement exposure has no producer of "superseded description" facts at
  all; `executable-installation` owns generic executable custody, and
  WIRE-RUNTIME-AND-INSTALLATION still has no provider performing the
  write-to-execute transition, so component replacement obligations would
  initially be declared-only rows.
- Topology's existing `AdmittedComponent` binding is the reference shape for
  "consumer takes exactly `verify_component` output plus the request it
  verified under".
