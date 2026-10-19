# Assumed binding result bound

The root-assumed u32 result <= 1 survives each boundary and indexes `[7, 11]`
of length 2, observing 11; `Supplied::get` retains its own contract.

The original root reproduction is preserved in `main.omg` and `build.omg`.
The first fetched-main failure was:

```
call `assume_binding_contract` supplies static machine arguments, but its generic callee did not resolve
```

## What this row discharges

Owning spec: [assumed binding contracts](../../../../wiki/spec/build/assumed_binding_contracts.md)
(see "Implementation must demonstrate … an honestly checked supplier and
caller … and explicitly accepted refutable entry/exit relationships"). This row
discharges the **explicitly accepted exit-side result contract**: the root's
`assume_binding_contract<Required::get, Supplied::get>` installs
`Required::get ensures result <= 1` on a supplier that guarantees nothing,
and the caller's downstream index proof consumes the assumed bound. It
covers establishment (admission only after an inspected, accepted
**Extremely high risk - assumed binding contract** Product risk row),
composition (the bound substitutes across all nine call/data boundaries),
and identity (each row commits the exact contracts, mapping, selected
implementation, root, target, assumed chain and source closure — any source
edit invalidates acceptance).

Still open on the ASSUMED-BINDING-CONTRACTS board: input-side authority
issuance, invocation correspondence and operational/lifetime relationships
fail closed; unsupported authority establishment is not demonstrated here;
project-level acceptance across real dependency packages is exercised only
through the repository fixtures.

## Places and evidence

The nine places are `local`, `field`, `parameter`, `call_result`, `match_join`,
`const_name`, `other_state`, `other_machine`, and `target_body`. Each has an
`controls/insufficient_bound` twin which retains only `<= 2` and rejects at
the index with `cannot prove index … is within length 2`. The call-result
and state twins weaken the same retained fact at both the requirement and
forwarding declaration so no stronger `<= 1` remains. The match witness
invokes both arms. `LENGTH` remains 2 in the const place.

Run each place with `omega run <place>/main.omg` after inspecting and accepting
its exact package risk row through `omega update`. An assumption is root
installation policy, separate from the supplier's checked conformance and
from project acceptance. Every row is labelled **Extremely high risk - assumed
binding contract** and commits the contracts, mapping, selected implementation,
root, target, complete assumed chain and consumed semantic source closure.

All nine positives plus the root check and execute with native exit 0 and
Terminal exit 0 (`omega run --both`, macos_arm64; `omega.lock` rows accepted
per project for the host target — the lock is host-local and not committed).
Every weaker-bound twin rejects at its index (`omega --check`, exit 1).
Four hosted target bodies are source checked; foreign native execution is not
claimed.

Pinned fixtures: positives under `tests/omega/pass/facts/
assumed_binding_result_bound_through_<place>_exit` (consent via
`accepted_assumed_bindings.txt`), twins under `tests/omega/fail/facts/
assumed_binding_result_bound_through_<place>_rejected`, and matching
provider controls under `tests/omega/{pass,fail}/providers/`.

## Additional controls

`controls/` retains direct-supplier noninheritance, missing/competing/cyclic
supply, bad checked supplier guarantees, runtime carrier/order/passing-mode
mismatches, blocking incompatibility and build-time calls — all rejecting at
`--check` with their authored fragment. Two controls differ:

- `controls/missing_link` has no assume link, so `Required::get` has no
  provider. `--check` passes (provider admission is a realization
  obligation); `omega run` rejects `no provider is selected for boundary
  requirement` for the entry target.
- `controls/renamed_lifetime_mapping` is a positive: same lifetime arity,
  alpha-renamed (`'a,'b` vs `'x,'y`) — checks clean, proving the link accepts
  alpha-equivalent lifetime correspondence.

The bounded result link does not close input-side authority issuance or the
full ASSUMED-BINDING-CONTRACTS board outcome. Unsupported authority
establishment, invocation correspondence and operational/lifetime
relationships fail closed.

## Fixture consent and update invalidation

The nine corpus entry fixtures explicitly opt into repository test consent
with `accepted_assumed_bindings.txt`. The existing fixture package support
projects fresh exact root/target risk rows and supplies them through
ordinary package compilation. Production execution still requires its
accepted lock. Changing `Actual::get` from 1 to 0 after acceptance rejects
before execution because the source identity changed; the original source
is restored after this control.
