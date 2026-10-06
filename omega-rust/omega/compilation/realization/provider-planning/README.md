# Provider planning

Omega realization owns provider selection and its calling, component and task
plans here, beside selected execution settlement. Exact provider-grant resolution
belongs to [backend provider evidence](../../backend/plans/provider-evidence/src/provider_grants.rs),
not compiler trust-report reconstruction.

This still consumes checked trees and package-input custody for compile-time
evaluation. Those dependencies remain an unfinished bridge to the Terminal-only
handoff; relocating the owner does not complete that boundary.

[Provider selection](../../../../../wiki/spec/build/provider_selection.md) and
[opaque representation selection](../../../../../wiki/spec/build/opaque_representations.md)
own the public contract. Start at [provider_settlement.rs](src/provider_settlement.rs)
for typed provider settlement: consume prepared target defaults, derive
candidates, select plans, construct canonical host-service plans, verify
independent components, and retain aligned facts and provenance. Derivation is
the one producer of each candidate: settlement does not re-validate what it just
derived ([verification placement](../../../../pipeline.md#verification-placement)),
and adapter refinement of a requirement's service and invocation ceilings is
Psi conformance checking. Build admission prepares target declarations before
this operation; it does not implement it. Package review replays host-service
and component admission through the same owner, and replays selected provenance
against the typed program with
[selection replay](src/provider_planning/provenance_replay/selection_replay.rs).

[provider_planning.rs](src/provider_planning.rs) binds selected plans to a checked program. The binding
operation resolves grants, plans receipt and operator evidence, resolves
installation reach, then publishes the checked updates together. [lib.rs](src/lib.rs)
only wires the public API.

[Receipt binding](src/provider_planning/receipt_binding.rs) validates admitted
receipts against exact granted requirements; [installation reach](src/provider_planning/installation_reach.rs)
resolves the selected realization's reach. Both return planned results without
mutating the checked program. [selection_provenance.rs](src/provider_planning/selection_provenance.rs) owns authored
selection inputs and target defaults.

For selected compile-time execution,
[adapter conformance](src/provider_planning/provenance_replay/adapter_conformance.rs)
joins the exact requirement schema and realization row to its checked adapter.
The compiler forwards that adapter to Psi's build-time evaluator, which owns
execution admissibility, operands and replay; it does not reconstruct provider
authority.

[Target declarations](src/target_machines.rs) prepare the source family roster,
rejoin generated extensions and select the product's typed bodies. The retained
carrier consumes that roster into typed defaults and origins for settlement;
build evaluation and the compiler do not implement those selection rules.
[Provider declarations](src/provider_declarations.rs) decode static defaults
and executed Build choices into the same exact selection identities. Package
review uses its declaration replay without executing Build again.

This is transitional source/checked-tree realization, not target-neutral Psi
checking or the final post-Terminal installation route. The dependency on Psi's
typed declaration checks supports these exact joins and does not give Omega
authority to change source-language meaning.

Target defaults preserve their exact producer roster through target-marker
erasure and typed construction, then rejoin those typed machines before plan
selection. Do not replace the consuming carrier with a raw machine-name channel.
Authored selection identity/order and build-over-default precedence remain intact.

[Program-entry planning](src/program_entry.rs) owns target-slot selection,
source-signature admission, paired semantic/physical calling contracts, package
provenance and receiver-establishment custody. It consumes root-binding rows,
not a Build configuration. Its [root-binding admission](src/program_entry/root_bindings.rs)
rejoins executed Build requests to exact declarations in their lexical product
package and rejects mismatched descriptions or duplicate slots. Build evaluation
supplies the execution receipts and asks this owner about product-entry query
compatibility; the compiler consumes its settled entry directly. Backend `program-entry-plan` retains storage and ABI evidence.
The focused planner tests run with
`cargo nextest run -p provider-planning --lib program_entry::tests`.

[Calling-policy planning](src/calling_policy_plans/mod.rs) consumes closed plans.
Its [opaque-use records](src/calling_policy_plans/opaque_representations.rs)
retain exact source joins, shape roots, application commitments, and explicit
lifecycle/movement dispositions. Arena symbols are private join coordinates;
canonical evidence uses package-qualified declarations, not arena identity.

`selection_provenance.rs` closes each selected Independent composition at the
component-closure fence: `selected_provider_plan_facts_with_independent_components`
requires exactly one verified component description
(`component_description::VerifiedComponent::realizes_selected_plan`) per
independently selected plan and one selected plan per supplied component;
`independent_components.rs` owns that join. Only the verifier-owned carrier may
establish it (a copied inventory cannot), which is why `component-description`
sits below native realization and deployment policy. Routes that supply no
verified components call `selected_provider_plan_facts`, where every Independent
selection still rejects; the rejection never becomes a Fused fallback, and
retaining a mode in provenance is not implementation of that mode.
