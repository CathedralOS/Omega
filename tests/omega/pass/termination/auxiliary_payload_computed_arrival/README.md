# Auxiliary payload computed arrival

The named state is reached only through a computed arrival whose payload slot
combines two auxiliary entry inputs. That slot names no entry role, so no
entry constraint can reach it and it can never establish the rank slot's
mapping; the rank itself still arrives by an exact mapping and descends.

From the repository root:

```text
cargo run -p omega -- --check tests/omega/pass/termination/auxiliary_payload_computed_arrival/main.omg
```

This must succeed at source checking. The paired
[`endpoint_folded_into_payload`](../../../fail/termination/endpoint_folded_into_payload/main.omg)
folds the range ceiling into the same payload shape and must reject: the
endpoint's only carrier is gone, so its pinning premise is missing.
