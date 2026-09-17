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
mutating the checked program. [selection.rs](src/selection.rs) owns authored
selection inputs and target defaults.

Target defaults preserve their exact producer roster through target-marker
erasure and typed construction, then rejoin those typed machines before plan
selection. Do not replace the consuming carrier with a raw machine-name channel.
Authored selection identity/order and build-over-default precedence remain intact.

[Calling-policy planning](src/calling_policy_plans.rs) consumes closed plans.
Its [opaque-use records](src/calling_policy_plans/opaque_representations.rs)
retain exact source joins, shape roots, application commitments, and explicit
lifecycle/movement dispositions. Arena symbols are private join coordinates;
canonical evidence uses package-qualified declarations, not arena identity.

`provider_planning.rs` currently rejects selected Independent composition before checked
product/review publication because component closure and the Service carrier are
not yet constructed. Retaining a mode in provenance is not implementation of
that mode, and the rejection must not become a Fused fallback. The consumer-side
join now exists as `VerifiedComponent::realizes_selected_plan` in
`component-candidate`, but this crate cannot consume it: the architecture test
keeps `component-candidate` out of the ordinary compiler closure, and only the
verifier-owned carrier may establish the join (a copied inventory cannot).
