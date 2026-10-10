# generic copy forward

Compile-scope probe. No standard-library dependency.

- `copy_it<T [copy]>` instantiated from argument types (Light +
  i32); `wrap<U [copy]>` forwards its own bound as `copy_it`'s
  requirement — nested bound forwarding.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/generic_copy_forward/main.omg
mbx run -p omega -- update --project samples/cli/probes/generic_copy_forward
```
