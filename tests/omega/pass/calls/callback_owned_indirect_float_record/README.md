# Direct floating ABI with indirect body storage

After package preparation:

```sh
omega run tests/omega/pass/calls/callback_owned_indirect_float_record/main.omg
```

Real system libffi passes and returns four doubles in D0–D3. The ordinary
body instead receives an owned indirect record and returns into a separate
caller-owned result area. A helper permutes all four members. Exit 0 requires
67/17/91/43 and unchanged 17/91/43/67 input.

This challenges direct-to-indirect argument transport and indirect-to-direct
result transport, not just forwarding identical ABI pointers. The Darwin CIF
prefix wrapper and explicit metadata loans match the other libffi witnesses.

The native entry control also validates this actual D0–D3 transport before
changing its CALL destination. Its generated effects must use vector state and
reject a plan that removes vector permission, independently of the body's own
effects. The complete entry emitter must reject the same changed plan.
