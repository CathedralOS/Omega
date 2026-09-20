# Provider planning

[Provider selection](../../../../wiki/spec/build/provider_selection.md) and
[opaque representation selection](../../../../wiki/spec/build/opaque_representations.md)
own the public contract. Start at [provider_planning.rs](src/provider_planning.rs):
it selects exact provider plans and binds them to a checked program. The binding
operation resolves grants, plans receipt and operator evidence, resolves
installation reach, then publishes the checked updates together. [lib.rs](src/lib.rs)
only wires the public API.

[Receipt binding](src/provider_planning/receipt_binding.rs) validates admitted
receipts against exact granted requirements; [installation reach](src/provider_planning/installation_reach.rs)
resolves the selected realization's reach. Both return planned results without
mutating the checked program. [selection_provenance.rs](src/provider_planning/selection_provenance.rs) owns authored
selection inputs and target defaults.

Target defaults preserve their exact producer roster through target-marker
erasure and typed construction, then rejoin those typed machines before plan
selection. Do not replace the consuming carrier with a raw machine-name channel.
Authored selection identity/order and build-over-default precedence remain intact.

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
sits below the architecture test's runtime quarantine while
`component-candidate` stays outside this crate's closure. Routes that supply no
verified components call `selected_provider_plan_facts`, where every
Independent selection still rejects; the rejection never becomes a Fused
fallback, and retaining a mode in provenance is not implementation of that mode.
