# Arithmetic Domains

Depend-free compile-scope probe: the defined-overflow arithmetic domains —
the first sample where the same operands observe three different answers
chosen by the operand's domain.

- `i8 in Saturating` clamps: 100 + 100 overflows the signed range
  [-128, 127] and the result saturates to 127.
- `i8 in Wrapping` wraps: the same 100 + 100 lands at -56.
- `u8 in Trapping` aborts on overflow; 100 + 50 = 150 stays legal under
  u8, proving a trapping operand participates in ordinary arithmetic
  without firing.

The domain is the observable difference, not the operands — the machine
accepts only on (127, -56, 150). Compile-checked only (no std
dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/arithmetic_domains/main.omg
mbx run -p omega -- update --project samples/cli/probes/arithmetic_domains
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
