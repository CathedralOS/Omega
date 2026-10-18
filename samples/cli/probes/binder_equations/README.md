# binder equations

Compile-scope probe. No standard-library dependency.

- `where Length == u64::AtMost<Capacity>` — omitted-binder instantiations
  (`u64 in AtMost<256>`, `u64 in AtMost<257>`), explicit pair
  `TinyBytes<u64 in AtMost<256>, 256>`.
- `domain<const Bound: u64> u64::AtMost<Bound>` + `where Length ==
  u64::AtMost<Capacity>` domain-index equation; instantiation
  `Domained<u64 in AtMost<256>, 256>`.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/binder_equations/main.omg
mbx run -p omega -- update --project samples/cli/probes/binder_equations
```
