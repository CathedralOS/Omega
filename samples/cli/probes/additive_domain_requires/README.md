# Additive Domain Requires

Depend-free compile-scope probe: domain-constrained operator meanings —
the first sample where `+` selection is gated by a `requires` domain fact.

- `Quantity::Additive` is a domain over the carrier `value` requiring
  `value >= 0`; `Quantity { value: 3 }` establishes the domain fact for
  the literal at construction.
- `machine + Quantity::Additive::add` implements `+` only when its own
  `requires right in Quantity::Additive` discharges — operator selection
  is positive proof context, not a type rule.
- `combine`'s `requires a in D, b in D` carries the caller's facts into
  the `a + b` site, discharging both operand facts.

The machine combines 3 + 4 = 7 under proof, reads it back, and accepts.
Compile-checked only (no std dependency):

```sh
mbx run -p omega -- --check samples/cli/probes/additive_domain_requires/main.omg
mbx run -p omega -- update --project samples/cli/probes/additive_domain_requires
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
