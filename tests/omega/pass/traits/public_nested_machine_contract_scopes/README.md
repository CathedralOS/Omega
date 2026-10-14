# Public nested machine contract scopes

Two public traits use equal-sized static telescopes with different callback
results, plus immutable and mutable borrowed results. Each nested callback's
lifetime shadows the containing method's spelling. Review must retain these
distinct contracts when preparing successive signatures.

`omega update --offline --project tests/omega/pass/traits/public_nested_machine_contract_scopes --target macos_arm64`
must review the complete public API successfully. `omega run` on this project
exits zero. The review route, not only checking the unused trait declarations,
exercises normalization of public contracts.
