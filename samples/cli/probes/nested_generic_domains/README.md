# Nested + Generic Domains

Compile-scope probe: nested domain requires, const-generic domains,
and domain-qualified top-level consts. No standard-library dependency.

- `domain u64::Nested requires value in Small` — a requires clause may
  reference another domain on the same carrier; literals re-prove it.
- `domain<const Bound: u64> u64::Below<Bound>` — const-generic domain
  parameters.
- `Packet::SmallCount requires packet.count in Nested` — nested
  requires through a record field; `Packet { count: 3 }` proves it.
- `const X: T in D = literal` — domain-qualified consts.

## Gaps found while authoring (filed)

- **Const-generic domain requires don't discharge from values.**
  Passing `3` (literal or const) into a `u64 in Below<5>` parameter
  fails `cannot prove requires contract` — the `value < Bound`
  predicate isn't resolved under the generic instantiation. Scalar
  escape hatch: `3 as u64 in Below<5>` grants explicitly.
- **Generic array-element domains have no proof route.**
  `domain<const Index: u64> [u64; 2]::SmallAt<Index>(bytes) requires
  bytes[Index] in Small` — neither a `[99, 3]` literal/const argument
  nor `[99, 3] as [u64; 2] in SmallAt<1>` can prove the predicate.
- **Domain-qualified consts evaluate to values.** `const INDEXED:
  u64 in Below<5> = 3` admits at declaration, but passing `INDEXED`
  re-proves `3 in Below<5>` and fails like a plain literal — the
  const's declared qualification isn't sticky at use sites.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/nested_generic_domains/main.omg
mbx run -p omega -- update --project samples/cli/probes/nested_generic_domains
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
