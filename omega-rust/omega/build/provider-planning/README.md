# Provider planning

[Provider selection](../../../../wiki/spec/build/provider_selection.md) and
[opaque representation selection](../../../../wiki/spec/build/opaque_representations.md)
own the public contract. Start at [lib.rs](src/lib.rs), then
[selection.rs](src/selection.rs) and [plans.rs](src/plans.rs).

Target defaults preserve their exact producer roster through target-marker
erasure and typed construction, then rejoin those typed machines before plan
selection. Do not replace the consuming carrier with a raw machine-name channel.
Authored selection identity/order and build-over-default precedence remain intact.

[Calling-policy planning](src/calling_policy_plans.rs) consumes closed plans.
Its [opaque-use records](src/calling_policy_plans/opaque_representations.rs)
retain exact source joins, shape roots, application commitments, and explicit
lifecycle/movement dispositions. Arena symbols are private join coordinates;
canonical evidence uses package-qualified declarations, not arena identity.

`plans.rs` currently rejects selected Independent composition before checked
product/review publication because component closure and the Service carrier are
not yet constructed. Retaining a mode in provenance is not implementation of
that mode, and the rejection must not become a Fused fallback.
