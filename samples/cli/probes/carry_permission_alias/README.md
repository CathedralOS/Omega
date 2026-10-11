# carry_permission_alias

`pub domain Token::SuspensionSafe = Carry::AcrossSuspend` — domain ALIASES a carry permission; provider's `ensures result in Token::SuspensionSafe` grants it.\n\n## Gaps found while authoring (filed)\n- The alias resolves in checked semantics but NOT in package-review projection: a `pub boundary trait` contract naming it reports 'unresolved domain' (review-only; --check green). Trait kept non-pub here.

Compile-checked only: run a check target, not native.
