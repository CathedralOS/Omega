# carry_permission_alias

`pub domain Token::SuspensionSafe = Carry::AcrossSuspend` — domain ALIASES a carry permission; provider's `ensures result in Token::SuspensionSafe` grants it.\n\n## Gaps found while authoring (filed)\n- The alias resolves in checked semantics but NOT in package-review projection: a `pub boundary trait` contract naming it reports `unresolved domain` (review-only; --check green). Verified diagnosis: the membership fact resolves through the alias to `Carry::AcrossSuspend` — toolchain vocabulary with no declared domain, no symbol, and no semantic owner — and the review record's `Unresolved` owner is deliberately un-encodable ("admission must reject it"). The corpus fixture `carry_permission_provider_adapter` fails `omega update` identically: public contract projection of carry domains is a designed admission boundary (owner decision), not a missing resolver. Trait kept non-pub here.

Compile-checked only: run a check target, not native.
