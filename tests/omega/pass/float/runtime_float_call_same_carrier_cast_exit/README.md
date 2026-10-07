# Payload-preserving scalar casts

From the repository root:

```sh
omega run --both tests/omega/pass/float/runtime_float_call_same_carrier_cast_exit/main.omg
```

A helper returns a finite floating value and increments its caller's counter.
Erasing the predicate with `as f64` changes no payload bits and must retain
one invocation. The same cast also appears in another helper's returned value.
Both values are 3.75, the counter is 2, and execution exits 0.

The negative `float/same_carrier_cast_cannot_mint_finite` control keeps predicate
establishment separate from this payload transfer. `float/finite_result_requires_proof`
also rejects a callee that claims finiteness without proving its returned value.
Existing named-policy and semantic-domain controls retain their independent
checking obligations.
