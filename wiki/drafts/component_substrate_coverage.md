# Component substrate: producer/replay coverage inventory

Census of the `COMPONENT-SUBSTRATE` carrier (TASKS.md): which description
facts have a producer, which have an independent replayer, and which the
`Remaining work` bullets still owe. Audited on `9e80227d2b`, linux x86-64.

## Carrier and producers

`component-description` (schema `COMPONENT_DESCRIPTION_SCHEMA_V2 = 2`) is the
one canonical description. Two producers share one entrypoint,
`describe_component_facts(ComponentDescriptionFacts)`:

| Producer | Path | Supplies |
| --- | --- | --- |
| Psi capsule | `compiler::published_independent_component_description` (`compiler/package.rs`), driven by `compile_dependency_closure`'s retry with the `IndependentComponentDiscovery` write-beside output | artifact, selected-provider plans, optional progress manifest; `stack_demand: None`, `realization_identity: None` |
| Native candidate | `component-candidate/src/lib.rs::describe_component` | the same facts **plus** `StackDemandFacts` and the bound native-realization identity |

The native producer is behind the runtime quarantine
(`tests/architecture/layering.rs` forbids `component-candidate` —
which joins a native artifact to component policy — inside compilation
closures). That quarantine, not missing machinery, is why Psi-published
descriptions leave `stack_demand`/`realization_identity` absent.

## Per-roster replay coverage

`verify_component` (`component_verification.rs`) re-derives the module
inventory (`derive_component_inventory` — the shared producer/verifier
derivation) on its own decode, so no producer row is trusted rather than
replayed. Consumers: `provider-planning` closes each `Independent` plan via
`VerifiedComponent::realizes_selected_plan`; `build-evaluation`'s
`provider_settlement/independent_components.rs` re-verifies attached
descriptions at settlement; the package-evidence replay re-verifies under the
build's admission profile; `terminal_artifact/composition_modes.rs` fences
every settled `Independent` edge out of both product routes.

| Roster | Producer source | Replayer coverage |
| --- | --- | --- |
| `artifact_bytes` | sealed Terminal artifact | canonical decode + subject re-derivation (`terminal_psi_identity` vs `expected_subject`) + `terminal_verifier` under `admission_profile` + `EarlyFrontier` gate |
| `imports` | called requirements minus provider-sealed | `check_imports`: only unsealed requirements may slot, contract identity re-derived, no slot omission |
| `exports` | `inventory.exports` | `check_exports`: exact set equality |
| `entries` (7 kinds) | `inventory.entries` + assumption-bound declarations | `check_entries`: module-derived exact cover + `AssumptionBound` rows must name an accepted assumption |
| `outgoing` (3 classes) | called boundaries, port writes, concrete service reach | `check_outgoing`: exact cover; `ProviderSealed` must name a roster provider sealing that requirement; `PhysicalMechanism`/`AssumptionBound` digests must be accepted |
| `service_bounds` | `inventory.service_bounds` | `check_service_bounds` — exact cover, never folded into `ServiceCeiling` |
| `custody` (8 kinds) | `inventory.custody` | `check_custody` |
| `providers` + `provider_closure_digest` | `SelectedProviderPlanFacts` | `check_providers` against inventory + seal map; native side additionally cross-checks closure report/digest |
| `obligations` (5 kinds) | stack facts, unsealed imports, providers, `ProgramLocalRootIntroduction` custody rows, progress manifest | `check_obligations`: import/provider-occurrence/resource-admission rows are required-presence; stack and progress rows are declared facts the artifact cannot re-derive |
| `assumptions` | port-mechanism digests | every row must be in the request's accepted roster |
| `realization_identity` | native candidate only | artifact evidence only; never installation authority |

## Gaps this census confirms

1. **Native facts absent on the Psi path.** `stack_demand` and
   `realization_identity` stay `None` for compiler-published descriptions —
   so a Psi-published description emits **no `StackProvision` obligation**
   either. Delivering them is `component-candidate`'s native route, gated by
   the runtime quarantine and the native-realization delivery chain, not by
   missing description machinery.
2. **No mapping or lease rows can even be spelled.** `ObligationKind` =
   {StackProvision, ProviderOccurrence, ImportBinding, ProgressDemand,
   ResourceAdmission}; `CustodyKind` has eight variants — neither enum has a
   mapping or lease arm, so the row's "ObligationKind and CustodyKind carry
   no mapping or lease row yet" is a schema-level gap, not a producer gap.
3. **No carrier into the product for a settled `Independent` edge.** The
   `composition_modes` fence rejects rather than emitting a silent Fused
   artifact — symbolic imports/exports, entry/leave, resource demands, and
   installation/replacement obligations have no product-side representation
   yet.
4. **No installation/replacement consumer.** `component-deployment`,
   `component-publication`, `executable-installation`, `external-roots` are
   all inside the runtime quarantine; the only consumer of
   `verify_component` today is the settle-time join. An
   admission/replacement route and the TOPOLOGY-PLAN-VERIFICATION join do
   not exist.

## Consequence for the row's named legs

- "Supply the native facts" is blocked on getting a native artifact into a
  compilation-adjacent closure — the producer exists but is quarantined by
  design.
- "Realize the settled edge" is the carrier design: new product-side
  representation for the obligations/symbolic rows the description already
  publishes.
- "Expose the consumer to independent admission/replacement" needs the
  runtime-quarantined deployment/publication owners as consumers — new
  admission plumbing, not description changes.
