# Finite Family Code

Depend-free compile-scope probe: const-generic requirement families — the
first sample with a trait requirement bounded by a finite `where`
disjunction over a const parameter.

- `Shape<T>::code<Width: u32>` is a finite family: only `Width == 16` and
  `Width == 32` admit; every other instantiation is unsatisfiable by
  construction.
- `Item::code<Width>` declares `satisfies Shape<Item>::code` in its own
  signature — the authored row IS the conformance — and `Primary`'s
  roster binds it with `Shape::code = Item::code`.
- The `where` disjunction admits on trait requirements only: a free
  machine carrying the same clause is rejected at admission ("requires a
  structural type equation or a named conformance bound").

The machine calls `Item::code<16>` and `Item::code<32>` directly and
accepts when both rows return the item's 7. Compile-checked only (no std
dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/finite_family_code/main.omg
mbx run -p omega -- update --project samples/cli/probes/finite_family_code
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
