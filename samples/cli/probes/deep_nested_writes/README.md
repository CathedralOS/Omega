# deep nested writes

Compile-scope probe. No standard-library dependency.

- established_by_domains: `pub domain X::W established by T::m,
  P::m` multi-route provenance — the named establisher is itself
  the granter, so its `ensures result in W` discharges on a plain
  `Folder {}` construction inside the impl body.
- multi_effect_reaches: `reaches Net Allocator` multi-service
  trait surface; caller spells `reaches Net + Allocator`.
- worker_copy_states: record clone through a `&mut copied` state
  cycle + `requires position < 2` + whole-array-field literal
  copy + `-> (Worker::copy(copied))` state call result.
- deep_nested_writes: `main.nested.second.b`, sum-into-field
  `main.holder.msg = Msg::Pong{..}`, indexed-element-field
  `main.pair[1].a` — writes land without bleeding into neighbors.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/deep_nested_writes/main.omg
mbx run -p omega -- update --project samples/cli/probes/deep_nested_writes
```
