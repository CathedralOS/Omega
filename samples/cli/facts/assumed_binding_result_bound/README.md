# Assumed binding result bound

The root-assumed u32 result <= 1 survives each boundary and indexes `[7, 11]`
of length 2, observing 11; `Supplied::get` retains its own contract.

The original root reproduction is preserved in `main.omg` and `build.omg`.
The first fetched-main failure was:

```
call `assume_binding_contract` supplies static machine arguments, but its generic callee did not resolve
```

Run each place with `omega run <place>/main.omg` after inspecting and accepting
its exact package risk row through `omega update`. An assumption is root
installation policy, separate from the supplier's checked conformance and
from project acceptance. Every row is labelled **Extremely high risk - assumed
binding contract** and commits the contracts, mapping, selected implementation,
root, target, complete assumed chain and consumed semantic source closure.

The nine places are `local`, `field`, `parameter`, `call_result`, `match_join`,
`const_name`, `other_state`, `other_machine`, and `target_body`. Each has an
`controls/insufficient_bound` twin which retains only `<= 2` and rejects at
the index. The call-result and state twins weaken the same retained fact at
both the requirement and forwarding declaration so no stronger `<= 1` remains.
The match witness invokes both arms. `LENGTH` remains 2 in the const place.

All nine positives check and execute with native exit 0 and Terminal exit 0.
Every weaker-bound twin rejects at its index. Each retained native image also
executes with exit 0; exported canonical Terminal envelopes decode, while
single-byte corruption rejects. Four hosted target bodies are source checked;
foreign native execution is not claimed.

Additional controls retain direct-supplier noninheritance, missing/competing or
cyclic supply, bad checked supplier guarantees, runtime carrier/order/passing
mode mismatches, blocking incompatibility and build-time calls. The bounded
result link does not close input-side authority issuance or the full
ASSUMED-BINDING-CONTRACTS board outcome. Unsupported authority establishment,
invocation correspondence and operational/lifetime relationships fail closed.

The nine corpus entry fixtures explicitly opt into repository test consent with
`accepted_assumed_bindings.txt`. The existing fixture package support projects
fresh exact root/target risk rows and supplies them through ordinary package
compilation. Production execution still requires its accepted lock. Changing
`Actual::get` from 1 to 0 after acceptance rejects before execution because the
source identity changed; the original source is restored after this control.
