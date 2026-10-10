# Coordinate domain

Depend-free compile-scope probe: a user-authored domain — the first sample
to declare `domain<const Axis: u64> i64::Coordinate<Axis>` and to attach
generic machines to it. Domain-typed values erase to the carrier inside an
operation body and re-qualify with the unit on the way out; there is no
builtin arithmetic fallback — only the machines homed in the domain serve
`+` and `*` on `i64 in Coordinate`.

- `as i64 in Coordinate<7>` casts a plain i64 into the domain
- `a + b` resolves to `Coordinate::add<Axis>` over the shared axis
- `v * k` resolves to `Coordinate::scale<Axis>` (the raw factor stays a
  plain `i64` — mixed domain/raw operands spell each side explicitly)
- a different axis is a different type: `Coordinate<3>` only composes with
  `Coordinate<3>` machines (the mix itself is a fail-corpus shape)

The machine computes `(40 + 2) * 2 == 84` on axis 7, then `20 + 1 == 21`
on axis 3. Compile-checked only (no std dependency, no ProcessExit
provider):

```sh
mbx run -p omega -- --check samples/cli/probes/coordinate_domain/main.omg
mbx run -p omega -- update --project samples/cli/probes/coordinate_domain
```

Both complete green on Linux x86-64; `omega.lock` publishes locally and is
ignored like every sample lock.
