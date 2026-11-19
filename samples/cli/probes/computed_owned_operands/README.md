# Computed Owned Operands

Runnable probe: call results passed straight into owned parameters,
`f(make(...))`, with no named local. Each nested call is an ordinary owned
temporary that its consumer's call moves into the callee.

It combines an affine record, a record whose cleanup hook runs once inside
the callee (observed through a stored loan), a linear value the callee
consumes, a sum case, three nested producers, a borrowed argument lent
before an owned temporary, a byte literal that builds an owned result, a
call result carried through a state transition, and one passed in that
later state.

```sh
mbx run -p omega -- run --both samples/cli/probes/computed_owned_operands/main.omg
```

Expected exit: 0 natively and in Terminal; a wrong result crashes with `Trap`.
