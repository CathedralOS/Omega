# call type args

Compile-scope probe. No standard-library dependency.

- `capacity<u64 in AtMost<257> >()` explicit type argument at a call +
  `where Length == u64::AtMost<Capacity>` type equation binding the
  const parameter.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/call_type_args/main.omg
mbx run -p omega -- update --project samples/cli/probes/call_type_args
```
