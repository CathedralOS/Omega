# Multi-Effect Trait

Compile-scope probe: one boundary machine declaring several service
identities. No standard-library dependency.

- `boundary trait Net { machine write_line(...) reaches Net
  Allocator; }` — the declared set is the surface's ceiling.
- `pub domain [u8]::Utf8 requires valid_utf8` — domain arg in the
  boundary signature.

## Gaps found while authoring (filed)

- A `Binding<Net>` field needs a Fused provider even for `--check`:
  `client.net.write_line` is provider-gated, so only the
  declaration surface is exercised.
- `size_of<T>()`/`align_of<T>()` are authored as ordinary calls but
  reject in packages: "unimplemented: requires a portable Terminal
  data-layout query" — the corpus fixture documents the same.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/multi_effect_trait/main.omg
mbx run -p omega -- update --project samples/cli/probes/multi_effect_trait
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
